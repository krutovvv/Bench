// Telegram, on grammY. Each chat is a session (source "telegram"; Bench lists it under Telegram).
// One turn = two messages, as in Bench's folded steps:
//   1. a silent status message — "Working…" plus the last few steps, edited as they happen, and
//      "Done · 6 steps · 41 s" at the end;
//   2. the answer, as a new message, so the notification comes when the work is done.
// Questions (clarify) and approvals are buttons; voice is transcribed with the stt settings;
// photos go to the model; MEDIA:/path files are sent as attachments; /new and /stop.
//
// Bots: the general one ("telegram" in bench.sqlite { token, allowed_users: [ids], agent }) and
// one per project that has its own (`projects.telegram`): a project's bot answers with the
// project's environment and its chats are the project's threads. `syncTelegram` starts and stops
// them as their settings change.

import { Bot, InputFile, type Context as TgContext } from "grammy";
import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { transcribe } from "./audio.ts";
import { approvalsMode, BENCH_HOME, getSetting, homeAgent, agentEnv, reasoningEffort, resolveModel } from "./config.ts";
import { HttpsProxyAgent } from "https-proxy-agent";
import { projects } from "./builtin/projects.ts";
import type { RunListener, Runtime } from "./runtime.ts";

const MAX = 4096;
const EDIT_EVERY_MS = 1500;
/** Messages that come in a run (several written one after another, a forwarded batch) are one
 * turn: it starts once the chat is quiet this long after the last of them is ready (a voice
 * message once it is transcribed), or longer while the batch is only forwarded messages (the
 * instruction usually follows). A new message always takes over: the running turn is stopped
 * at once, and the next one gets everything sent since. */
const GATHER_MS = 2000;
const GATHER_FORWARDED_MS = 6000;

/** The messages for a chat's next turn, in the order they came: a voice message or a file holds
 * its place while it is transcribed or downloaded. */
export class Batch {
	parts: (string | undefined)[] = [];
	attachments: { path: string; mime: string }[] = [];
	pending = 0;
	last = 0;
	onlyForwarded = true;
	timer?: NodeJS.Timeout;
	name: string;
	constructor(name: string) {
		this.name = name;
	}
	reserve(message: number, forwarded: boolean): number {
		this.pending++;
		this.last = message;
		this.onlyForwarded &&= forwarded;
		return this.parts.push(undefined) - 1;
	}
	fill(slot: number, part: string | undefined): void {
		this.parts[slot] = part;
		this.pending--;
	}
	get ready(): boolean {
		return this.pending === 0;
	}
	text(): string {
		return this.parts.filter((p): p is string => !!p).join("\n\n");
	}
}

interface Config {
	token: string;
	allowed_users: string[];
	/** Proxy URL (http://host:port) for this bot; beats the agent's TELEGRAM_PROXY. */
	proxy?: string;
	agent?: string;
}

interface Turn {
	chat: number;
	/** The person's message it answers: the status and the answer are replies to it. */
	replyTo?: number;
	status?: number;
	steps: number;
	/** The answer as it is being written: its last line is what the status says — the model's
	 * own words read better in one line than a list of tool calls, and stay until new words. */
	text?: string;
	/** What the newest call is doing, in one word — until the model writes something. */
	word?: string;
	started: number;
	dirty: boolean;
	lastEdit: number;
	/** Stopped by a newer message: its answer is not sent. */
	superseded?: boolean;
	timer?: NodeJS.Timeout;
	typing?: NodeJS.Timeout;
}

