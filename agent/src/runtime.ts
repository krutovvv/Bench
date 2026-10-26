// The agent: Pi Durable runs the conversations with the extensions a profile selects
// (src/extensions.ts), this file gives them their model and turns Pi's events into the events
// Bench reads (message.delta, tool.start, clarify.request, …). It is also the tools' host.

import { BACKGROUND_CONTEXT, withAbortSignal } from "@earendil-works/chord/context";
import { createModels, createProvider, type Model } from "@earendil-works/pi-ai";
import { anthropicMessagesApi } from "@earendil-works/pi-ai/api/anthropic-messages.lazy";
import { openAICompletionsApi } from "@earendil-works/pi-ai/api/openai-completions.lazy";
import { InsertEntry, Inserts, insertsOf, patchInsert, type InsertView } from "./inserts.ts";
import type { JsonObject } from "@earendil-works/chord";
import { createRegistry, Harness, InboxDoc, watchEvents, type AgentEvent, type Conversation } from "@earendil-works/pi-durable";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { openNodeSqliteStorage } from "@earendil-works/pi-durable/storage/sqlite/node";
import { randomBytes, randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { arm, jobs, setScheduler } from "./builtin/cron.ts";
import { AGENT_HOME, agents, getSetting, readConfig, readAgent, reasoningEffort, resolveModel, setSetting, type ModelChoice } from "./config.ts";
import { EMPTY_TOTALS, planFresh, totalsOf, usagePayload } from "./usage.ts";
import { forAgent, installMcp, loadExtensions } from "./extensions.ts";
import { ensureMcp } from "./mcp.ts";
import { setHost, type Host, type SessionInfo } from "./host.ts";
import { projects } from "./builtin/projects.ts";
import { Store, type SessionRow, type Todo } from "./store.ts";

const ctx = BACKGROUND_CONTEXT;

/** One event for Bench: `{ session_id, type, payload }`. */
export type Emit = (sessionId: string, type: string, payload: Record<string, unknown>) => void;

interface Pending {
	resolve(answer: string): void;
	kind: "clarify" | "approval";
	payload: Record<string, unknown>;
}

/** What one open session keeps in memory while it lives. */
interface Live {
	row: SessionRow;
	conversation: Conversation;
	busy: boolean;
	generation: number;
	abort?: AbortController;
	/** Text of the in-flight assistant message per content block, to turn snapshots into deltas. */
	blocks: Map<number, { type: string; text: string }>;
	answer: string;
	/** Why this run died, if it did: said at once and kept with the answer, so a dead turn is
	 * never just silence. */
	fail?: string;
	prompts: Map<string, Pending>;
	stop?: () => Promise<unknown>;
	usage: Totals & { started: number };
	approvedForSession: Set<string>;
	/** Arguments of the running tool calls, by call id. */
	toolArgs: Map<string, any>;
	/** The thread's inserts as last read from Pi (the tools' view of them). */
	inserts: InsertView[];
	/** The messages waiting in Pi's inbox, as last told to Bench. */
	queue: QueuedMessage[];
	/** Follow-ups Pi placed while the run was ending: told to Bench after its answer, so each answer stays with its message. The id lets Bench retire its row in the queue. */
	placed: { id: string; text: string }[];
	/** Messages this agent queued, by Pi submission id, until placed or taken back. */
	pending: Map<string, { text: string; mode: string }>;
	/** Queue checks, one at a time. */
	queueLine: Promise<void>;
}

/** A message the person sent while the agent works, waiting in the conversation's inbox. */
export interface QueuedMessage {
	id: string;
	/** "followUp": sent when the agent answers; "steer": joins the running work after its tools. */
	mode: string;
	text: string;
}

/** The text of a user message's content (a string, or blocks). */
function contentText(c: any): string {
	return typeof c === "string" ? c : Array.isArray(c) ? c.filter((b: any) => b?.type === "text").map((b: any) => b.text).join("") : "";
}

/** An old thread's subject as an insert: "files:<id>" → files {host}, "tasks:today" → tasks {list}… */
function legacyInsert(subject: string): { key: string; state: JsonObject } | undefined {
	const [kind, ...rest] = subject.split(":");
	const value = rest.join(":");
	if (kind === "ssh") return { key: `ssh:${value || "local"}`, state: {} };
	if (kind === "files" && value) return { key: "files", state: { host: value } };
	if (kind === "tasks") return { key: "tasks", state: { list: value || "today" } };
	if (kind === "tool" && value) return { key: "tools", state: { tool: value } };
	if (kind === "browser") return { key: "browser", state: {} };
	return undefined;
}

const BLOCK_OPEN = "<block-state>";
const BLOCK_CLOSE = "</block-state>";
/** How a stopped run says so in the history: one quoted line, the same live and after a restart. */
const RUN_STOPPED = "> The run stopped: ";

/** A user message without the block state Bench sent with it. */
export function stripBlock(text: string): string {
	const at = text.lastIndexOf(BLOCK_OPEN);
	return at >= 0 && text.trimEnd().endsWith(BLOCK_CLOSE) ? text.slice(0, at).trimEnd() : text;
}

/**
 * A waiting message's content with its words replaced: the block state Bench sent with it stays
 * (the model still sees what the insert showed), and so does every attachment — only the person's
 * words change. Reading and writing use the same two shapes: a string, or a text block then the
 * attachments.
 */
export function withText(content: any, text: string): any {
	const said = contentText(content);
	const at = said.lastIndexOf(BLOCK_OPEN);
	const block = at >= 0 && said.trimEnd().endsWith(BLOCK_CLOSE) ? `\n\n${said.slice(at)}` : "";
	const body = `${text}${block}`;
	const files = Array.isArray(content) ? content.filter((b: any) => b?.type !== "text") : [];
	return files.length ? [{ type: "text", text: body }, ...files] : body;
}

export class Runtime {
	store = new Store();
	harness!: Harness;
	/** Pi's storage: the whole transcript for Bench (the conversation's view is what the model sees, compacted). */
	storage!: any;
	models = createModels();
	live = new Map<string, Live>();
	byConversation = new Map<string, Live>();
	private registered = new Set<string>();
	/** Subagents' conversations act for their parent's session. */
	private adopted = new Map<string, string>();

	listeners = new Set<Emit>();
	/** Every event goes to Bench's sockets (the sink) and to in-process listeners (cron, subagents). */
	emit: Emit = (sessionId, type, payload) => {
		this.sink(sessionId, type, payload);
		for (const l of this.listeners) l(sessionId, type, payload);
	};

	private sink: Emit;

	constructor(sink: Emit) {
		this.sink = sink;
	}

	async open(): Promise<void> {
		fs.mkdirSync(AGENT_HOME, { recursive: true });
		const registry = (this.registry = createRegistry());
		setHost(this.host());
		await loadExtensions(registry, () => this.reconfigureAll());
		this.storage = await openNodeSqliteStorage(path.join(AGENT_HOME, "pi.sqlite"));
		this.harness = await Harness.open(this.storage, {
			models: this.models,
			registry,
			// Pi's coding tools work in the conversation's directory.
			env: ({ cwd }: any) => new NodeExecutionEnv({ cwd: cwd ?? process.env.HOME ?? "/" }),
			identity: { attribution: { name: "bench-agent", url: "https://github.com/krutovvv/bench" } },
			settings: {
				retry: { maxRetries: Number(readConfig("default")?.agent?.api_max_retries ?? 3) },
				toolExecution: "parallel",
				afterFailedRun: "continue",
			},
			onReport: (error: unknown, info: any) => console.error("[pi]", info?.type, String((error as Error)?.message ?? error).slice(0, 300)),
		} as any, ctx);
		// Every model a stored session uses must be known before Pi resumes unfinished runs.
		for (const row of this.store.list({ limit: 10000 })) {
			try {
				this.modelRef(row.profile, resolveModel(row.profile, { model: row.model, provider: row.provider }));
			} catch {}
		}
		this.harness.resume();
		// Runs a previous process left unfinished continue now: watch their sessions so their
		// answers reach the index and Bench sees them as running.
		const graph = await this.harness.taskGraph(ctx);
		const conversations = new Set(Object.values((graph.value as any).tasks ?? {}).map((t: any) => String(t.conversationId)));
		graph.dispose?.();
		for (const c of conversations) {
			const row = this.store.byConversation(c);
			if (row) await this.session(row.id).catch(() => undefined);
		}
		// Scheduled jobs: their sleeping tasks live in one hidden conversation.
		setScheduler({
			conversation: async () => {
				const id = getSetting<number>("scheduler_conversation");
				const found = id !== undefined ? await this.harness.conversation(id as any, ctx) : undefined;
				if (found) return found;
				const created = await this.harness.createConversation({ ownership: { kind: "ownerless" } } as any, ctx);
				setSetting("scheduler_conversation", created.id);
				return created;
			},
			abortTask: async (id) => void (await this.harness.abortTask(Number(id) as any, ctx)),
		});
		for (const job of jobs.list()) if (job.enabled && !job.task_id) await arm(job).catch((e) => console.error("[cron]", job.name, e));
	}

	/** After an extension was reloaded or MCP reconnected: new turns use the new code. */
	reloadMcp(agent?: string): void {
		for (const a of agent ? [agent] : agents()) installMcp(this.registry, a);
		this.reconfigureAll();
	}

	private reconfigureAll(): void {
		for (const live of this.live.values()) void live.conversation.configure({ extensions: forAgent(readAgent(live.row.profile).extensions, live.row.profile) } as any, ctx).catch(() => {});
	}

	private registry!: ReturnType<typeof createRegistry>;

	// ---- models ----------------------------------------------------------

	/** Registers the profile's provider with pi-ai and returns the model reference for `configure()`. */
	private modelRef(profile: string, choice: ModelChoice): { provider: string; modelId: string } {
		const id = `${profile}.${choice.provider}`.replace(/[^a-zA-Z0-9._-]/g, "_");
		const key = `${id}|${choice.model}|${choice.baseUrl}|${choice.contextLength}|${choice.apiKey ? "k" : ""}`;
		if (!this.registered.has(key)) {
			this.registered.add(key);
			const anthropic = choice.apiMode === "anthropic_messages";
			const model: Model<any> = {
				id: choice.model,
				name: choice.model,
				api: anthropic ? "anthropic-messages" : "openai-completions",
				provider: id,
				baseUrl: choice.baseUrl,
				reasoning: true,
				input: ["text", "image"],
				cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
				contextWindow: choice.contextLength,
				maxTokens: Math.min(32000, Math.floor(choice.contextLength / 4)),
				compat: anthropic ? undefined : { supportsDeveloperRole: false },
			} as any;
			const apiKey = choice.apiKey;
			this.models.setProvider(
				createProvider({
					id,
					name: choice.provider,
					baseUrl: choice.baseUrl,
					auth: { apiKey: { name: choice.provider, resolve: async () => ({ auth: apiKey ? { apiKey } : {} }) } },
					models: [model],
					api: anthropic ? anthropicMessagesApi() : openAICompletionsApi(),
				} as any),
			);
		}
		return { provider: id, modelId: choice.model };
	}

	private thinking(effort: string): any {
		const e = (effort || "medium").toLowerCase();
		if (e === "none" || e === "off") return "off";
		return ["minimal", "low", "medium", "high", "xhigh"].includes(e) ? e : "medium";
	}

	// ---- sessions --------------------------------------------------------

	/** The agent's MCP servers, connected before its first turn. */
	private async mcpFor(agent: string): Promise<void> {
		// A project's threads use the general servers too.
		for (const a of agent.startsWith("project:") ? ["default", agent] : [agent])
			if (await ensureMcp(a).catch(() => false)) installMcp(this.registry, a);
	}

	async create(profile: string, cwd: string | undefined, source = "local", extra: Partial<SessionRow> = {}): Promise<Live> {
		await this.mcpFor(profile);
		const choice = resolveModel(profile);
		// A session with a project's environment (a thread, a cron run) belongs to the project and
		// works in its folder.
		const project = profile.startsWith("project:") ? profile.slice(8) : "";
		if (project && !extra.project) extra = { ...extra, project };
		const dir = cwd && fs.existsSync(cwd) ? cwd : project ? projects.dir(project) : defaultCwd(profile);
		const conversation = await this.harness.createConversation(
			{ ownership: { kind: "ownerless" }, agent: { model: this.modelRef(profile, choice), thinkingLevel: this.thinking(reasoningEffort(profile)), cwd: dir, extensions: forAgent(readAgent(profile).extensions, profile) } } as any,
			ctx,
		);
		const id = `${new Date().toISOString().slice(0, 10).replace(/-/g, "")}_${randomBytes(4).toString("hex")}`;
		const row = this.store.create({ id, conversation: conversation.id, profile, source, cwd: dir, model: choice.model, provider: choice.provider, effort: reasoningEffort(profile), ...extra });
		return this.attach(row, conversation);
	}

	/** The open session for `id`, attaching to its conversation on first use. */
	async session(id: string): Promise<Live | undefined> {
		const open = this.live.get(id);
		if (open) return open;
		const row = this.store.get(id);
		if (!row) return undefined;
		const conversation = await this.harness.conversation(Number(row.conversation) as any, ctx);
		if (!conversation) return undefined;
		return this.attach(row, conversation);
	}

	private async attach(row: SessionRow, conversation: Conversation): Promise<Live> {
		const live: Live = {
			row,
			conversation,
			busy: false,
			generation: 0,
			blocks: new Map(),
			answer: "",
			prompts: new Map(),
			// The counters survive in the row: reopening the thread continues the same numbers
			// rather than starting from zero (a fresh turn re-reads the window anyway).
			usage: { ...totalsOf(row.usage), started: Date.now() },
			approvedForSession: new Set(),
			toolArgs: new Map(),
			inserts: [],
			queue: [],
			placed: [],
			pending: new Map(),
			queueLine: Promise.resolve(),
		};
		this.live.set(row.id, live);
		this.byConversation.set(String(conversation.id), live);
		const stream = await watchEvents(this.harness, conversation.id, ctx);
		live.busy = stream.snapshot.run !== undefined;
		stream.start(async (events) => {
			for (const e of events) this.onEvent(live, e);
		});
		live.stop = () => stream.stop();
		await this.inserts(row.id).catch(() => {});
		return live;
	}

	async close(id: string): Promise<void> {
		const live = this.live.get(id);
		if (!live) return;
		this.live.delete(id);
		this.byConversation.delete(String(live.conversation.id));
		await live.stop?.();
	}

	async delete(id: string): Promise<void> {
		const live = this.live.get(id);
		if (live?.busy) await live.conversation.abort(ctx);
		await this.close(id);
		this.store.delete(id);
	}

	// ---- turns -----------------------------------------------------------

	/**
	 * A message from the person. While the agent works it waits in Pi's inbox (`mode` "queue",
	 * sent when the agent answers) or joins the running work after its current tools ("steer").
	 */
	async submit(id: string, text: string, attachments: { path: string; mime: string }[] = [], context = "", mode: "queue" | "steer" = "steer"): Promise<void> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		const row = this.store.get(id)!;
		live.row = row;
		await this.mcpFor(row.profile);
		// The model, effort and extensions follow the agent at every turn.
		const choice = resolveModel(row.profile, { model: row.model, provider: row.provider });
		await live.conversation.configure({ model: this.modelRef(row.profile, choice), thinkingLevel: this.thinking(row.effort || reasoningEffort(row.profile)), cwd: row.cwd, extensions: forAgent(readAgent(row.profile).extensions, row.profile) } as any, ctx);
		const images = attachments
			.filter((a) => a.mime.startsWith("image/") && fs.existsSync(a.path))
			.map((a) => ({ type: "image" as const, data: fs.readFileSync(a.path).toString("base64"), mimeType: a.mime }));
		// What the thread's live block shows, for the model only (Bench's history drops it).
		const said = context.trim() ? `${text}\n\n${BLOCK_OPEN}\n${context.trim()}\n${BLOCK_CLOSE}` : text;
		const content: any = images.length ? [{ type: "text", text: said }, ...images] : said;
		this.store.addMessage(id, { role: "user", text });
		if (live.busy) {
			// Queued: Pi keeps it in the inbox until the run answers. Steered: placed after the
			// current tool round.
			const sub = await live.conversation.submit({ type: "input", content, whenBusy: mode === "queue" ? "followUp" : "steer", requestId: randomUUID() }, ctx);
			// Watched until Pi places it (its turn starts) or it is taken back.
			live.pending.set(String(sub.id), { text, mode: mode === "queue" ? "followUp" : "steer" });
			if (mode === "queue") this.queueChanged(live);
			return;
		}
		live.busy = true;
		live.generation++;
		live.answer = "";
		live.blocks.clear();
		live.abort = new AbortController();
		live.usage.started = Date.now();
		live.usage.output = 0;
		this.emit(id, "message.start", {});
		await live.conversation.submit({ type: "input", content, requestId: randomUUID() }, ctx);
		if (!row.title) void this.title(live, text);
	}

	async interrupt(id: string): Promise<void> {
		const live = this.live.get(id);
		if (!live) return;
		live.abort?.abort();
		for (const [pid, p] of live.prompts) {
			p.resolve("");
			live.prompts.delete(pid);
		}
		await live.conversation.abort(ctx);
	}

	// ---- the queue (Pi's inbox) ------------------------------------------------

	/** The messages waiting in the thread's inbox, oldest first. */
	async queue(id: string): Promise<QueuedMessage[]> {
		const live = this.live.get(id) ?? (await this.session(id));
		if (!live) return [];
		const inbox = (await this.harness.snapshot(InboxDoc as any, live.conversation.id, ctx)) as any;
		return ((inbox?.items ?? []) as any[])
			.filter((i) => i.mode !== "write")
			.map((i) => ({ id: String(i.id), mode: i.mode, text: stripBlock(contentText(i.content)) }));
	}

	/**
	 * Something may have changed the queue (Pi's inbox events come in batches, so every chance
	 * counts: a submit, a withdrawal, a placed entry, a run's start and end). Checks run one at a
	 * time: Bench gets the queue when it changed, and the text of each queued message Pi placed in
	 * the conversation, as the thread's next turn.
	 */
	private queueChanged(live: Live): void {
		live.queueLine = live.queueLine.then(() => this.checkQueue(live)).catch((err) => console.error("[queue]", err));
	}

	private async checkQueue(live: Live): Promise<void> {
		const now = await this.queue(live.row.id);
		// Taken back (withdrawn, or dropped by Stop): no longer watched. A placed one is told by
		// its `entry_appended`.
		for (const sub of [...live.pending.keys()]) {
			if (now.some((n) => n.id === sub)) continue;
			const record = await (await this.harness.submission(sub as any, ctx))?.status(ctx).catch(() => undefined);
			if (record && (record.status === "queued" || ("entry" in record && record.entry !== undefined))) continue;
			live.pending.delete(sub);
		}
		if (JSON.stringify(now) !== JSON.stringify(live.queue)) {
			live.queue = now;
			this.emit(live.row.id, "session.queue", { items: now });
		}
	}

	/** Take a waiting message back out of the queue. */
	async withdraw(id: string, submission: string): Promise<string> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		const result = await this.harness.abortSubmission(submission as any, ctx, live.conversation.id);
		if (result === "aborted") {
			live.pending.delete(submission);
			await this.dropFromInbox(live, submission);
		}
		this.queueChanged(live);
		return result;
	}

	/**
	 * An aborted submission's inbox item: Pi 1.4 marks the submission unanswered but leaves the item,
	 * and the next boundary still places it. Taken out of the inbox document in a commit.
	 */
	private async dropFromInbox(live: Live, submission: string): Promise<void> {
		await live.conversation.commit(async (tx) => {
			const inbox = (await tx.doc(InboxDoc as any, live.conversation.id)) as any;
			const at = inbox.items.findIndex((i: any) => String(i.id) === submission);
			if (at >= 0) inbox.items.splice(at, 1);
		}, ctx);
	}

	/**
	 * Change the words of a message that still waits in the queue. It has not gone anywhere, so a
	 * wrong word is fixable; one already placed in the run is not — Pi settles that submission — and
	 * comes back as `not_queued`. The same inbox document we already patch to take a message out.
	 */
	async edit(id: string, submission: string, text: string): Promise<"edited" | "not_queued"> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		let done = false;
		await live.conversation.commit(async (tx) => {
			const inbox = (await tx.doc(InboxDoc as any, live.conversation.id)) as any;
			const at = inbox.items.findIndex((i: any) => String(i.id) === submission);
			if (at < 0 || inbox.items[at].mode === "write") return;
			inbox.items[at] = { ...inbox.items[at], content: withText(inbox.items[at].content, text) };
			done = true;
		}, ctx);
		if (done) {
			// History keeps the words the person meant, not the ones they fixed: the message was
			// stored when it was sent, before it ever waited.
			const watched = live.pending.get(submission);
			if (watched) {
				this.store.editUserMessage(live.row.id, watched.text, text);
				watched.text = text;
			}
		}
		this.queueChanged(live);
		return done ? "edited" : "not_queued";
	}

	/** Send a waiting message now: out of the queue, into the running work (or a new run). */
	async sendNow(id: string, submission: string): Promise<void> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		const inbox = (await this.harness.snapshot(InboxDoc as any, live.conversation.id, ctx)) as any;
		const item = (inbox?.items ?? []).find((i: any) => String(i.id) === submission);
		if (!item) return;
		if ((await this.harness.abortSubmission(submission as any, ctx, live.conversation.id)) !== "aborted") return;
		live.pending.delete(submission);
		await this.dropFromInbox(live, submission);
		// Its turn shows when Pi places it (after the current tools, or when the run answers).
		const sub = await live.conversation.submit({ type: "input", content: item.content, ...(live.busy ? { whenBusy: "steer" } : {}), requestId: randomUUID() } as any, ctx);
		live.pending.set(String(sub.id), { text: stripBlock(contentText(item.content)), mode: "steer" });
		this.queueChanged(live);
	}

	/** Regenerate: drop the last exchange (a fork before the last user message) so the next prompt replaces it. */
	async undo(id: string): Promise<void> {
		const live = await this.session(id);
		if (!live) return;
		const view = await live.conversation.viewState(ctx);
		const entries = (view.value as any).entries as any[];
		view.dispose();
		let last = -1;
		for (let i = entries.length - 1; i >= 0; i--) if (entries[i].kind === "pi.user") { last = i; break; }
		if (last <= 0) return;
		const fork = await live.conversation.fork(entries[last - 1].id, { ownership: { kind: "ownerless" } } as any, ctx);
		const msgs = this.store.messages(id);
		const lastUser = [...msgs].reverse().find((m) => m.role === "user");
		if (lastUser) this.store.truncateMessages(id, lastUser.timestamp);
		await this.close(id);
		this.store.update(id, { conversation: fork.id });
	}

	// ---- prompts (clarify, approval) --------------------------------------

	private prompt(live: Live, kind: "clarify" | "approval", payload: Record<string, unknown>, timeoutS: number): Promise<string> {
		const promptId = randomUUID();
		return new Promise<string>((resolve) => {
			const timer = setTimeout(() => done(""), timeoutS * 1000);
			const done = (answer: string) => {
				clearTimeout(timer);
				if (!live.prompts.delete(promptId)) return;
				this.emit(live.row.id, `${kind}.settled`, { prompt_id: promptId });
				resolve(answer);
			};
			const full = { ...payload, prompt_id: promptId, request_id: promptId, execution_generation: live.generation };
			live.prompts.set(promptId, { resolve: done, kind, payload: full });
			this.emit(live.row.id, `${kind}.request`, full);
		});
	}

	respond(id: string, promptId: string, answer: string): void {
		this.live.get(id)?.prompts.get(promptId)?.resolve(answer);
	}

	// ---- the tools' host ------------------------------------------------------

	// ---- inserts (src/inserts.ts) ----------------------------------------------

	/** The thread's inserts in order, read from its transcript and its document. */
	async inserts(id: string): Promise<InsertView[]> {
		const live = this.live.get(id) ?? (await this.session(id));
		if (!live) return [];
		const view = await live.conversation.viewState(ctx);
		const entries = ((view.value as any).entries ?? []) as any[];
		view.dispose();
		const doc = (await this.harness.snapshot(Inserts as any, live.conversation.id, ctx)) as any;
		live.inserts = insertsOf(entries, doc);
		// A thread from before inserts: its one tool ("subject") becomes its first insert.
		const legacy = !live.inserts.length && !doc?.items ? legacyInsert(live.row.subject ?? "") : undefined;
		if (legacy) {
			this.setSubject(id, "");
			return this.insert(id, randomBytes(4).toString("hex"), legacy.key, legacy.state);
		}
		return live.inserts;
	}

	/** Put an insert into the thread, where the conversation is now. An id it already has changes nothing. */
	async insert(id: string, insertId: string, key: string, state: JsonObject = {}): Promise<InsertView[]> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		if (!/^[\w-]{1,64}$/.test(insertId)) throw new Error("bad insert id");
		if (!key) throw new Error("insert key missing");
		const known = (await this.harness.snapshot(Inserts as any, live.conversation.id, ctx)) as any;
		if (known?.items?.[insertId]) return this.inserts(id);
		// The state first: an entry is never without its state.
		await live.conversation.commit(async (tx) => {
			const doc = (await tx.doc(Inserts as any, live.conversation.id)) as any;
			doc.items[insertId] = { key, state, folded: false, removed: false };
		}, ctx);
		const sub = await live.conversation.submit({ type: "write", entry: { kind: InsertEntry.kind ?? "bench.insert", data: { id: insertId, key } }, requestId: randomUUID() } as any, ctx);
		await sub.wait(ctx);
		return this.inserts(id);
	}

	/** Change an insert: its key (another server), its state, folded, or removed. */
	async patchInsert(id: string, insertId: string, patch: Record<string, unknown>): Promise<InsertView[]> {
		const live = await this.session(id);
		if (!live) throw new Error(`session not found: ${id}`);
		const found = await live.conversation.commit(async (tx) => patchInsert((await tx.doc(Inserts as any, live.conversation.id)) as any, insertId, patch), ctx);
		if (!found) throw new Error(`no insert ${insertId}`);
		return this.inserts(id);
	}

	/** The thread's block changes (a tool attached in it): stored, and the live turn's prompt sees it. */
	setSubject(id: string, subject: string): void {
		this.store.update(id, { subject });
		const live = this.live.get(id);
		if (live) live.row.subject = subject;
	}

	private sessionInfo(row: SessionRow): SessionInfo {
		return { id: row.id, profile: row.profile, cwd: row.cwd, source: row.source, chat_id: row.chat_id, subject: row.subject, project: row.project ?? "", inserts: this.live.get(row.id)?.inserts ?? [] };
	}

	private host(): Host {
		const cfg = () => readConfig("default");
		return {
			session: (conversationId) => {
				const key = String(conversationId);
				const live = this.byConversation.get(key);
				if (live) return this.sessionInfo(live.row);
				const adopted = this.adopted.get(key);
				const row = adopted ? this.store.get(adopted) : this.store.byConversation(key);
				return row ? this.sessionInfo(row) : undefined;
			},
			adopt: (conversationId, sessionId) => void this.adopted.set(String(conversationId), sessionId),
			ask: async (sessionId, question, choices, multiSelect, signal) => {
				const live = await this.session(sessionId);
				if (!live) return "";
				const answer = this.prompt(live, "clarify", { question, choices, multi_select: multiSelect }, Number(cfg()?.agent?.clarify_timeout ?? 600));
				signal?.addEventListener("abort", () => [...live.prompts.values()].forEach((p) => p.kind === "clarify" && p.resolve("")));
				return answer;
			},
			approve: async (sessionId, command, description) => {
				const live = await this.session(sessionId);
				if (!live) return "deny";
				if (live.approvedForSession.has(command)) return "session";
				const choice = await this.prompt(live, "approval", { command, description, choices: ["once", "session", "always", "deny"], allow_session: true, allow_permanent: true }, Number(cfg()?.approvals?.timeout ?? 120));
				if (choice === "session" || choice === "always") live.approvedForSession.add(command);
				return choice || "deny";
			},
			setTodos: (sessionId, todos: Todo[]) => {
				// A plan with new steps is a new plan: it gets its own moment, and Bench keeps its
				// block there instead of trailing it after the thread. Steps of the same plan
				// (same ids) only change their status and stay where they were put.
				const prev = this.store.get(sessionId)?.todos ?? [];
				const fresh = planFresh(prev, todos);
				const now = Date.now() / 1000;
				this.store.update(sessionId, fresh ? { todos, todos_at: now } : { todos });
				const at = fresh ? now : this.store.get(sessionId)?.todos_at ?? 0;
				const live = this.live.get(sessionId);
				if (live) live.row.todos = todos;
				this.emit(sessionId, "todo.updated", { todos, at });
			},
			todos: (sessionId) => this.store.get(sessionId)?.todos ?? [],
			emit: (sessionId, type, payload) => this.emit(sessionId, type, payload),
			search: (query, limit, profile) =>
				this.store.search(query, limit, profile).map((h) => ({ session_id: h.session_id, title: this.store.get(h.session_id)?.title ?? "", snippet: h.snippet, when: new Date(h.ts * 1000).toISOString() })),
			run: async (profile, prompt, opts) => {
				const r = await this.runToCompletion(profile, prompt, opts);
				return { sessionId: r.sessionId, text: r.text };
			},
			deliver: async (target, text) => {
				const [kind, to] = [target.slice(0, target.indexOf(":")), target.slice(target.indexOf(":") + 1)];
				const send = this.deliverers.get(kind);
				if (!send) throw new Error(`no way to deliver to ${kind}`);
				await send(to, text);
			},
		};
	}

	/** Where scheduled answers can go: "telegram" → (chat, text). */
	deliverers = new Map<string, (to: string, text: string) => Promise<void>>();

	// ---- events ----------------------------------------------------------

	private onEvent(live: Live, e: AgentEvent): void {
		const id = live.row.id;
		if (process.env.BENCH_AGENT_TRACE) fs.appendFileSync(process.env.BENCH_AGENT_TRACE, JSON.stringify(e) + "\n");
		switch (e.type) {
			case "inbox_update":
				this.queueChanged(live);
				break;
			case "submission": {
				// A queued message placed in the conversation: in the event stream's own order (after
				// the previous run's end, before the next one's start), so its turn comes between them.
				const rec: any = e.record;
				const item = live.pending.get(String(rec?.id));
				if (!item || rec.status !== "placed") break;
				live.pending.delete(String(rec.id));
				if (live.busy && item.mode === "steer") {
					// Joined the running turn (after its current tools): from here the work answers
					// this message, so its turn starts now, and the answer so far stays with the one
					// before.
					if (live.answer) this.store.addMessage(id, { role: "assistant", text: live.answer });
					live.answer = "";
					live.blocks.clear();
					this.emit(id, "message.user", { text: item.text, queued: true, id: String(rec.id) });
				} else if (live.busy) live.placed.push({ id: String(rec.id), text: item.text });
				else this.emit(id, "message.user", { text: item.text, queued: true, id: String(rec.id) });
				this.queueChanged(live);
				break;
			}
			case "run_start":
				if (!live.busy) {
					live.busy = true;
					live.generation++;
					this.emit(id, "message.start", {});
				}
				break;
			case "message_start": {
				live.blocks.clear();
				// The first tokens can come with the start itself.
				const msg: any = e.message;
				if (msg?.role === "assistant") msg.content?.forEach((b: any, i: number) => this.onChange(live, { type: "block", contentIndex: i, block: b }));
				break;
			}
			case "message_update":
				for (const c of e.changes) this.onChange(live, c as any);
				if (e.usage) live.usage.input = (e.usage.input ?? 0) + (e.usage.cacheRead ?? 0) + (e.usage.cacheWrite ?? 0) || live.usage.input;
				break;
			case "message_end": {
				const msg: any = e.entry.model?.[0];
				if (msg?.role === "assistant") {
					// Whatever the deltas missed (a final block replaced at once) is flushed here.
					msg.content?.forEach((b: any, i: number) => this.onChange(live, { type: "block", contentIndex: i, block: b }));
					const text = (msg.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
					if (text.trim()) live.answer = text;
					live.usage.calls++;
					if (msg.usage) {
						live.usage.input = (msg.usage.input ?? 0) + (msg.usage.cacheRead ?? 0) + (msg.usage.cacheWrite ?? 0) || live.usage.input;
						live.usage.output += msg.usage.output ?? 0;
						live.usage.cached = msg.usage.cacheRead ?? 0;
					}
					if (msg.stopReason === "error") this.failTurn(live, msg.errorMessage ?? "the model stopped with an error");
				}
				live.blocks.clear();
				break;
			}
			case "tool_execution_start": {
				if (e.parentToolCallId) break;
				const args = e.args as any;
				live.toolArgs.set(e.toolCallId, args);
				this.emit(id, "tool.start", { tool_id: e.toolCallId, name: e.toolName, args, context: preview(e.toolName, args) });
				this.store.addMessage(id, { role: "tool_call", text: JSON.stringify(args), tool_name: e.toolName, tool_call_id: e.toolCallId, args });
				break;
			}
			case "tool_execution_end": {
				if (e.parentToolCallId) break;
				const m: any = e.entry?.model?.[0];
				const text = (m?.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
				const details = (e.entry?.data as any)?.details ?? m?.details;
				let result: any = text;
				try {
					const parsed = JSON.parse(text);
					if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) result = parsed;
				} catch {}
				if (details && typeof details === "object") result = typeof result === "object" ? { ...result, ...details } : { output: result, ...details };
				if (m?.isError && typeof result === "object") result.success = false;
				// A file edit: the path and the diff Bench shows (Pi's edit gives a unified patch).
				const args = live.toolArgs.get(e.toolCallId);
				live.toolArgs.delete(e.toolCallId);
				if ((e.toolName === "edit" || e.toolName === "write") && args?.path && !m?.isError) {
					const full = path.resolve(live.row.cwd, String(args.path).replace(/^~(?=\/)/, process.env.HOME ?? "~"));
					result = { ...(typeof result === "object" ? result : { output: result }), resolved_path: full, ...(details?.patch ? { diff: details.patch } : {}) };
				}
				this.emit(id, "tool.complete", { tool_id: e.toolCallId, name: e.toolName, result });
				this.store.addMessage(id, { role: "tool", text, tool_name: e.toolName, tool_call_id: e.toolCallId });
				break;
			}
			case "usage_changed":
				this.emitUsage(live);
				break;
			case "run_end":
				this.finish(live);
				this.queueChanged(live);
				break;
			case "task_failed":
				console.error("[pi] task failed", e.kind, e.message);
				// The generation task is the run itself: its failure is the reason the turn went
				// quiet, and run_end follows without carrying it.
				if (e.kind === "pi.generation") this.failTurn(live, e.message);
				break;
		}
	}

	private onChange(live: Live, c: { type: string; contentIndex: number; delta?: string; block?: any }): void {
		const id = live.row.id;
		const blockText = (b: any) => (b?.type === "text" ? b.text : b?.type === "thinking" ? b.thinking : undefined);
		const push = (kind: string, text: string) => {
			if (!text) return;
			this.emit(id, kind === "thinking" ? "reasoning.delta" : "message.delta", { text });
		};
		const cur = live.blocks.get(c.contentIndex);
		if (c.type === "text_delta" || c.type === "thinking_delta") {
			const kind = c.type === "text_delta" ? "text" : "thinking";
			const b = cur ?? { type: kind, text: "" };
			b.text += c.delta ?? "";
			live.blocks.set(c.contentIndex, b);
			push(kind, c.delta ?? "");
		} else if (c.type === "text_start" || c.type === "thinking_start" || c.type === "block") {
			const full = blockText(c.block);
			if (full === undefined) return;
			const kind = c.block.type;
			const had = cur?.text ?? "";
			if (full.startsWith(had)) push(kind, full.slice(had.length));
			live.blocks.set(c.contentIndex, { type: kind, text: full });
		}
	}

	private usageFrame(live: Live): Record<string, unknown> {
		const choice = resolveModel(live.row.profile, { model: live.row.model, provider: live.row.provider });
		return usagePayload(live.row.model, live.usage, choice.contextLength);
	}

	private emitUsage(live: Live): void {
		const secs = Math.max(1, (Date.now() - live.usage.started) / 1000);
		// Speed of the turn in progress; between turns, the last turn's.
		if (live.busy) live.usage.tps = Math.round((live.usage.output / secs) * 10) / 10;
		this.emit(live.row.id, "session.usage", { usage: this.usageFrame(live) });
		// Between turns these are the thread's settled numbers: save them, so the bar is not
		// empty when the thread or the app is reopened.
		if (!live.busy) this.store.update(live.row.id, { usage: { ...live.usage } });
	}

	/** The reason a run stopped, as one line. Said at once, kept in the history: without it a
	 * failed turn looks like an answer that had nothing to say, and after a restart there is
	 * nothing left to explain the emptiness. */
	private failTurn(live: Live, why: unknown): void {
		const text = String(why ?? "")
			.replace(/^error:?\s*/i, "")
			.replace(/\s+/g, " ")
			.trim()
			.slice(0, 400);
		if (!text || live.fail?.includes(text)) return;
		live.fail = live.fail ? `${live.fail}; ${text}` : text;
		this.emit(live.row.id, "message.fail", { text: RUN_STOPPED + text });
	}

	private finish(live: Live): void {
		if (!live.busy) return;
		live.busy = false;
		// The turn's blended rate, as one number worth remembering.
		const turnSecs = Math.max(1, (Date.now() - live.usage.started) / 1000);
		if (live.usage.output) live.usage.tps = Math.round((live.usage.output / turnSecs) * 10) / 10;
		const id = live.row.id;
		// The failure travels with the answer: half an answer plus the reason it stopped is the
		// truth of the turn, and it is what a reopened thread has to show.
		const text = live.answer + (live.fail ? `${live.answer ? "\n\n" : ""}${RUN_STOPPED}${live.fail}` : "");
		if (text) this.store.addMessage(id, { role: "assistant", text });
		this.store.touch(id);
		this.emitUsage(live);
		this.emit(id, "message.complete", { text });
		this.emit(id, "sessions.changed", {});
		for (const p of live.placed.splice(0))
			this.emit(id, "message.user", { text: p.text, queued: true, id: p.id });
	}

	// ---- titles ----------------------------------------------------------

	private async title(live: Live, message: string): Promise<void> {
		const fallback = message.replace(/\s+/g, " ").trim().slice(0, 60);
		try {
			const choice = resolveModel(live.row.profile, { model: live.row.model, provider: live.row.provider });
			const ref = this.modelRef(live.row.profile, choice);
			const model = this.models.getModel(ref.provider, ref.modelId)!;
			const res: any = await (this.models as any).complete(
				model,
				{ systemPrompt: TITLE_PROMPT, messages: [{ role: "user", content: message.slice(0, 2000), timestamp: Date.now() }] },
				{ reasoningEffort: undefined, maxTokens: 400 },
			);
			const out = (res?.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
			const m = out.match(/"title"\s*:\s*"([^"]+)"/);
			const title = (m?.[1] ?? out.replace(/<think>[\s\S]*?<\/think>/g, "")).trim().replace(/^["']|["'.]$/g, "").slice(0, 80);
			this.store.update(live.row.id, { title: title || fallback });
		} catch {
			this.store.update(live.row.id, { title: fallback });
		}
		this.emit(live.row.id, "sessions.changed", {});
	}

	// ---- history (session.resume) -------------------------------------------

	/** The transcript as Bench reads it: `messages[]`. */
	async history(id: string): Promise<any[]> {
		const live = await this.session(id);
		if (!live) return [];
		// Every entry, not the model's view: after a compaction the view starts at the summary, and
		// the person keeps seeing the whole thread. The summary itself is the model's, not shown.
		const entries: any[] = [];
		let cursor: any;
		do {
			const page = await this.storage.scanEntries({ conversationId: live.conversation.id, order: "ascending" }, 500, cursor, ctx);
			entries.push(...page.items);
			cursor = page.next;
		} while (cursor);
		const out: any[] = [];
		let row = 0;
		for (const entry of entries) {
			if (entry.kind === "pi.compaction") continue;
			for (const m of entry.model ?? []) {
				row++;
				if (m.role === "user") {
					const text = typeof m.content === "string" ? m.content : (m.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
					out.push({ role: "user", content: stripBlock(text), row_id: row });
				} else if (m.role === "assistant") {
					const text = m.content.filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
					const reasoning = m.content.filter((b: any) => b.type === "thinking").map((b: any) => b.thinking).join("");
					const calls = m.content.filter((b: any) => b.type === "toolCall");
					out.push({
						role: "assistant",
						content: text,
						reasoning,
						row_id: row,
						tool_calls: calls.map((c: any) => ({ id: c.id, type: "function", function: { name: c.name, arguments: JSON.stringify(c.arguments) } })),
					});
					for (const c of calls) out.push({ role: "tool_call", tool_name: c.name, name: c.name, tool_call_id: c.id, args: c.arguments, context: preview(c.name, c.arguments) });
				} else if (m.role === "toolResult") {
					const text = (m.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
					out.push({ role: "tool", content: text, tool_name: m.toolName, name: m.toolName, tool_call_id: m.toolCallId });
				}
			}
		}
		// Bench reads tool rows' args from the tool row itself.
		const args = new Map(out.filter((m) => m.role === "tool_call").map((m) => [m.tool_call_id, m]));
		return out
			.filter((m) => m.role !== "tool_call")
			.map((m) => (m.role === "tool" && args.has(m.tool_call_id) ? { ...m, args: args.get(m.tool_call_id).args, context: args.get(m.tool_call_id).context } : m));
	}

	pendingPrompts(id: string): any[] {
		const live = this.live.get(id);
		return live ? [...live.prompts.values()].map((p) => ({ kind: p.kind, ...p.payload })) : [];
	}

	isBusy(id: string): boolean {
		return this.live.get(id)?.busy ?? false;
	}

	/** Runs one prompt to its answer in a fresh session (cron jobs, subagents). */
	async runToCompletion(profile: string, prompt: string, opts: { source: string; cwd?: string; title?: string; signal?: AbortSignal; onEvent?: (type: string, payload: any) => void }): Promise<{ sessionId: string; text: string; status: string }> {
		const live = await this.create(profile, opts.cwd, opts.source, { title: opts.title ?? "" } as any);
		const done = new Promise<{ text: string; status: string }>((resolve) => {
			const listener: Emit = (sid, type, payload) => {
				if (sid !== live.row.id) return;
				opts.onEvent?.(type, payload);
				if (type === "message.complete") {
					this.listeners.delete(listener);
					resolve({ text: String(payload.text ?? ""), status: opts.signal?.aborted ? "interrupted" : "completed" });
				}
			};
			this.listeners.add(listener);
		});
		opts.signal?.addEventListener("abort", () => void this.interrupt(live.row.id));
		await this.submit(live.row.id, prompt);
		const r = await done;
		return { sessionId: live.row.id, ...r };
	}
}

const TITLE_PROMPT =
	"You name chat sessions. Given the user's opening message, write a title that lets them find this conversation again in a list.\n\n" +
	"Rules:\n- 3 to 7 words, sentence case (capitalize only the first word and proper nouns).\n- Name what the user wants DONE, not that they asked a question.\n" +
	"- Keep technical terms, filenames, numbers, and error codes exact.\n- Drop filler words: the, this, my, a, an.\n" +
	"- No trailing punctuation, no quotes, no tool names, no 'Title:' prefix.\n- Never answer the message. Name it.\n- Always produce something, even for a bare greeting.\n" +
	"- Write the title in the same language as the user's message.\n" +
	'Good: {"title": "Fix login button on mobile"}\nGood: {"title": "Postgres connection pool exhaustion"}\nGood: {"title": "Friendly greeting"}\n' +
	'Too vague: {"title": "Code changes"}\nToo long: {"title": "Investigate and fix the issue where the login button does not respond on mobile devices"}\n\n' +
	'Reply with JSON only: {"title": "..."}';

function mtime(file: string): number {
	try {
		return fs.statSync(file).mtimeMs;
	} catch {
		return 0;
	}
}

export function defaultCwd(profile: string): string {
	const cfg = readConfig(profile);
	const cwd = cfg?.terminal?.cwd;
	if (typeof cwd === "string" && cwd && cwd !== "." && fs.existsSync(cwd.replace(/^~/, process.env.HOME ?? ""))) return cwd.replace(/^~/, process.env.HOME ?? "");
	return process.env.HOME ?? "/";
}

/** The one-line detail Bench shows on a tool step (its `context`). */
export function preview(name: string, args: any): string {
	if (!args || typeof args !== "object") return "";
	const pick = args.command ?? args.path ?? args.query ?? args.url ?? args.urls?.[0] ?? args.name ?? args.goal ?? args.pattern ?? args.action;
	const text = typeof pick === "string" ? pick : JSON.stringify(args);
	return text.length > 160 ? text.slice(0, 157) + "..." : text;
}
