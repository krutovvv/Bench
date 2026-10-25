// Telegram, on grammY. Each chat is a session (source "telegram"; Bench lists it under Telegram).
// One turn = two messages, as in Bench's folded steps:
//   1. a silent status message — "Working…" plus the last few steps, edited as they happen, and
//      "Done · 6 steps · 41 s" at the end;
//   2. the answer, as a new message, so the notification comes when the work is done.
// Questions (clarify) and approvals are buttons; voice is transcribed with the stt settings;
// photos go to the model; MEDIA:/path files are sent as attachments; /new and /stop.
//
// Bots: the general one ("telegram" in bench.sqlite { token, allowed_users: [ids], profile }) and
// one per project that has its own (`projects.telegram`): a project's bot answers with the
// project's environment and its chats are the project's threads. `syncTelegram` starts and stops
// them as their settings change.

import { Bot, InputFile, type Context as TgContext } from "grammy";
import fs from "node:fs";
import path from "node:path";
import { transcribe } from "./audio.ts";
import { BENCH_HOME, getSetting, profileEnv } from "./config.ts";
import { HttpsProxyAgent } from "https-proxy-agent";
import { projects } from "./builtin/projects.ts";
import type { Emit, Runtime } from "./runtime.ts";

const MAX = 4096;
const EDIT_EVERY_MS = 1500;
const SHOWN_STEPS = 6;

interface Config {
	token: string;
	allowed_users: string[];
	profile?: string;
}

interface Turn {
	chat: number;
	status?: number;
	steps: string[];
	started: number;
	dirty: boolean;
	lastEdit: number;
	timer?: NodeJS.Timeout;
	typing?: NodeJS.Timeout;
}

/** A tool step as one short line: an icon, the tool, its main argument. */
function stepLine(name: string, detail: string): string {
	const icon: Record<string, string> = { bash: "💻", read: "📄", write: "✏️", edit: "✏️", skill_view: "📖", skill_manage: "📖", memory: "🧠", clarify: "❓", todo: "📋", delegate: "👥", cron: "⏰", session_search: "🔎", tool_search: "🧰" };
	const label = name.startsWith("mcp__") ? name.split("__").slice(1).join(" ") : name;
	const short = detail.replace(/\s+/g, " ").trim().slice(0, 70);
	return `${icon[name] ?? (name.startsWith("mcp__") ? "🔌" : "🔧")} ${label}${short ? `: ${short}` : ""}`;
}

/** The bots running: "" the general one, else a project's id → its settings and how to stop it. */
const running = new Map<string, { config: string; stop: () => void; send: (chat: number, text: string) => Promise<void> }>();

/** Start the bots whose settings exist, stop the ones whose settings went or changed. */
export async function syncTelegram(rt: Runtime): Promise<void> {
	const wanted = new Map<string, Config>();
	const general: Config | undefined = getSetting("telegram");
	if (general?.token) wanted.set("", general);
	for (const p of projects.list()) if (p.telegram?.token) wanted.set(p.id, { ...p.telegram, profile: `project:${p.id}` });
	for (const [key, bot] of running)
		if (JSON.stringify(wanted.get(key) ?? null) !== bot.config) {
			bot.stop();
			running.delete(key);
		}
	for (const [key, cfg] of wanted)
		if (!running.has(key))
			await startBot(rt, cfg, key).catch((e) => console.error(`[telegram] ${key || "general"}: ${(e as Error).message}`));
	// Scheduled answers to a chat: through the general bot, else the first that runs.
	rt.deliverers.set("telegram", async (to, text) => {
		const bot = running.get("") ?? [...running.values()][0];
		if (!bot) throw new Error("no Telegram bot runs");
		await bot.send(Number(to), text);
	});
}