/** The status's one live line from the streamed answer: its last written line, bare. */
function liveLine(text: string | undefined): string {
	if (!text) return "";
	const lines = text.split("\n").map((l) => l.replace(/^[#>\-*+\d.\s]+/, "").trim()).filter(Boolean);
	return (lines[lines.length - 1] ?? "").slice(0, 120);
}

/** One word for what a kind of tool is doing, for the status line. */
function activityWord(name: string): string {
	if (name.startsWith("mcp__browser")) return "browsing…";
	const base = name.startsWith("mcp__") ? name.split("__").slice(2).join(" ").replace(/_/g, " ") : name.replace(/_/g, " ");
	const words: Record<string, string> = {
		bash: "running a command…", ssh_run: "running a command…",
		read: "reading…", edit: "editing…", write: "editing…", search_replace: "editing…",
		grep: "searching…", glob: "searching…", ls: "searching…", session_search: "searching…", tool_search: "searching…",
		web_fetch: "checking the web…", web_search: "checking the web…",
		delegate: "delegating…", todo: "planning…", memory: "taking notes…", tasks: "the task list…",
		cron: "the schedule…", clarify: "asking you…", skill_view: "a skill…", skill_manage: "a skill…",
	};
	return words[name] ?? `using ${base}…`;
}

/** The bots running: "" the general one, else a project's id → its settings and how to stop it. */
const running = new Map<string, { config: string; stop: () => void; send: (chat: number, text: string) => Promise<void> }>();

/** How each bot is reaching Telegram now, for the person to see in Settings → Telegram:
 * "connecting" from the start, "live" once getMe answered (a token and a proxy that work),
 * "error" with the reason when it could not start or polling fell over. Saved bots that never
 * answer read as a long "connecting": a dead proxy says nothing until it times out. */
export const telegramStatus = new Map<string, { state: "connecting" | "live" | "error"; detail?: string; at: number }>();

function noteStatus(key: string, state: "connecting" | "live" | "error", detail?: string): void {
	telegramStatus.set(key, { state, ...(detail ? { detail } : {}), at: Date.now() });
}

/** Forget a bot so the next `syncTelegram` starts it anew (Settings → Telegram → Check). */
export function reconnectTelegram(key: string): void {
	running.get(key)?.stop();
	running.delete(key);
	telegramStatus.delete(key);
}


/** Start the bots whose settings exist, stop the ones whose settings went or changed. */
export async function syncTelegram(rt: Runtime): Promise<void> {
	const wanted = new Map<string, Config>();
	const general: Config | undefined = getSetting("telegram");
	if (general?.token) wanted.set("", general);
	for (const p of projects.list()) if (p.telegram?.token) wanted.set(p.id, { ...p.telegram, agent: `project:${p.id}` });
	for (const [key, bot] of running)
		if (JSON.stringify(wanted.get(key) ?? null) !== bot.config) {
			bot.stop();
			running.delete(key);
			telegramStatus.delete(key);
		}
	for (const [key, cfg] of wanted)
		if (!running.has(key)) {
			noteStatus(key, "connecting");
			await startBot(rt, cfg, key).catch((e) => {
				console.error(`[telegram] ${key || "general"}: ${(e as Error).message}`);
				noteStatus(key, "error", (e as Error).message ?? String(e));
			});
		}
	// Scheduled answers to a chat ("<project>/<chat>" or "<chat>"): through that project's bot, else
	// the general one, else the first that runs.
	rt.deliverers.set("telegram", async (to, text) => {
		const [project, chat] = to.includes("/") ? to.split("/", 2) : ["", to];
		const bot = running.get(project) ?? running.get("") ?? [...running.values()][0];
		if (!bot) throw new Error("no Telegram bot runs");
		await bot.send(Number(chat), text);
	});
}

async function startBot(rt: Runtime, cfg: Config, project: string): Promise<void> {
	// The general bot's settings were first written with `profile`.
	const agent = cfg.agent || (cfg as { profile?: string }).profile || "default";
	const allowed = new Set(cfg.allowed_users.map(String));
	// Where Telegram is reached only through a proxy: the bot's own URL when it has one, else
	// the agent's env (TELEGRAM_PROXY, else HTTPS_PROXY).
	const env = agentEnv("default");
	const proxy = cfg.proxy || env.TELEGRAM_PROXY || env.HTTPS_PROXY || env.https_proxy;
	const bot = new Bot(cfg.token, proxy ? { client: { baseFetchConfig: { agent: new HttpsProxyAgent(proxy), compress: true } } } : undefined);
	const api = bot.api;

	const turns = new Map<string, Turn>(); // thread id -> its running turn
	const replies = new Map<string, number>(); // thread id -> the message its next turn answers
	const gathering = new Map<number, Batch>(); // chat -> the messages gathered for its next turn
	const prompts = new Map<string, { thread: string; promptId: string; kind: string; choices: string[] }>(); // "chat:message"

	// This bot's chats only: a chat with another bot is another thread.
	const ours = (s: { project?: string }) => (s.project ?? "") === project;
	const chatOf = (threadId: string): number | undefined => {
		const row = rt.store.get(threadId);
		const id = row?.chat_id;
		return row && ours(row) && row.source === "telegram" && id && !id.includes(":") ? Number(id) : undefined;
	};
	const threadFor = async (chat: number, name: string): Promise<string> => {
		const row = rt.store.list({ source: "telegram", limit: 1000 }).find((s) => s.chat_id === String(chat) && ours(s));
		if (row) return row.id;
		return (await rt.create(agent, undefined, "telegram", { chat_id: String(chat), display_name: name } as any)).row.id;
	};

	const statusText = (t: Turn, done?: string) => {
		const secs = Math.round((Date.now() - t.started) / 1000);
		if (done) return `${done} · ${t.steps} ${t.steps === 1 ? "step" : "steps"} · ${secs} s`;
		const line = liveLine(t.text) || t.word || "";
		return line ? `⏳ ${line} · ${secs} s` : `⏳ Working… ${secs} s`;
	};
	const editStatus = async (t: Turn, done?: string) => {
		t.dirty = false;
		t.lastEdit = Date.now();
		const text = statusText(t, done).slice(0, MAX);
		try {
			if (t.status) await api.editMessageText(t.chat, t.status, text);
			else t.status = (await api.sendMessage(t.chat, text, { disable_notification: true, ...replyOpts(t.replyTo) })).message_id;
		} catch {}
	};
	const schedule = (t: Turn) => {
		t.dirty = true;
		if (t.timer) return;
		t.timer = setTimeout(() => {
			t.timer = undefined;
			if (t.dirty) void editStatus(t);
		}, Math.max(0, EDIT_EVERY_MS - (Date.now() - t.lastEdit)));
	};

	const sendAnswer = async (chat: number, final: string, replyTo?: number) => {
		const { text, files } = extractMedia(final);
		for (const chunk of split(text.trim())) {
			if (!chunk) continue;
			try {
				await api.sendMessage(chat, toHtml(chunk), { parse_mode: "HTML", link_preview_options: { is_disabled: true }, ...replyOpts(replyTo) });
			} catch {
				await api.sendMessage(chat, chunk, replyOpts(replyTo)).catch(() => {});
			}
		}
		for (const f of files) await sendFile(chat, f).catch((e) => api.sendMessage(chat, `Could not send ${path.basename(f)}: ${e.message}`).catch(() => {}));
		if (!text.trim() && !files.length) await api.sendMessage(chat, "(no answer)").catch(() => {});
	};

	const sendFile = async (chat: number, file: string) => {
		const ext = path.extname(file).toLowerCase();
		const input = new InputFile(file);
		if ([".png", ".jpg", ".jpeg", ".webp"].includes(ext)) await api.sendPhoto(chat, input);
		else if ([".mp4", ".mov"].includes(ext)) await api.sendVideo(chat, input);
		else if ([".mp3", ".m4a"].includes(ext)) await api.sendAudio(chat, input);
		else if ([".ogg", ".opus"].includes(ext)) await api.sendVoice(chat, input);
		else await api.sendDocument(chat, input);
	};


	const listener: RunListener = (threadId, e) => {
		const chat = chatOf(threadId);
		if (chat === undefined) return;
		if (e.kind === "start") {
			const replyTo = replies.get(threadId);
			replies.delete(threadId);
			const t: Turn = { chat, replyTo, steps: 0, text: "", started: Date.now(), dirty: false, lastEdit: 0 };
			turns.set(threadId, t);
			void editStatus(t);
			api.sendChatAction(chat, "typing").catch(() => {});
			t.typing = setInterval(() => api.sendChatAction(chat, "typing").catch(() => {}), 4500);
		} else if (e.kind === "delta") {
			const t = turns.get(threadId);
			if (!t) return;
			t.text = (t.text ?? "") + e.text;
			// Words win the line over the newest call's word.
			t.word = undefined;
			schedule(t);
		} else if (e.kind === "tool") {
			const t = turns.get(threadId);
			if (!t) return;
			// Counted, not listed: the status line says the kind of the newest call, in a word.
			t.steps++;
			t.word = activityWord(e.name);
			schedule(t);
		} else if (e.kind === "subagent") {
			const t = turns.get(threadId);
			if (!t) return;
			t.steps++;
			// A delegation says what it delegated — it is a readable line in its own right.
			t.text = `👥 ${e.goal.slice(0, 110)}`;
			schedule(t);
		} else if (e.kind === "complete") {
			const t = turns.get(threadId);
			turns.delete(threadId);
			if (t) {
				clearTimeout(t.timer);
				clearInterval(t.typing);
				void editStatus(t, t.superseded ? "⏹ Stopped for your new message" : "✅ Done");
			}
			if (!t?.superseded) void sendAnswer(chat, e.text, t?.replyTo);
		} else if (e.kind === "ask") {
			const keyboard = e.ask.choices.map((c, i) => [{ text: c, callback_data: `p:${i}` }]);
			api.sendMessage(chat, e.question, keyboard.length ? { reply_markup: { inline_keyboard: keyboard } } : {})
				.then((m) => prompts.set(`${chat}:${m.message_id}`, { thread: threadId, promptId: e.ask.id, kind: e.ask.kind, choices: e.ask.choices }))
				.catch(() => {});
		}
	};
	rt.runs.add(listener);

	// Files (photos, voice) come the way the bot's messages do: through the proxy where Telegram
	// is reached only that way.
	const download = async (fileId: string): Promise<{ data: Buffer; name: string }> => {
		const f = await api.getFile(fileId);
		const data = await new Promise<Buffer>((resolve, reject) => {
			https
				.get(`https://api.telegram.org/file/bot${cfg.token}/${f.file_path}`, proxy ? { agent: new HttpsProxyAgent(proxy) } : {}, (r) => {
					if (r.statusCode !== 200) return reject(new Error(`download: HTTP ${r.statusCode}`));
					const chunks: Buffer[] = [];
					r.on("data", (c) => chunks.push(c));
					r.on("end", () => resolve(Buffer.concat(chunks)));
				})
				.on("error", reject);
		});
		return { data, name: path.basename(f.file_path ?? "file") };
	};

	bot.use(async (ctx, next) => {
		if (allowed.has(String(ctx.from?.id ?? ""))) return next();
		if (ctx.chat) await ctx.reply(`Not authorized. Your id: ${ctx.from?.id}`).catch(() => {});
	});

	bot.command("start", (ctx) => ctx.reply("Hi! Write or send a voice message."));
	bot.command("new", async (ctx) => {
		const chat = ctx.chat.id;
		const old = rt.store.list({ source: "telegram", limit: 1000 }).find((s) => s.chat_id === String(chat) && ours(s));
		if (old) rt.store.update(old.id, { chat_id: `${chat}:${Date.now()}` } as any);
		const id = await threadFor(chat, nameOf(ctx));
		await ctx.reply(threadCard(id), { parse_mode: "HTML", ...replyOpts(ctx.message?.message_id) });
	});
	bot.command("stop", async (ctx) => {
		await rt.interrupt(await threadFor(ctx.chat.id, nameOf(ctx)));
		await ctx.reply("Stopped.");
	});

	// A message is taken in at once (grammY handles updates one by one, so a voice message being
	// transcribed would hold up the ones after it): it stops the running turn, takes its place in
	// the chat's batch, and is filled in when its voice or file is ready.
	bot.on("message", async (ctx) => {
		const m = ctx.message;
		const chat = ctx.chat.id;
		const thread = await threadFor(chat, nameOf(ctx));
		const typed = m.text ?? m.caption ?? "";
		// A typed answer to a pending question.
		const pending = [...prompts.entries()].find(([k, p]) => k.startsWith(`${chat}:`) && p.thread === thread && p.kind === "clarify");
		if (pending && typed) {
			prompts.delete(pending[0]);
			rt.asks.respond(thread, pending[1].promptId, typed);
			return;
		}
		const origin = forwardedFrom(m);
		const g = gathering.get(chat) ?? new Batch(nameOf(ctx));
		gathering.set(chat, g);
		clearTimeout(g.timer);
		const slot = g.reserve(m.message_id, !!origin);
		api.sendChatAction(chat, "typing").catch(() => {});
		// What was sent last matters more than what the agent is doing: stop it now.
		if (rt.isBusy(thread)) {
			const t = turns.get(thread);
			if (t) t.superseded = true;
			void rt.interrupt(thread).catch(() => {});
		}
		void (async () => {
			let part: string | undefined;
			try {
				part = await partOf(m, typed, g);
				// Forwarded messages say whose they were.
				if (part && origin) part = `[forwarded from ${origin}]\n${part}`;
			} catch (e) {
				await ctx.reply(String((e as Error).message), replyOpts(m.message_id)).catch(() => {});
			}
			g.fill(slot, part);
			if (gathering.get(chat) === g && g.ready) {
				clearTimeout(g.timer);
				g.timer = setTimeout(() => void flush(chat), g.onlyForwarded ? GATHER_FORWARDED_MS : GATHER_MS);
			}
		})();
	});

	/** A message as text for the agent: typed, transcribed, or a file saved and named. */
	const partOf = async (m: any, typed: string, g: Batch): Promise<string | undefined> => {
		let text = typed;
		const voice = m.voice ?? m.audio;
		if (voice) {
			const { data, name } = await download(voice.file_id);
			try {
				text = await transcribe(agent, data, voice.mime_type ?? "audio/ogg", name.replace(/\.oga$/, ".ogg"));
			} catch (e) {
				throw new Error(`Could not transcribe: ${(e as Error).message}`);
			}
			if (!text) throw new Error("(nothing recognized)");
		}
		const photo = m.photo?.at(-1);
		const doc = m.document;
		if (photo || doc) {
			const { data, name } = await download((photo ?? doc)!.file_id);
			const dir = path.join(BENCH_HOME, "attachments");
			fs.mkdirSync(dir, { recursive: true });
			const p = path.join(dir, `${Date.now()}-${doc?.file_name ?? name}`);
			fs.writeFileSync(p, data);
			const mime = photo ? "image/jpeg" : (doc?.mime_type ?? "application/octet-stream");
			if (mime.startsWith("image/")) g.attachments.push({ path: p, mime });
			text = `${text}\n\n[attached: ${p}]`.trim();
		}
		return text || undefined;
	};

	// The gathered messages as one new turn, answering the last of them. A turn stopped for them
	// is let finish its stop first, so they start a turn of their own rather than join it.
	const flush = async (chat: number) => {
		const g = gathering.get(chat);
		if (!g?.ready) return;
		gathering.delete(chat);
		const text = g.text();
		if (!text) return;
		const thread = await threadFor(chat, g.name);
		for (let i = 0; i < 100 && rt.isBusy(thread); i++) await new Promise((r) => setTimeout(r, 100));
		replies.set(thread, g.last);
		await rt.submit(thread, text, g.attachments).catch((e) => api.sendMessage(chat, `Could not send: ${(e as Error).message}`).catch(() => {}));
	};

	/** What a thread runs with, as /new shows it. */
	const threadCard = (id: string): string => {
		const row = rt.store.get(id);
		const runs = row?.agent ?? agent;
		const m = resolveModel();
		const name = project ? (projects.get(project)?.name ?? project) : "Host";
		// The exact number, with the familiar short form beside it: 131072 tokens is the same
		// window people call 128K, and showing one of the two alone reads as a different setting.
		const ctx = `${m.contextLength.toLocaleString("en-US")} tokens (${Math.round(m.contextLength / 1024)}K)`;
		return [
			`✨ <b>New thread</b> · ${esc(name)}`,
			"",
			`◆ Model: <code>${esc(m.model)}</code>`,
			`◆ Provider: ${esc(m.provider)}`,
			`◆ Context: ${esc(ctx)}`,
			`◆ Thinking: ${esc(row?.effort || reasoningEffort(homeAgent(runs)))}`,
			`◆ Approvals: ${approvalsMode(homeAgent(runs)) === "manual" ? "asks before risky commands" : "off"}`,
			"",
			"Messages sent in a row (or forwarded together) are read as one. A new message stops what I'm doing, and I start over with everything you sent. /stop just stops.",
		].join("\n");
	};

	bot.on("callback_query:data", async (ctx) => {
		const msg = ctx.callbackQuery.message;
		const key = `${msg?.chat.id}:${msg?.message_id}`;
		const p = prompts.get(key);
		await ctx.answerCallbackQuery().catch(() => {});
		if (!p || !msg) return;
		const choice = p.choices[Number(ctx.callbackQuery.data.slice(2))] ?? "";
		prompts.delete(key);
		rt.asks.respond(p.thread, p.promptId, choice);
		await ctx.editMessageText(`${(msg as any).text ?? ""}\n\n→ ${choice.replace(/\s*\(Recommended\)$/, "")}`).catch(() => {});
	});

	// A failure handling a message is said in the chat, not only in the log.
	bot.catch((e) => {
		console.error("[telegram]", e.message);
		(e as any).ctx?.reply(`Could not handle that: ${String((e as any).error?.message ?? e.message)}`).catch(() => {});
	});
	await bot.api.setMyCommands([
		{ command: "new", description: "Start a new thread" },
		{ command: "stop", description: "Stop the running turn" },
	]).catch(() => {});
	const me = await bot.api.getMe();
	noteStatus(project, "live", me.username ? `@${me.username}` : "");
	console.log(`[telegram] @${me.username}${project ? ` (project ${project})` : ""}`);
	// Stopping a bot (its settings changed) rejects its start: that is not a crash of the server.
	let stopping = false;
	bot.start({ allowed_updates: ["message", "callback_query"] }).catch((e) => {
		console.error(`[telegram] @${me.username} stopped: ${(e as Error)?.message ?? e}`);
		if (!stopping) noteStatus(project, "error", (e as Error)?.message ?? String(e));
	});
	running.set(project, {
		config: JSON.stringify(cfg),
		stop: () => {
			stopping = true;
			rt.runs.delete(listener);
			void bot.stop().catch(() => {});
		},
		send: (chat, text) => sendAnswer(chat, text),
	});
	if (project) projects.setBotName(project, me.username ?? "");
}

/** Reply to the person's message (and still send when it is gone). */
const replyOpts = (id?: number) => (id ? { reply_parameters: { message_id: id, allow_sending_without_reply: true } } : {});

/** Whose a forwarded message was: a person, a hidden one, a chat or a channel. */
function forwardedFrom(m: any): string | undefined {
	const o = m.forward_origin;
	if (!o) return undefined;
	if (o.type === "user") return [o.sender_user?.first_name, o.sender_user?.last_name].filter(Boolean).join(" ") || o.sender_user?.username || "someone";
	if (o.type === "hidden_user") return o.sender_user_name || "someone";
	return o.chat?.title || o.author_signature || "a chat";
}

const nameOf = (ctx: TgContext) => [ctx.from?.first_name, ctx.from?.last_name].filter(Boolean).join(" ") || ctx.from?.username || String(ctx.from?.id ?? "");

/** `MEDIA:/absolute/path` lines become attachments; the rest is the text. */
export function extractMedia(text: string): { text: string; files: string[] } {
	const files: string[] = [];
	const out = text
		.replace(/\[\[audio_as_voice\]\]/g, "")
		.replace(/`?MEDIA:(\/[^\s`]+)`?/g, (_m, p: string) => {
			if (fs.existsSync(p)) files.push(p);
			return fs.existsSync(p) ? "" : p;
		});
	return { text: out.replace(/\n{3,}/g, "\n\n"), files };
}

/** Splits at paragraph or line ends so each piece fits Telegram's 4096 (before markup). */
export function split(text: string, max = 3800): string[] {
	const out: string[] = [];
	let rest = text;
	while (rest.length > max) {
		let cut = rest.lastIndexOf("\n\n", max);
		if (cut < max / 2) cut = rest.lastIndexOf("\n", max);
		if (cut < max / 2) cut = max;
		out.push(rest.slice(0, cut));
		rest = rest.slice(cut).replace(/^\n+/, "");
	}
	out.push(rest);
	return out;
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Markdown as the agent writes it → Telegram HTML (bold, italic, strike, spoiler, code, links, headers, quotes; tables as bullets). */
export function toHtml(md: string): string {
	const blocks: string[] = [];
	let text = md.replace(/```(\w*)\n?([\s\S]*?)```/g, (_m, lang: string, code: string) => {
		blocks.push(lang ? `<pre><code class="language-${esc(lang)}">${esc(code.replace(/\n$/, ""))}</code></pre>` : `<pre>${esc(code.replace(/\n$/, ""))}</pre>`);
		return `\u0000${blocks.length - 1}\u0000`;
	});
	text = tablesToBullets(text);
	const inline: string[] = [];
	text = text.replace(/`([^`\n]+)`/g, (_m, c: string) => {
		inline.push(`<code>${esc(c)}</code>`);
		return `\u0001${inline.length - 1}\u0001`;
	});
	text = esc(text)
		.replace(/^#{1,6}\s+(.+)$/gm, "<b>$1</b>")
		.replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2">$1</a>')
		.replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>")
		.replace(/__([^_\n]+)__/g, "<b>$1</b>")
		.replace(/(^|[^*\w])\*([^*\n]+)\*(?=[^*\w]|$)/g, "$1<i>$2</i>")
		.replace(/(^|[^_\w])_([^_\n]+)_(?=[^_\w]|$)/g, "$1<i>$2</i>")
		.replace(/~~([^~\n]+)~~/g, "<s>$1</s>")
		.replace(/\|\|([^|\n]+)\|\|/g, "<tg-spoiler>$1</tg-spoiler>")
		.replace(/^(?:&gt;\s?(.*)(?:\n|$))+/gm, (block) => `<blockquote>${block.replace(/^&gt;\s?/gm, "").trim()}</blockquote>\n`)
		.replace(/^(\s*)[-*]\s+/gm, "$1• ");
	return text.replace(/\u0001(\d+)\u0001/g, (_m, i) => inline[Number(i)]).replace(/\u0000(\d+)\u0000/g, (_m, i) => blocks[Number(i)]).trim();
}

/** A pipe table becomes one bullet group per row ("header: value"). */
function tablesToBullets(text: string): string {
	return text.replace(/((?:^\|.*\|[ \t]*\n?)+)/gm, (table) => {
		const rows = table.trim().split("\n").map((r) => r.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim()));
		if (rows.length < 2 || !rows[1].every((c) => /^:?-{2,}:?$/.test(c))) return table;
		const head = rows[0];
		return rows.slice(2).map((r) => r.map((c, i) => `${i === 0 ? "- " : "  "}${head[i] ? `**${head[i]}:** ` : ""}${c}`).join("\n")).join("\n\n") + "\n";
	});
}
