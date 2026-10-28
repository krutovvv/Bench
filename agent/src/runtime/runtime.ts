// The agent: Pi Durable runs the conversations with the extensions an agent's settings select
// (src/extensions.ts). This is the core of its threads — a thread made, opened, sent a message,
// stopped, forked — and where its other parts meet:
//
// - `models`: the model every thread runs with, and its fallback (models.ts);
// - `stream`: a run's events turned into what Bench hears (stream.ts);
// - `journal`: a thread read whole, and each commit's news told (thread.ts);
// - `holds`, `queue`, `asks`, `inserts`, `titles`, `processes`: the person's pause, the waiting
//   messages, the questions, the inserts, the titles, the background processes (each its file);
// - the tools' host and where they work (tools-host.ts).

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { createRegistry, Harness, watchEvents, type Conversation, type ConversationId, type TaskId } from "@earendil-works/pi-durable";
import { openNodeSqliteStorage } from "@earendil-works/pi-durable/storage/sqlite/node";
import type { SqliteStorage } from "@earendil-works/pi-durable/storage/sqlite";
import { randomBytes, randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { Asks } from "./asks.ts";
import { arm, jobs, setScheduler } from "../builtin/cron.ts";
import { Processes } from "../builtin/process.ts";
import { projects } from "../builtin/projects.ts";
import { AGENT_HOME, agents, defaultCwd, getSetting, readAgent, readConfig, reasoningEffort, setSetting } from "../config.ts";
import { forAgent, installMcp, loadExtensions } from "../extensions.ts";
import { HoldDoc, Holds, type Hold } from "./hold.ts";
import { setHost } from "../host.ts";
import { envFor, hostOf } from "./tools-host.ts";
import { ThreadInserts, type InsertView } from "./inserts.ts";
import { ensureMcp } from "../mcp/mcp.ts";
import { Models, thinking } from "./models.ts";
import type { Ask, Event, ThreadEvent } from "../protocol.gen.ts";
import { Queue } from "./queue.ts";
import { Store, type ThreadRow } from "../store.ts";
import { Stream } from "./stream.ts";
import { BLOCK_CLOSE, BLOCK_OPEN, Journal } from "./journal.ts";
import { Titles } from "./title.ts";
import { totalsOf, type Totals } from "./usage.ts";

const ctx = BACKGROUND_CONTEXT;

/** Tell every connected Bench something (crates/protocol `Event`). */
export type Publish = (event: Event) => void;

/** What a run does, for listeners in this process (Telegram, scheduled runs, subagents' parents):
 * not the wire — Bench reads a thread from its journal and its `ThreadEvent`s. */
export type RunEvent =
	| { kind: "start" }
	| { kind: "delta"; text: string }
	| { kind: "tool"; name: string }
	| { kind: "subagent"; goal: string }
	| { kind: "ask"; ask: Ask; question: string }
	| { kind: "complete"; text: string };
export type RunListener = (threadId: string, e: RunEvent) => void;

/** A call waiting for the person's answer. */
interface Pending {
	resolve(answer: string): void;
	ask: Ask;
}

/** What one open session keeps in memory while it lives. */
export interface Live {
	row: ThreadRow;
	conversation: Conversation;
	busy: boolean;
	/** The run's holds as last committed to its `bench.hold` document (src/hold.ts). */
	holds: Hold[];
	generation: number;
	abort?: AbortController;
	/** Text of the in-flight assistant message per content block, to turn snapshots into deltas. */
	blocks: Map<number, { type: string; text: string }>;
	/** The message being written has begun a tool call: its words are narration, not an answer. */
	narrating?: boolean;
	answer: string;
	/** Why this run died, if it did: said at once and kept with the answer, so a dead turn is
	 * never just silence. */
	fail?: string;
	prompts: Map<string, Pending>;
	stop?: () => Promise<unknown>;
	usage: Totals & { started: number };
	/** The thread's inserts as last read from Pi (the tools' view of them). */
	inserts: InsertView[];
}

/** A new thread's id: the day and a few random bytes. */
const newId = () => `${new Date().toISOString().slice(0, 10).replace(/-/g, "")}_${randomBytes(4).toString("hex")}`;

export class Runtime {
	store = new Store();
	harness!: Harness;
	/** Pi's storage: the whole transcript for Bench (the conversation's view is what the model sees, compacted). */
	storage!: SqliteStorage;
	live = new Map<string, Live>();
	byConversation = new Map<string, Live>();
	/** In-process listeners of what runs do (`RunEvent`). */
	runs = new Set<RunListener>();
	/** Where scheduled answers can go: "telegram" → (chat, text). */
	deliverers = new Map<string, (to: string, text: string) => Promise<void>>();

	readonly models = new Models();
	readonly stream = new Stream(this);
	readonly journal = new Journal(this);
	readonly holds = new Holds(this);
	readonly queue = new Queue(this);
	readonly asks = new Asks(this);
	readonly inserts = new ThreadInserts(this);
	readonly titles = new Titles(this);
	readonly processes = new Processes(this);

	private registry!: ReturnType<typeof createRegistry>;

	private publish: Publish;

	constructor(publish: Publish) {
		this.publish = publish;
	}

	/** A change of thread `id`, to every Bench. */
	tell(id: string, event: ThreadEvent): void {
		this.publish({ type: "thread", thread: id, event });
	}

	/** What a run did, to the listeners in this process. */
	report(id: string, e: RunEvent): void {
		for (const l of this.runs) l(id, e);
	}

	/** The list of threads changed (made, renamed, deleted, a run started or ended). */
	listChanged(): void {
		this.publish({ type: "threads_changed" });
	}

	async open(): Promise<void> {
		fs.mkdirSync(AGENT_HOME, { recursive: true });
		const models = this.models;
		const registry = (this.registry = createRegistry());
		setHost(hostOf(this));
		await loadExtensions(registry, () => this.reconfigureAll());
		this.storage = await openNodeSqliteStorage(path.join(AGENT_HOME, "pi.sqlite"));
		this.harness = await Harness.open(this.storage, {
			models: this.models.pi,
			registry,
			// Pi's coding tools work in the conversation's directory — on its project's machine, when
			// that is another one (`machine/ssh.ts`).
			env: ({ conversationId, cwd }: any) => envFor(String(conversationId), cwd),
			identity: { attribution: { name: "bench-agent", url: "https://github.com/krutovvv/bench" } },
			settings: {
				// Six tries back off over about a minute: long enough to ride out a busy model (503
				// model_busy when a bot and a thread write at once) and a restart of the agent.
				retry: { maxRetries: Number(readConfig("default")?.agent?.api_max_retries ?? 6) },
				toolExecution: "parallel",
				// Each project's compaction (its Model section), read anew at every use: a change
				// holds from the next step, and a new model's window reshapes it.
				get compaction() {
					return { modelOverrides: models.compactionOverrides() };
				},
				// No `afterFailedRun`: after a run the model failed, the queue behind it waits for the
				// person's next message. "continue" fed every waiting message to a model that could not
				// be reached, and a whole queue was spent on "Connection error." in a minute.
			},
			onReport: (error: unknown, info: any) => console.error("[pi]", info?.type, String((error as Error)?.message ?? error).slice(0, 300)),
		} as any, ctx);
		// Every model a stored session uses must be known before Pi resumes unfinished runs.
		for (const row of this.store.list({ limit: 10000 })) {
			try {
				this.models.ref(row.agent);
			} catch {}
		}
		this.journal.watch();
		this.harness.resume();
		// Runs a previous process left unfinished continue now: watch their sessions so their
		// answers reach the index and Bench sees them as running.
		const graph = await this.harness.taskGraph(ctx);
		const conversations = new Set(Object.values((graph.value as any).tasks ?? {}).map((t: any) => String(t.conversationId)));
		graph.dispose?.();
		for (const c of conversations) {
			const row = this.store.byConversation(c);
			if (row) await this.thread(row.id).catch(() => undefined);
		}
		// Scheduled jobs: their sleeping tasks live in one hidden conversation.
		setScheduler({
			conversation: async () => {
				const id = getSetting<number>("scheduler_conversation");
				const found = id !== undefined ? await this.harness.conversation(id as ConversationId, ctx) : undefined;
				if (found) return found;
				const created = await this.harness.createConversation({ ownership: { kind: "ownerless" as const } }, ctx);
				setSetting("scheduler_conversation", created.id);
				return created;
			},
			abortTask: async (id) => void (await this.harness.abortTask(Number(id) as TaskId, ctx)),
		});
		for (const job of jobs.list()) if (job.enabled && !job.task_id) await arm(job).catch((e) => console.error("[cron]", job.name, e));
	}

	/** After an extension was reloaded or MCP reconnected: new turns use the new code. */
	reloadMcp(agent?: string): void {
		for (const a of agent ? [agent] : agents()) installMcp(this.registry, a);
		this.reconfigureAll();
	}

	private reconfigureAll(): void {
		for (const live of this.live.values()) void live.conversation.configure({ extensions: forAgent(readAgent(live.row.agent).extensions, live.row.agent) }, ctx).catch(() => {});
	}

	/** The agent's MCP servers, connected before its first turn. */
	private async mcpFor(agent: string): Promise<void> {
		// A project's threads use the general servers too.
		for (const a of agent.startsWith("project:") ? ["default", agent] : [agent])
			if (await ensureMcp(a).catch(() => false)) installMcp(this.registry, a);
	}

	// ---- threads ------------------------------------------------------------

	async create(agent: string, cwd: string | undefined, source = "local", extra: Partial<ThreadRow> = {}): Promise<Live> {
		await this.mcpFor(agent);
		// A session with a project's environment (a thread, a cron run) belongs to the project and
		// works in its folder.
		const project = agent.startsWith("project:") ? agent.slice(8) : "";
		if (project && !extra.project) extra = { ...extra, project };
		const machine = project ? projects.get(project)?.machine : undefined;
		const dir = machine ? (cwd ?? projects.dir(project)) : cwd && fs.existsSync(cwd) ? cwd : project ? projects.dir(project) : defaultCwd(agent);
		const conversation = await this.harness.createConversation(
			{ ownership: { kind: "ownerless" }, agent: { model: this.models.ref(agent), thinkingLevel: thinking(reasoningEffort(agent)), cwd: dir, extensions: forAgent(readAgent(agent).extensions, agent) } },
			ctx,
		);
		const row = this.store.create({ id: newId(), conversation: String(conversation.id), agent, source, cwd: dir, effort: reasoningEffort(agent), ...extra });
		return this.attach(row, conversation);
	}

	/** The open session for `id`, attaching to its conversation on first use. */
	async thread(id: string): Promise<Live | undefined> {
		const open = this.live.get(id);
		if (open) return open;
		const row = this.store.get(id);
		if (!row) return undefined;
		const conversation = await this.harness.conversation(Number(row.conversation) as ConversationId, ctx);
		if (!conversation) return undefined;
		return this.attach(row, conversation);
	}

	private async attach(row: ThreadRow, conversation: Conversation): Promise<Live> {
		const hold = await this.harness.snapshot(HoldDoc, conversation.id, ctx);
		const live: Live = {
			row,
			conversation,
			busy: false,
			holds: hold?.holds ?? [],
			generation: 0,
			blocks: new Map(),
			answer: "",
			prompts: new Map(),
			// The counters survive in the row: reopening the thread continues the same numbers
			// rather than starting from zero (a fresh turn re-reads the window anyway).
			usage: { ...totalsOf(row.usage), started: Date.now() },
			inserts: [],
		};
		this.live.set(row.id, live);
		this.byConversation.set(String(conversation.id), live);
		const events = await watchEvents(this.harness, conversation.id, ctx);
		live.busy = events.snapshot.run !== undefined;
		events.start(async (list) => {
			for (const e of list) this.stream.onEvent(live, e);
		});
		live.stop = () => events.stop();
		await this.inserts.list(row.id).catch(() => {});
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

	/** The person changed the thread's reasoning effort. */
	setEffort(id: string, effort: string): void {
		this.store.update(id, { effort });
		const live = this.live.get(id);
		if (!live) return;
		live.row = this.store.get(id)!;
		this.tell(id, { kind: "info", value: this.models.info(live.row) });
	}

	isBusy(id: string): boolean {
		return this.live.get(id)?.busy ?? false;
	}

	// ---- turns ------------------------------------------------------------------

	/**
	 * A message from the person. While the agent works it waits in Pi's inbox (`mode` "queue",
	 * sent when the agent answers) or joins the running work after its current tools ("steer").
	 */
	async submit(id: string, text: string, attachments: { path: string; mime: string }[] = [], context = "", mode: "queue" | "steer" = "steer", request: string = randomUUID()): Promise<void> {
		const live = await this.thread(id);
		if (!live) throw new Error(`thread not found: ${id}`);
		const row = this.store.get(id)!;
		live.row = row;
		await this.mcpFor(row.agent);
		// The model (one for every thread, the gear's), effort and extensions follow at every turn.
		await live.conversation.configure({ model: this.models.ref(row.agent), thinkingLevel: thinking(row.effort || reasoningEffort(row.agent)), cwd: row.cwd, extensions: forAgent(readAgent(row.agent).extensions, row.agent) }, ctx);
		const images = attachments
			.filter((a) => a.mime.startsWith("image/") && fs.existsSync(a.path))
			.map((a) => ({ type: "image" as const, data: fs.readFileSync(a.path).toString("base64"), mimeType: a.mime }));
		// What the thread's live block shows, for the model only (Bench's history drops it).
		const said = context.trim() ? `${text}\n\n${BLOCK_OPEN}\n${context.trim()}\n${BLOCK_CLOSE}` : text;
		const content: any = images.length ? [{ type: "text", text: said }, ...images] : said;
		if (live.busy) {
			// Queued: Pi keeps it in the inbox until the run answers. Steered: placed after the
			// current tool round. `request` is Bench's id for the message (docs/thread-objects.md):
			// Pi keeps it with the submission, so its row in the queue and its entry are known as
			// the one pressed.
			await live.conversation.submit({ type: "input", content, whenBusy: mode === "queue" ? "followUp" : "steer", requestId: request }, ctx);
			return;
		}
		live.busy = true;
		live.generation++;
		await this.holds.clear(live);
		live.abort = new AbortController();
		// The run is live from here: Pi's own run_start comes later and finds the thread busy.
		this.stream.beginRun(live);
		await live.conversation.submit({ type: "input", content, requestId: request }, ctx);
		if (!row.title) void this.titles.name(live, text);
	}

	async interrupt(id: string): Promise<void> {
		const live = await this.thread(id);
		if (!live) return;
		live.abort?.abort();
		await this.holds.stop(live);
		this.asks.declineAll(live);
		await live.conversation.abort(ctx);
	}

	/**
	 * A new thread that goes on from entry `at` of this one: Pi's fork, its history through that
	 * entry, the documents as their semantics say (the pause starts free). The thread it came
	 * from is untouched. Its first message is the person's next one.
	 */
	async fork(id: string, at: number): Promise<ThreadRow> {
		const live = await this.thread(id);
		if (!live) throw new Error(`thread not found: ${id}`);
		const fork = await live.conversation.fork(at as any, { ownership: { kind: "ownerless" as const } }, ctx);
		const row = live.row;
		const made = this.store.create({
			id: newId(),
			conversation: String(fork.id),
			agent: row.agent,
			source: "local",
			cwd: row.cwd,
			effort: row.effort,
			title: row.title ? `${row.title} (fork)` : "",
			...(row.project ? { project: row.project } : {}),
		});
		this.store.copyIndex(row.id, made.id, at);
		await this.attach(made, fork);
		return made;
	}

	/** Regenerate: drop the last exchange (a fork before the last user message) so the next prompt replaces it. */
	async undo(id: string): Promise<void> {
		const live = await this.thread(id);
		if (!live) return;
		const view = await live.conversation.viewState(ctx);
		const entries = (view.value as any).entries as any[];
		view.dispose();
		let last = -1;
		for (let i = entries.length - 1; i >= 0; i--) if (entries[i].kind === "pi.user") { last = i; break; }
		if (last <= 0) return;
		const at = entries[last - 1].id;
		const fork = await live.conversation.fork(at, { ownership: { kind: "ownerless" as const } }, ctx);
		this.store.cutIndex(id, Number(at));
		await this.close(id);
		this.store.update(id, { conversation: String(fork.id) });
	}
}