async function startBot(rt: Runtime, cfg: Config, project: string): Promise<void> {
	const profile = cfg.profile || "default";
	const allowed = new Set(cfg.allowed_users.map(String));
	// Where Telegram is reached only through a proxy (TELEGRAM_PROXY, else HTTPS_PROXY in the agent's env).
	const env = profileEnv("default");
	const proxy = env.TELEGRAM_PROXY || env.HTTPS_PROXY || env.https_proxy;
	const bot = new Bot(cfg.token, proxy ? { client: { baseFetchConfig: { agent: new HttpsProxyAgent(proxy), compress: true } } } : undefined);
	const api = bot.api;

	const turns = new Map<string, Turn>(); // session id -> its running turn
	const prompts = new Map<string, { session: string; promptId: string; kind: string; choices: string[] }>(); // "chat:message"

	// This bot's chats only: a chat with another bot is another thread.
	const ours = (s: { project?: string }) => (s.project ?? "") === project;
	const chatOf = (sessionId: string): number | undefined => {
		const row = rt.store.get(sessionId);
		const id = row?.chat_id;
		return row && ours(row) && row.source === "telegram" && id && !id.includes(":") ? Number(id) : undefined;
	};
	const sessionFor = async (chat: number, name: string): Promise<string> => {
		const row = rt.store.list({ source: "telegram", limit: 1000 }).find((s) => s.chat_id === String(chat) && ours(s));
		if (row) return row.id;
		return (await rt.create(profile, undefined, "telegram", { chat_id: String(chat), display_name: name } as any)).row.id;
	};

	const statusText = (t: Turn, done?: string) => {
		const secs = Math.round((Date.now() - t.started) / 1000);
		if (done) return `${done} · ${t.steps.length} ${t.steps.length === 1 ? "step" : "steps"} · ${secs} s`;
		const shown = t.steps.slice(-SHOWN_STEPS);
		const more = t.steps.length - shown.length;
		return [`⏳ Working… ${secs} s`, ...(more > 0 ? [`… ${more} more`] : []), ...shown].join("\n");
	};
	const editStatus = async (t: Turn, done?: string) => {
		t.dirty = false;
		t.lastEdit = Date.now();
		const text = statusText(t, done).slice(0, MAX);
		try {
			if (t.status) await api.editMessageText(t.chat, t.status, text);
			else t.status = (await api.sendMessage(t.chat, text, { disable_notification: true })).message_id;
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

	const sendAnswer = async (chat: number, final: string) => {
		const { text, files } = extractMedia(final);
		for (const chunk of split(text.trim())) {
			if (!chunk) continue;
			try {
				await api.sendMessage(chat, toHtml(chunk), { parse_mode: "HTML", link_preview_options: { is_disabled: true } });
			} catch {
				await api.sendMessage(chat, chunk).catch(() => {});
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


	const listener: Emit = (sessionId, type, payload) => {
		const chat = chatOf(sessionId);
		if (chat === undefined) return;
		if (type === "message.start") {
			const t: Turn = { chat, steps: [], started: Date.now(), dirty: false, lastEdit: 0 };
			turns.set(sessionId, t);
			void editStatus(t);
			api.sendChatAction(chat, "typing").catch(() => {});
			t.typing = setInterval(() => api.sendChatAction(chat, "typing").catch(() => {}), 4500);
		} else if (type === "tool.start") {
			const t = turns.get(sessionId);
			if (!t) return;
			t.steps.push(stepLine(String(payload.name), String(payload.context ?? "")));
			schedule(t);
		} else if (type === "subagent.start") {
			const t = turns.get(sessionId);
			if (!t) return;
			t.steps.push(`👥 ${String(payload.goal ?? "").slice(0, 70)}`);
			schedule(t);
		} else if (type === "message.complete") {
			const t = turns.get(sessionId);
			turns.delete(sessionId);
			if (t) {
				clearTimeout(t.timer);
				clearInterval(t.typing);
				void editStatus(t, "✅ Done");
			}
			void sendAnswer(chat, String(payload.text ?? ""));
		} else if (type === "clarify.request" || type === "approval.request") {
			const choices: string[] = type === "approval.request" ? ["once", "session", "always", "deny"] : ((payload.choices as string[]) ?? []);
			const text = type === "approval.request" ? `Approve this command?\n\n${payload.command}\n\n${payload.description ?? ""}` : String(payload.question ?? "");
			const keyboard = choices.map((c, i) => [{ text: c, callback_data: `p:${i}` }]);
			api.sendMessage(chat, text, keyboard.length ? { reply_markup: { inline_keyboard: keyboard } } : {})
				.then((m) => prompts.set(`${chat}:${m.message_id}`, { session: sessionId, promptId: String(payload.prompt_id), kind: type, choices }))
				.catch(() => {});
		}
	};
	rt.listeners.add(listener);

	const download = async (fileId: string): Promise<{ data: Buffer; name: string }> => {
		const f = await api.getFile(fileId);
		const r = await fetch(`https://api.telegram.org/file/bot${cfg.token}/${f.file_path}`);
		if (!r.ok) throw new Error(`download: HTTP ${r.status}`);
		return { data: Buffer.from(await r.arrayBuffer()), name: path.basename(f.file_path ?? "file") };
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
		await sessionFor(chat, nameOf(ctx));
		await ctx.reply("New session.");
	});
	bot.command("stop", async (ctx) => {
		await rt.interrupt(await sessionFor(ctx.chat.id, nameOf(ctx)));
		await ctx.reply("Stopped.");
	});

	bot.on("message", async (ctx) => {
		const m = ctx.message;
		const chat = ctx.chat.id;
		const session = await sessionFor(chat, nameOf(ctx));
		let prompt = m.text ?? m.caption ?? "";
		// A typed answer to a pending question.
		const pending = [...prompts.entries()].find(([k, p]) => k.startsWith(`${chat}:`) && p.session === session && p.kind === "clarify.request");
		if (pending && prompt) {
			prompts.delete(pending[0]);
			rt.respond(session, pending[1].promptId, prompt);
			return;
		}
		const attachments: { path: string; mime: string }[] = [];
		const voice = m.voice ?? m.audio;
		if (voice) {
			const { data, name } = await download(voice.file_id);
			try {
				prompt = await transcribe(profile, data, voice.mime_type ?? "audio/ogg", name.replace(/\.oga$/, ".ogg"));
			} catch (e) {
				return void ctx.reply(`Could not transcribe: ${(e as Error).message}`);
			}
			if (!prompt) return void ctx.reply("(nothing recognized)");
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
			if (mime.startsWith("image/")) attachments.push({ path: p, mime });
			prompt = `${prompt}\n\n[attached: ${p}]`.trim();
		}
		if (prompt) await rt.submit(session, prompt, attachments);
	});

	bot.on("callback_query:data", async (ctx) => {
		const msg = ctx.callbackQuery.message;
		const key = `${msg?.chat.id}:${msg?.message_id}`;
		const p = prompts.get(key);
		await ctx.answerCallbackQuery().catch(() => {});
		if (!p || !msg) return;
		const choice = p.choices[Number(ctx.callbackQuery.data.slice(2))] ?? "";
		prompts.delete(key);
		rt.respond(p.session, p.promptId, choice);
		await ctx.editMessageText(`${(msg as any).text ?? ""}\n\n→ ${choice.replace(/\s*\(Recommended\)$/, "")}`).catch(() => {});
	});

	bot.catch((e) => console.error("[telegram]", e.message));
	await bot.api.setMyCommands([
		{ command: "new", description: "Start a new session" },
		{ command: "stop", description: "Stop the running turn" },
	]).catch(() => {});
	const me = await bot.api.getMe();
	console.log(`[telegram] @${me.username}${project ? ` (project ${project})` : ""}`);
	// Stopping a bot (its settings changed) rejects its start: that is not a crash of the server.
	bot.start({ allowed_updates: ["message", "callback_query"] }).catch((e) => console.error(`[telegram] @${me.username} stopped: ${(e as Error)?.message ?? e}`));
	running.set(project, {
		config: JSON.stringify(cfg),
		stop: () => {
			rt.listeners.delete(listener);
			void bot.stop().catch(() => {});
		},
		send: (chat, text) => sendAnswer(chat, text),
	});
	if (project) projects.setBotName(project, me.username ?? "");
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
