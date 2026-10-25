// bench-agent's server: the protocol Bench speaks to Hermes in token mode — JSON-RPC 2.0 over a
// WebSocket at /api/ws?token=, REST under /api/ with X-Hermes-Session-Token — so Bench drives it
// as it drives `hermes serve`. Started by Bench (or `node src/server.ts`); writes its port and
// token to ~/.bench-agent/server.json.

import { execFileSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { WebSocketServer, type WebSocket } from "ws";
import YAML from "yaml";
import pkg from "../package.json" with { type: "json" };
import { timezone } from "./config.ts";
import { agentMcp, AGENT_HOME, BENCH_HOME, deepMerge, deleteProvider, getSetting, agentDir, agents, providers, readConfig, readEnv, readAgent, resolveModel, saveProvider, setEnv as storeEnv, setSetting, writeAgent, writeAgentMcp, migrateProfiles } from "./config.ts";
import { importHermes, migrateCronFile } from "./import-hermes.ts";
import { speak, transcribe } from "./audio.ts";
import { Runtime } from "./runtime.ts";
import { syncTelegram } from "./telegram.ts";
import { arm, describe, jobs, newJob, nextRun, runJob, type Job } from "./builtin/cron.ts";
import { connectMcp, mcpStatus } from "./mcp.ts";
import { tasks } from "./builtin/tasks.ts";
import { hosts, importSshConfig, importTransmit, type Host } from "./builtin/hosts.ts";
import { projects } from "./builtin/projects.ts";
import { listSkills as skillsOf } from "./builtin/skills.ts";
import * as remote from "./builtin/remote.ts";

const STATE = path.join(AGENT_HOME, "server.json");
const args = process.argv.slice(2);
const argPort = Number(args[args.indexOf("--port") + 1] || process.env.BENCH_AGENT_PORT || 0);

const prior = (() => {
	try {
		return JSON.parse(fs.readFileSync(STATE, "utf8"));
	} catch {
		return {};
	}
})();
const token: string = process.env.BENCH_AGENT_TOKEN || prior.token || randomBytes(32).toString("hex");

const sockets = new Set<WebSocket>();
function broadcast(sessionId: string, type: string, payload: Record<string, unknown>): void {
	const live = rt.live.get(sessionId);
	const frame = JSON.stringify({ jsonrpc: "2.0", method: "event", params: { session_id: sessionId, type, payload, execution_generation: live?.generation ?? 0 } });
	for (const ws of sockets) if (ws.readyState === ws.OPEN) ws.send(frame);
}

for (const line of importHermes()) console.log(`[import] ${line}`);
migrateCronFile();
for (const name of migrateProfiles()) console.log(`[migrate] profile ${name} → agents/${name}`);
const rt = new Runtime(broadcast);
await rt.open();


// ---- JSON-RPC ----------------------------------------------------------------

/** A project's environment, or "general": the agent's own (agents/default), what a remote
 * project shows — the server's agent with its general soul, skills, MCP, shared hosts, the
 * general Telegram bot. */
function envOf(id: string): { agent: string; dir: string; project: { id: string; name: string; telegram?: { token: string; allowed_users: string[]; username?: string } | null } } {
	if (id === "general") {
		const tg = getSetting("telegram");
		return { agent: "default", dir: agentDir("default"), project: { id, name: "General", telegram: tg?.token ? { token: tg.token, allowed_users: (tg.allowed_users ?? []).map(String) } : null } };
	}
	const p = projects.get(id);
	if (!p) throw new HttpError(404, "no such project");
	return { agent: `project:${id}`, dir: projects.env(id), project: p };
}

const methods: Record<string, (p: any) => Promise<any> | any> = {
	"profiles.list": () => ({
		profiles: agents().map((name) => ({ name, display_name: name, model: resolveModel(name).model })),
	}),
	"session.list": ({ limit, profile }) => ({
		sessions: rt.store
			// Every thread, whatever agent or project it runs with: the sidebar groups them by project.
			.list({ limit: limit ?? 200 })
			.filter((s) => s.source !== "subagent")
			.map((s) => ({ id: s.id, title: s.title, message_count: s.message_count, model: s.model, running: rt.isBusy(s.id), source: s.source, cwd: s.cwd, subject: s.subject ?? "", project: s.project ?? "", started_at: s.created, last_active: s.updated })),
	}),
	"session.create": async ({ profile, cwd, subject, project }) => {
		// A thread in a project works in the project's folder unless it asked for another.
		const dir = cwd || (project ? projects.dir(String(project)) : undefined);
		// A project's thread runs with the project's environment (general + its own).
		const agent = project ? `project:${project}` : profile || "default";
		const live = await rt.create(agent, dir, "local", { ...(subject ? { subject: String(subject) } : {}), ...(project ? { project: String(project) } : {}) });
		broadcast(live.row.id, "sessions.changed", {});
		return { session_id: live.row.id, stored_session_id: live.row.id, revision: 0, execution_generation: 0 };
	},
	// A tool attached to the thread (`/ssh web-1`, `/tasks`…): its block from now on.
	"session.subject": async ({ session_id, subject }) => {
		rt.setSubject(String(session_id), String(subject ?? ""));
		broadcast(session_id, "sessions.changed", {});
		return { ok: true };
	},
	// The person's inserts (src/inserts.ts): put one where the thread is now, or change one. Every
	// window showing the thread hears the new list.
	"session.insert": async ({ session_id, id, key, state }) => {
		const inserts = await rt.insert(String(session_id), String(id ?? ""), String(key ?? ""), state && typeof state === "object" ? state : {});
		broadcast(session_id, "session.inserts", { inserts });
		return { inserts };
	},
	"session.insert.patch": async ({ session_id, id, patch }) => {
		const inserts = await rt.patchInsert(String(session_id), String(id ?? ""), patch && typeof patch === "object" ? patch : {});
		broadcast(session_id, "session.inserts", { inserts });
		return { inserts };
	},
	"session.resume": async ({ session_id }) => {
		const live = await rt.session(session_id);
		if (!live) throw new Error(`session not found: ${session_id}`);
		const row = rt.store.get(session_id)!;
		return {
			session_id,
			stored_session_id: session_id,
			revision: 0,
			execution_generation: live.generation,
			running: live.busy,
			messages: await rt.history(session_id),
			inserts: await rt.inserts(session_id),
			queue: await rt.queue(session_id),
			prompts: rt.pendingPrompts(session_id),
			info: { reasoning_effort: row.effort, model: row.model, cwd: row.cwd, revision: 0, execution_generation: live.generation },
			todo_state: { todos: row.todos },
		};
	},
	"session.history": async ({ session_id }) => ({ messages: rt.store.messages(session_id) }),
	"session.close": async ({ session_id }) => {
		if (!rt.isBusy(session_id)) await rt.close(session_id);
		return { ok: true };
	},
	"session.delete": async ({ session_id }) => {
		await rt.delete(session_id);
		broadcast(session_id, "sessions.changed", {});
		return { ok: true };
	},
	"prompt.submit": async ({ session_id, text, attachments, context, mode }) => {
		await rt.submit(session_id, String(text ?? ""), attachments ?? [], String(context ?? ""), mode === "queue" ? "queue" : "steer");
		return { ok: true };
	},
	// The messages waiting while the agent works (Pi's inbox): take one back, or send it now.
	"session.queue": async ({ session_id }) => ({ items: await rt.queue(String(session_id)) }),
	"session.queue.withdraw": async ({ session_id, id }) => ({ result: await rt.withdraw(String(session_id), String(id)) }),
	"session.queue.send": async ({ session_id, id }) => {
		await rt.sendNow(String(session_id), String(id));
		return { ok: true };
	},
	"session.interrupt": async ({ session_id }) => {
		await rt.interrupt(session_id);
		return { ok: true };
	},
	"session.undo": async ({ session_id }) => {
		await rt.undo(session_id);
		return { ok: true };
	},
	"clarify.respond": ({ session_id, prompt_id, request_id, answer }) => {
		rt.respond(session_id, prompt_id ?? request_id, String(answer ?? ""));
		return { ok: true };
	},
	"approval.respond": ({ session_id, prompt_id, request_id, choice }) => {
		rt.respond(session_id, prompt_id ?? request_id, String(choice ?? "deny"));
		return { ok: true };
	},
	"config.set": async ({ key, value, session_id, profile }) => {
		if (key === "reasoning" && session_id) {
			rt.store.update(session_id, { effort: String(value) });
			broadcast(session_id, "session.info", { reasoning_effort: value });
		} else if (key === "approvals.mode") {
			writeAgent(profile || "default", { ...readAgent(profile || "default"), approvals: String(value) });
		}
		return { ok: true };
	},
	// The session's commands only: skills are found and loaded by the agent itself, not typed.
	"commands.catalog": () => ({ categories: [{ name: "Session", pairs: COMMANDS.map((c) => [c[0], c[1]]) }], pairs: [] }),
	"slash.exec": async (p) => slash(p),
	"command.dispatch": async ({ session_id, profile, name, arg }) => slash({ session_id, profile, command: `/${String(name).replace(/^\//, "")} ${arg ?? ""}`.trim() }),
	// Pi's bash has no background processes.
	"process.list": () => ({ processes: [] }),
	"process.kill": async ({ session_id, process_id }) => {
		void session_id;
		void process_id;
		return { ok: true };
	},
};

const COMMANDS: [string, string][] = [
	["/new", "Start a new session"],
	["/model", "Switch the model: /model <name> [--provider <p>]"],
	["/reasoning", "Set reasoning effort: low, medium, high, xhigh"],
	["/title", "Rename this session"],
	["/stop", "Stop the running turn"],
	["/retry", "Regenerate the last answer"],
	["/compact", "Compress the conversation context"],
];

function skillPairs(profile: string): [string, string][] {
	const dir = path.join(agentDir(profile), "skills");
	const out: [string, string][] = [];
	const walk = (d: string, depth: number) => {
		if (depth > 3 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory() || e.name.startsWith(".")) continue;
			const skill = path.join(d, e.name, "SKILL.md");
			if (fs.existsSync(skill)) {
				const m = fs.readFileSync(skill, "utf8").match(/^---\n([\s\S]*?)\n---/);
				let desc = "";
				try {
					desc = String(YAML.parse(m?.[1] ?? "")?.description ?? "");
				} catch {}
				out.push([`/${e.name}`, desc.slice(0, 80)]);
			} else walk(path.join(d, e.name), depth + 1);
		}
	};
	walk(dir, 0);
	return out.sort((a, b) => a[0].localeCompare(b[0]));
}

async function slash({ session_id, profile, command }: { session_id: string; profile?: string; command: string }) {
	const [name, ...rest] = String(command).trim().split(/\s+/);
	const arg = rest.join(" ");
	const row = session_id ? rt.store.get(session_id) : undefined;
	switch (name) {
		case "/model": {
			if (!row) return { type: "info", output: "No session." };
			const m = arg.match(/^(\S+)(?:\s+--provider\s+(\S+))?/);
			if (!m) return { type: "info", output: `Model: ${row.model} (${row.provider})` };
			rt.store.update(session_id, { model: m[1], provider: m[2] ?? row.provider });
			broadcast(session_id, "session.info", { model: m[1] });
			return { type: "info", output: `Model set to ${m[1]}${m[2] ? ` (${m[2]})` : ""}` };
		}
		case "/reasoning":
			if (row && arg) rt.store.update(session_id, { effort: arg });
			return { type: "info", output: `Reasoning effort: ${arg || row?.effort}` };
		case "/title":
			if (row && arg) rt.store.update(session_id, { title: arg });
			broadcast(session_id, "sessions.changed", {});
			return { type: "info", output: `Title: ${arg}` };
		case "/stop":
			await rt.interrupt(session_id);
			return { type: "info", output: "Stopped." };
		case "/retry":
			await rt.undo(session_id);
			return { type: "info", output: "Last exchange removed; send it again." };
		case "/compact": {
			const live = await rt.session(session_id);
			if (live) await live.conversation.compact(arg || undefined, BACKGROUND);
			return { type: "info", output: "Compacting the context." };
		}
		default: {
			// A skill: its body becomes the turn, as Hermes' /skill-name does.
			const skill = findSkill(profile || row?.profile || "default", name.slice(1));
			if (skill) {
				const body = fs.readFileSync(skill, "utf8");
				return { type: "skill", message: `[SYSTEM: The user has invoked the "${name.slice(1)}" skill, indicating they want you to follow its instructions. The full skill content is loaded below.]\n\n${body}${arg ? `\n\nThe user has provided the following instruction alongside the skill invocation: ${arg}` : ""}` };
			}
			return { type: "info", output: `Unknown command: ${name}` };
		}
	}
}

import { BACKGROUND_CONTEXT as BACKGROUND } from "@earendil-works/chord/context";

function findSkill(profile: string, name: string): string | undefined {
	const dir = path.join(agentDir(profile), "skills");
	const walk = (d: string, depth: number): string | undefined => {
		if (depth > 3 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory()) continue;
			const f = path.join(d, e.name, "SKILL.md");
			if (e.name === name && fs.existsSync(f)) return f;
			const deeper = walk(path.join(d, e.name), depth + 1);
			if (deeper) return deeper;
		}
	};
	return walk(dir, 0);
}

/** `PUT /api/config`: each top-level key merges into its setting; `model` goes to the profile. */
function patchConfig(profile: string, patch: any): void {
	for (const [k, v] of Object.entries(patch ?? {})) {
		if (k === "model" && v && typeof v === "object") {
			const m = v as any;
			const p = readAgent(profile);
			writeAgent(profile, { ...p, defaultProvider: m.provider ?? p.defaultProvider, defaultModel: m.default ?? m.name ?? p.defaultModel });
		} else setSetting(k, deepMerge(getSetting(k), v));
	}
}

// ---- REST ----------------------------------------------------------------

type Req = http.IncomingMessage & { body?: any; query: URLSearchParams; path: string };
type Route = (req: Req) => Promise<any> | any;

class HttpError extends Error {
	status: number;
	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

const expand = (p: string) => (p.startsWith("~") ? path.join(os.homedir(), p.slice(1)) : p);
const profileOf = (req: Req) => req.query.get("profile") || "default";

const required = (v: unknown, name: string): string => {
	if (typeof v !== "string" || !v.trim()) throw new HttpError(400, `${name} is required`);
	return v;
};

/** Runs a file operation on the host of the route; the server's errors come back as 502, without the password. */
async function onHost<T>(m: RegExpMatchArray, fn: (h: Host) => Promise<T>): Promise<T> {
	const h = hosts.get(decodeURIComponent(m[1]));
	if (!h) throw new HttpError(404, "no such host");
	try {
		return await fn(h);
	} catch (e) {
		if (e instanceof HttpError) throw e;
		throw new HttpError(502, remote.remoteError(e, h));
	}
}

const routes: Record<string, Route> = {
	"GET /api/status": () => ({
		version: `bench-agent ${pkg.version}`,
		gateway_running: true,
		gateway_state: "running",
		overall: "ok",
		active_agents: [...rt.live.values()].filter((l) => l.busy).length,
		gateway_platforms: {},
		backend: "pi",
		mcp: mcpStatus(),
	}),
	"GET /api/sessions": (req) => {
		const source = req.query.get("source") ?? undefined;
		const rows = rt.store.list({ source, limit: Number(req.query.get("limit") ?? 100) }).filter((s) => s.source !== "subagent");
		return { sessions: rows.map((s) => ({ id: s.id, title: s.title, cwd: s.cwd, source: s.source, git_repo_root: gitRoot(s.cwd), git_branch: null, chat_id: s.chat_id, user_id: s.chat_id, display_name: s.display_name })) };
	},
	"GET /api/sessions/search": (req) => ({ results: rt.store.search(req.query.get("q") ?? "", Number(req.query.get("limit") ?? 30)).map((r) => ({ session_id: r.session_id, snippet: r.snippet })) }),
	// What Bench still reads from Hermes' config.yaml, built from Bench's own settings (read-only).
	"GET /api/config/raw": (req) => ({ yaml: YAML.stringify(readConfig(profileOf(req))) }),
	"PUT /api/config": (req) => {
		patchConfig(profileOf(req), req.body?.config ?? {});
		return { ok: true };
	},
	"PUT /api/mcp/servers": (req) => {
		writeAgentMcp(profileOf(req), req.body?.servers ?? {});
		void connectMcp(profileOf(req)).then(() => rt.reloadMcp(profileOf(req)));
		return { ok: true };
	},
	"GET /api/skills": (req) => listSkills(profileOf(req)),
	"GET /api/skills/content": (req) => {
		const f = findSkill(profileOf(req), req.query.get("name") ?? "");
		if (!f) throw new HttpError(404, "skill not found");
		return { content: fs.readFileSync(f, "utf8"), path: f };
	},
	"PUT /api/skills/content": (req) => {
		const f = findSkill(profileOf(req), req.body?.name ?? "") ?? path.join(agentDir(profileOf(req)), "skills", req.body?.name, "SKILL.md");
		fs.mkdirSync(path.dirname(f), { recursive: true });
		fs.writeFileSync(f, String(req.body?.content ?? ""));
		return { ok: true, path: f };
	},
	"DELETE /api/files": (req) => {
		fs.rmSync(expand(req.body.path), { recursive: Boolean(req.body.recursive), force: true });
		return { ok: true };
	},
	"POST /api/files/mkdir": (req) => {
		fs.mkdirSync(expand(req.body.path), { recursive: true });
		return { ok: true };
	},
	"POST /api/files/upload": (req) => {
		const p = expand(req.body.path);
		if (fs.existsSync(p) && req.body.overwrite === false) throw new HttpError(409, "exists");
		fs.mkdirSync(path.dirname(p), { recursive: true });
		fs.writeFileSync(p, dataUrl(req.body.data_url));
		return { ok: true, path: p };
	},
	"GET /api/fs/read-text": (req) => {
		const p = expand(req.query.get("path") ?? "");
		const buf = fs.readFileSync(p);
		const binary = buf.subarray(0, 8000).includes(0);
		const cap = 2_000_000;
		return { text: binary ? "" : buf.subarray(0, cap).toString("utf8"), truncated: buf.length > cap, binary };
	},
	"POST /api/fs/write-text": (req) => {
		const p = expand(req.body.path);
		fs.mkdirSync(path.dirname(p), { recursive: true });
		fs.writeFileSync(p, String(req.body.content ?? ""));
		return { ok: true };
	},
	"GET /api/fs/list": (req) => {
		const p = expand(req.query.get("path") ?? os.homedir());
		try {
			return { entries: fs.readdirSync(p, { withFileTypes: true }).map((e) => ({ name: e.name, path: path.join(p, e.name), isDirectory: e.isDirectory() || (e.isSymbolicLink() && isDir(path.join(p, e.name))) })) };
		} catch (e) {
			return { entries: [], error: String((e as Error).message) };
		}
	},
	"GET /api/fs/default-cwd": () => ({ cwd: os.homedir() }),
	// A branch in its own folder, <repo>/.worktrees/<name>.
	"POST /api/git/worktree/add": (req) => {
		const repo = expand(String(req.body?.path ?? ""));
		const name = String(req.body?.name ?? "").trim().toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 60) || `idea-${Date.now()}`;
		const folder = path.join(repo, ".worktrees", name);
		const branch = `bench/${name}`;
		execFileSync("git", ["-C", repo, "worktree", "add", "-b", branch, folder], { stdio: "pipe" });
		return { path: folder, branch };
	},
	"POST /api/git/worktree/remove": (req) => {
		const repo = expand(String(req.body?.path ?? ""));
		execFileSync("git", ["-C", repo, "worktree", "remove", ...(req.body?.force ? ["--force"] : []), expand(String(req.body?.worktreePath ?? ""))], { stdio: "pipe" });
		return { ok: true };
	},
	"GET /api/chat/workspaces": () => ({ home: os.homedir() }),
	"POST /api/chat/image-upload": (req) => {
		const dir = path.join(BENCH_HOME, "attachments");
		fs.mkdirSync(dir, { recursive: true });
		const p = path.join(dir, `${Date.now()}-${path.basename(String(req.body.filename ?? "image.png"))}`);
		fs.writeFileSync(p, dataUrl(req.body.data_url));
		return { path: p };
	},
	"GET /api/cron/jobs": (req) => jobs.list(profileOf(req)).map(cronView),
	// The Tasks mode: open tasks and the ones done since ?done_since=YYYY-MM-DD.
	"GET /api/tasks": (req) => ({ tasks: tasks.list(req.query.get("done_since") ?? undefined) }),
	"POST /api/tasks": (req) => ({ task: tasks.save(req.body) }),
	// Projects: the sidebar's unit; a folder on disk is optional.
	"GET /api/projects": () => ({ projects: projects.list() }),
	"POST /api/projects": (req) => {
		if (!String(req.body?.name ?? "").trim()) throw new HttpError(400, "name is required");
		return { project: projects.save({ ...req.body, name: String(req.body.name).trim() }) };
	},
	// Server accesses (SSH, SFTP, FTP, FTPS) with their passwords.
	"GET /api/hosts": () => ({ hosts: hosts.list() }),
	// By `source` when given ("ssh_config:<alias>"): the row already imported is updated, not doubled.
	"POST /api/hosts": (req) => {
		const prev = req.body?.source ? hosts.bySource(String(req.body.source)) : undefined;
		return { host: hosts.save(prev ? { ...prev, ...req.body, id: prev.id } : req.body) };
	},
	"POST /api/hosts/import": () => ({ ssh_config: importSshConfig(), transmit: importTransmit() }),
	/** Bench's one-time move from Apple Reminders: items carry source_id (the reminder's id). */
	"POST /api/tasks/import": (req) => ({ imported: tasks.import(req.body?.tasks ?? []) }),
	// Dictation and read-aloud through Hermes' stt / tts config (OpenAI-compatible servers).
	"POST /api/audio/transcribe": async (req) => {
		try {
			return { transcript: await transcribe(profileOf(req), dataUrl(req.body?.data_url), String(req.body?.mime_type ?? "audio/wav")) };
		} catch (e) {
			throw new HttpError(502, String((e as Error).message));
		}
	},
	"POST /api/audio/speak": async (req) => {
		try {
			const { type, audio } = await speak(profileOf(req), String(req.body?.text ?? ""));
			return { data_url: `data:${type};base64,${audio.toString("base64")}` };
		} catch (e) {
			throw new HttpError(502, String((e as Error).message));
		}
	},
	"POST /api/providers/custom-endpoints": (req) => {
		const b = req.body ?? {};
		const id = String(b.id || b.name || "").trim();
		if (!id || !b.base_url) throw new HttpError(400, "id and base_url are required");
		const prev = providers().find((p) => p.id === id);
		const models = { ...(prev?.models ?? {}), ...(b.model ? { [b.model]: b.context_length ? { context_length: Number(b.context_length) } : {} } : {}) };
		saveProvider({ id, name: b.name ?? id, base_url: b.base_url, api_key: b.api_key || prev?.api_key, api_mode: prev?.api_mode ?? "chat_completions", models, context_length: b.context_length ? Number(b.context_length) : prev?.context_length });
		return { ok: true, id };
	},
	"POST /api/providers/custom-endpoints/validate": async (req) => {
		const b = req.body ?? {};
		try {
			const r = await fetch(`${String(b.base_url).replace(/\/$/, "")}/models`, { headers: b.api_key ? { Authorization: `Bearer ${b.api_key}` } : {}, signal: AbortSignal.timeout(10000) });
			if (!r.ok) return { ok: false, message: `HTTP ${r.status}`, models: [] };
			const j: any = await r.json();
			const models = (j.data ?? j.models ?? []).map((m: any) => m.id ?? m.name).filter(Boolean);
			return { ok: true, message: `${models.length} models`, models };
		} catch (e) {
			return { ok: false, message: String((e as Error).message), models: [] };
		}
	},
	"GET /api/profiles": () => ({
		profiles: agents().map((name) => ({ name, display_name: name, model: resolveModel(name).model, skill_count: listSkills(name).length, is_default: name === "default" })),
	}),
	"POST /api/profiles": (req) => {
		const name = String(req.body?.name ?? "").trim();
		if (!/^[a-z0-9][a-z0-9_-]*$/i.test(name)) throw new HttpError(400, "invalid profile name");
		if (agents().includes(name)) throw new HttpError(409, "profile exists");
		const from = req.body?.clone_from ? String(req.body.clone_from) : "default";
		writeAgent(name, readAgent(from));
		for (const f of ["SYSTEM.md", "AGENTS.md", "mcp.json"]) if (fs.existsSync(path.join(agentDir(from), f))) fs.copyFileSync(path.join(agentDir(from), f), path.join(agentDir(name), f));
		return { name };
	},
	"GET /api/providers/custom-endpoints": (req) => {
		const current = readAgent(profileOf(req)).defaultProvider;
		const endpoints = providers().map((p) => ({
			id: p.id,
			name: p.name,
			base_url: p.base_url,
			model: Object.keys(p.models)[0] ?? "",
			models: Object.keys(p.models),
			context_length: p.context_length ?? null,
			has_api_key: Boolean(p.api_key),
			api_key_preview: p.api_key ? `${p.api_key.slice(0, 3)}…${p.api_key.slice(-2)}` : "",
			is_current: p.id === current,
		}));
		return { endpoints, current };
	},
	"GET /api/env": (req) => {
		const env = readEnv(profileOf(req));
		return Object.fromEntries(Object.entries(env).map(([k, v]) => [k, { is_set: Boolean(v), redacted_value: v ? `${v.slice(0, 3)}…${v.slice(-2)}` : "", description: "", category: "", channel_managed: false, custom: true }]));
	},
	"PUT /api/env": (req) => {
		setEnv(String(req.body.key), String(req.body.value ?? ""));
		return { ok: true };
	},
	"DELETE /api/env": (req) => {
		setEnv(String(req.body.key), undefined);
		return { ok: true };
	},
};

// Routes with a path parameter.
const dynamic: [string, RegExp, (req: Req, m: RegExpMatchArray) => any][] = [
	["GET", /^\/api\/sessions\/([^/]+)\/mutation-snapshot$/, () => {
		throw new HttpError(404, "no mutation snapshots");
	}],
	["DELETE", /^\/api\/sessions\/([^/]+)$/, async (_req, m) => {
		await rt.delete(decodeURIComponent(m[1]));
		broadcast(m[1], "sessions.changed", {});
		return { ok: true };
	}],
	["PUT", /^\/api\/tasks\/([^/]+)$/, (req, m) => {
		const id = decodeURIComponent(m[1]);
		const prev = tasks.get(id);
		if (!prev) throw new HttpError(404, "no such task");
		return { task: tasks.save({ ...prev, ...req.body, id }) };
	}],
	["PUT", /^\/api\/hosts\/([^/]+)$/, (req, m) => {
		const prev = hosts.get(decodeURIComponent(m[1]));
		if (!prev) throw new HttpError(404, "no such host");
		return { host: hosts.save({ ...prev, ...req.body, id: prev.id }) };
	}],
	["PUT", /^\/api\/projects\/([^/]+)$/, (req, m) => {
		const prev = projects.get(decodeURIComponent(m[1]));
		if (!prev) throw new HttpError(404, "no such project");
		return { project: projects.save({ ...prev, ...req.body, id: prev.id }) };
	}],
	// What a project's threads get on top of the general agent: its soul, MCP servers, skills.
	["GET", /^\/api\/projects\/([^/]+)\/env$/, (_req, m) => {
		const id = decodeURIComponent(m[1]);
		const { agent, dir, project: p } = envOf(id);
		const soulFile = path.join(dir, "SYSTEM.md");
		const status = mcpStatus(agent);
		return {
			project: p,
			dir,
			soul: fs.existsSync(soulFile) ? fs.readFileSync(soulFile, "utf8") : "",
			mcp: Object.entries(agentMcp(agent)).map(([name, spec]: [string, any]) => {
				const s = status.find((x) => x.name === name);
				return { name, kind: spec?.url ? "http" : "stdio", enabled: spec?.enabled !== false, connected: !!s, tools: s?.tools ?? 0, error: s?.error ?? null, spec: JSON.stringify(spec, null, 2) };
			}),
			skills: skillsOf(agent).filter((k) => k.dir.startsWith(dir)).map((k) => ({ name: k.name, description: k.description })),
			prompts: fs.existsSync(path.join(dir, "prompts")) ? fs.readdirSync(path.join(dir, "prompts")).filter((f) => f.endsWith(".md")).map((f) => f.replace(/\.md$/, "")).sort() : [],
			mcp_json: fs.existsSync(path.join(dir, "mcp.json")) ? fs.readFileSync(path.join(dir, "mcp.json"), "utf8") : "",
			hosts: hosts.list().filter((h) => h.project === (id === "general" ? "" : id)).map((h) => ({ id: h.id, name: h.name, kind: h.kind, host: h.host, user: h.user })),
			// Its scheduled jobs: what runs when, and how the last run went.
			cron: jobs.list(agent).map(cronView),
			// The bot's token is not sent back, only whether there is one.
			telegram: p.telegram ? { username: p.telegram.username ?? "", allowed_users: p.telegram.allowed_users, has_token: !!p.telegram.token } : null,
		};
	}],
	// A project's own Telegram bot: its token and who may write to it; "" token = none.
	["PUT", /^\/api\/projects\/([^/]+)\/telegram$/, async (req, m) => {
		const { project: p } = envOf(decodeURIComponent(m[1]));
		const token = String(req.body?.token ?? p.telegram?.token ?? "").trim();
		const allowed = (req.body?.allowed_users ?? p.telegram?.allowed_users ?? []).map(String).filter(Boolean);
		// The general bot stays a setting; a project's is the project's.
		if (p.id === "general") setSetting("telegram", token ? { ...(getSetting("telegram") ?? {}), token, allowed_users: allowed } : undefined);
		else projects.save({ ...(projects.get(p.id) as any), telegram: token ? { token, allowed_users: allowed } : null });
		await syncTelegram(rt);
		return { ok: true };
	}],
	// A project's MCP server: on / off, or gone.
	["PUT", /^\/api\/projects\/([^/]+)\/mcp\/([^/]+)$/, (req, m) => {
		const { agent } = envOf(decodeURIComponent(m[1]));
		const name = decodeURIComponent(m[2]);
		const servers = agentMcp(agent);
		if (!servers[name]) throw new HttpError(404, "no such server");
		servers[name] = { ...servers[name], enabled: req.body?.enabled !== false };
		writeAgentMcp(agent, servers);
		return { ok: true };
	}],
	["DELETE", /^\/api\/projects\/([^/]+)\/mcp\/([^/]+)$/, (_req, m) => {
		const { agent } = envOf(decodeURIComponent(m[1]));
		const servers = agentMcp(agent);
		delete servers[decodeURIComponent(m[2])];
		writeAgentMcp(agent, servers);
		return { ok: true };
	}],
	// A skill of the project's threads (its own or a general one), to read.
	["GET", /^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/, (_req, m) => {
		const id = decodeURIComponent(m[1]);
		const skill = skillsOf(envOf(id).agent).find((k) => k.name === decodeURIComponent(m[2]));
		if (!skill) throw new HttpError(404, "no such skill");
		return { name: skill.name, dir: skill.dir, text: fs.readFileSync(path.join(skill.dir, "SKILL.md"), "utf8") };
	}],
	// A project's skill changed by hand (its SKILL.md as typed).
	["PUT", /^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/, (req, m) => {
		const id = decodeURIComponent(m[1]);
		const { dir } = envOf(id);
		const skill = skillsOf(envOf(id).agent).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
		if (!skill) throw new HttpError(404, "no such skill of the project");
		fs.writeFileSync(path.join(skill.dir, "SKILL.md"), String(req.body?.text ?? ""));
		return { ok: true };
	}],
	// The project's mcp.json as typed: it must be JSON with mcpServers.
	["PUT", /^\/api\/projects\/([^/]+)\/mcp$/, (req, m) => {
		const id = decodeURIComponent(m[1]);
		let parsed: any;
		try {
			parsed = JSON.parse(String(req.body?.text ?? ""));
		} catch (e) {
			throw new HttpError(400, `not JSON: ${(e as Error).message}`);
		}
		if (typeof parsed?.mcpServers !== "object") throw new HttpError(400, "it needs an mcpServers object");
		writeAgentMcp(envOf(id).agent, parsed.mcpServers);
		return { ok: true };
	}],
	// An access taken out of the project (it stays in the store).
	["DELETE", /^\/api\/projects\/([^/]+)\/hosts\/([^/]+)$/, (_req, m) => {
		const h = hosts.get(decodeURIComponent(m[2]));
		if (h && h.project === decodeURIComponent(m[1])) hosts.save({ ...h, project: "" });
		return { ok: true };
	}],
	// Prompts a thread can insert (@ in the composer): the general ones and the project's own.
	["GET", /^\/api\/projects\/([^/]+)\/prompts$/, (_req, m) => {
		const id = decodeURIComponent(m[1]);
		const read = (dir: string, own: boolean) =>
			fs.existsSync(dir)
				? fs.readdirSync(dir).filter((f) => f.endsWith(".md")).map((f) => ({ name: f.replace(/\.md$/, ""), text: fs.readFileSync(path.join(dir, f), "utf8"), own }))
				: [];
		const own = read(path.join(envOf(id).dir, "prompts"), true);
		const general = id === "general" ? [] : read(path.join(agentDir("default"), "prompts"), false).filter((g) => !own.some((o) => o.name === g.name));
		return { prompts: [...own, ...general].sort((a, b) => a.name.localeCompare(b.name)) };
	}],
	["PUT", /^\/api\/projects\/([^/]+)\/prompts\/([^/]+)$/, (req, m) => {
		const dir = path.join(envOf(decodeURIComponent(m[1])).dir, "prompts");
		fs.mkdirSync(dir, { recursive: true });
		fs.writeFileSync(path.join(dir, `${path.basename(decodeURIComponent(m[2]))}.md`), String(req.body?.text ?? ""));
		return { ok: true };
	}],
	["DELETE", /^\/api\/projects\/([^/]+)\/prompts\/([^/]+)$/, (_req, m) => {
		fs.rmSync(path.join(envOf(decodeURIComponent(m[1])).dir, "prompts", `${path.basename(decodeURIComponent(m[2]))}.md`), { force: true });
		return { ok: true };
	}],
	// One MCP server's config (its entry in mcp.json), as typed.
	["PUT", /^\/api\/projects\/([^/]+)\/mcp\/([^/]+)\/spec$/, (req, m) => {
		const { agent } = envOf(decodeURIComponent(m[1]));
		let spec: any;
		try {
			spec = JSON.parse(String(req.body?.text ?? ""));
		} catch (e) {
			throw new HttpError(400, `not JSON: ${(e as Error).message}`);
		}
		const servers = agentMcp(agent);
		servers[decodeURIComponent(m[2])] = spec;
		writeAgentMcp(agent, servers);
		return { ok: true };
	}],
	// A project's own skill goes (its folder).
	["DELETE", /^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/, (_req, m) => {
		const id = decodeURIComponent(m[1]);
		const { dir } = envOf(id);
		const skill = skillsOf(envOf(id).agent).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
		if (!skill) throw new HttpError(404, "no such skill");
		fs.rmSync(skill.dir, { recursive: true, force: true });
		return { ok: true };
	}],
	["PUT", /^\/api\/projects\/([^/]+)\/soul$/, (req, m) => {
		const id = decodeURIComponent(m[1]);
		fs.writeFileSync(path.join(envOf(id).dir, "SYSTEM.md"), String(req.body?.text ?? ""));
		return { ok: true };
	}],
	["DELETE", /^\/api\/projects\/([^/]+)$/, (_req, m) => {
		projects.remove(decodeURIComponent(m[1]));
		return { ok: true };
	}],
	["DELETE", /^\/api\/hosts\/([^/]+)$/, (_req, m) => {
		hosts.remove(decodeURIComponent(m[1]));
		return { ok: true };
	}],
	// A host's files (FTP, FTPS, SFTP): Bench's file browser.
	["GET", /^\/api\/hosts\/([^/]+)\/ls$/, (req, m) => onHost(m, (h) => remote.ls(h, req.query.get("path")))],
	["GET", /^\/api\/hosts\/([^/]+)\/file$/, (req, m) =>
		onHost(m, async (h) => {
			const max = Math.min(Number(req.query.get("max") || 5_000_000), 50_000_000);
			const r = await remote.read(h, required(req.query.get("path"), "path"), max);
			return { path: r.path, size: r.size, ...remote.encodeContent(r.data, r.truncated), truncated: r.truncated };
		})],
	["PUT", /^\/api\/hosts\/([^/]+)\/file$/, (req, m) =>
		onHost(m, async (h) => {
			const b = req.body ?? {};
			if (typeof b.text !== "string" && typeof b.base64 !== "string") throw new HttpError(400, "text or base64 is required");
			await remote.write(h, required(b.path, "path"), typeof b.text === "string" ? Buffer.from(b.text, "utf8") : Buffer.from(b.base64, "base64"));
			return { ok: true };
		})],
	["DELETE", /^\/api\/hosts\/([^/]+)\/file$/, (req, m) => onHost(m, async (h) => (await remote.remove(h, required(req.query.get("path"), "path")), { ok: true }))],
	["POST", /^\/api\/hosts\/([^/]+)\/mkdir$/, (req, m) => onHost(m, async (h) => (await remote.mkdir(h, required(req.body?.path, "path")), { ok: true }))],
	["POST", /^\/api\/hosts\/([^/]+)\/rename$/, (req, m) => onHost(m, async (h) => (await remote.rename(h, required(req.body?.from, "from"), required(req.body?.to, "to")), { ok: true }))],
	["DELETE", /^\/api\/tasks\/([^/]+)$/, (_req, m) => {
		tasks.remove(decodeURIComponent(m[1]));
		return { ok: true };
	}],
	["PUT", /^\/api\/cron\/jobs\/([^/]+)$/, async (req, m) => {
		const job = jobs.resolve(decodeURIComponent(m[1]));
		const u = req.body?.updates ?? {};
		const schedule = typeof u.schedule === "object" ? u.schedule?.expr : u.schedule;
		const next: Job = { ...job, prompt: u.prompt ?? job.prompt, name: u.name ?? job.name, schedule: schedule ?? job.schedule };
		try {
			next.next_run = nextRun(next.schedule, Date.now(), null);
		} catch (e) {
			throw new HttpError(400, `the schedule: ${(e as Error).message}`);
		}
		jobs.save(next);
		await arm(next);
		return cronView(jobs.get(job.id)!);
	}],
	// A project's new scheduled job (the general environment's: "general").
	["POST", /^\/api\/projects\/([^/]+)\/cron$/, async (req, m) => {
		const { agent } = envOf(decodeURIComponent(m[1]));
		const name = String(req.body?.name ?? "").trim();
		const prompt = String(req.body?.prompt ?? "").trim();
		const schedule = String(req.body?.schedule ?? "").trim();
		if (!name || !prompt || !schedule) throw new HttpError(400, "name, prompt and schedule are required");
		let job: Job;
		try {
			job = newJob({ profile: agent, name, prompt, schedule });
		} catch (e) {
			throw new HttpError(400, `the schedule: ${(e as Error).message}`);
		}
		jobs.save(job);
		await arm(job);
		return cronView(jobs.get(job.id)!);
	}],
	["DELETE", /^\/api\/cron\/jobs\/([^/]+)$/, async (_req, m) => {
		const job = jobs.resolve(decodeURIComponent(m[1]));
		jobs.remove(job.id);
		// Its sleeping task stops (the job is gone).
		await arm(job);
		return { ok: true };
	}],
	["POST", /^\/api\/cron\/jobs\/([^/]+)\/(trigger|pause|resume)$/, async (_req, m) => {
		const id = decodeURIComponent(m[1]);
		const job = jobs.resolve(id);
		if (m[2] === "trigger") void runJob(job);
		else {
			const enabled = m[2] === "resume";
			const next = { ...job, enabled, next_run: enabled ? nextRun(job.schedule, Date.now(), null) : job.next_run };
			jobs.save(next);
			await arm(next);
		}
		return { ok: true };
	}],
	["GET", /^\/api\/profiles\/([^/]+)\/soul$/, (_req, m) => {
		const f = path.join(agentDir(decodeURIComponent(m[1])), "SYSTEM.md");
		return { content: fs.existsSync(f) ? fs.readFileSync(f, "utf8") : "" };
	}],
	["PUT", /^\/api\/profiles\/([^/]+)\/soul$/, (req, m) => {
		const f = path.join(agentDir(decodeURIComponent(m[1])), "SYSTEM.md");
		fs.mkdirSync(path.dirname(f), { recursive: true });
		fs.writeFileSync(f, String(req.body?.content ?? ""));
		return { ok: true };
	}],
	["PATCH", /^\/api\/profiles\/([^/]+)$/, (req, m) => {
		const from = agentDir(decodeURIComponent(m[1]));
		const name = String(req.body?.new_name ?? "");
		if (!/^[a-z0-9][a-z0-9_-]*$/i.test(name) || m[1] === "default") throw new HttpError(400, "invalid rename");
		fs.renameSync(from, agentDir(name));
		return { name };
	}],
	["POST", /^\/api\/providers\/custom-endpoints\/([^/]+)\/activate$/, (req, m) => {
		const id = decodeURIComponent(m[1]);
		const p = providers().find((x) => x.id === id);
		if (!p) throw new HttpError(404, `no provider ${id}`);
		const profile = profileOf(req);
		writeAgent(profile, { ...readAgent(profile), defaultProvider: id, defaultModel: Object.keys(p.models)[0] ?? readAgent(profile).defaultModel });
		return { ok: true };
	}],
	["DELETE", /^\/api\/providers\/custom-endpoints\/([^/]+)$/, (_req, m) => {
		deleteProvider(decodeURIComponent(m[1]));
		return { ok: true };
	}],
	["DELETE", /^\/api\/profiles\/([^/]+)$/, (_req, m) => {
		if (m[1] === "default") throw new HttpError(400, "the default profile cannot be deleted");
		fs.rmSync(agentDir(decodeURIComponent(m[1])), { recursive: true, force: true });
		return { ok: true };
	}],
];

function listSkills(profile: string) {
	const dir = path.join(agentDir(profile), "skills");
	const out: { name: string; category: string; description: string; enabled: boolean }[] = [];
	const disabled = new Set<string>(readConfig(profile)?.skills?.disabled ?? []);
	const walk = (d: string, category: string, depth: number) => {
		if (depth > 3 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory() || e.name.startsWith(".")) continue;
			const f = path.join(d, e.name, "SKILL.md");
			if (fs.existsSync(f)) {
				let desc = "";
				try {
					desc = String(YAML.parse(fs.readFileSync(f, "utf8").match(/^---\n([\s\S]*?)\n---/)?.[1] ?? "")?.description ?? "");
				} catch {}
				out.push({ name: e.name, category, description: desc, enabled: !disabled.has(e.name) });
			} else walk(path.join(d, e.name), category ? `${category}/${e.name}` : e.name, depth + 1);
		}
	};
	walk(dir, "", 0);
	return out;
}

function setEnv(key: string, value: string | undefined): void {
	if (!/^[A-Z_][A-Z0-9_]*$/i.test(key)) throw new HttpError(400, "invalid key");
	storeEnv(key, value);
}

function dataUrl(url: string): Buffer {
	const m = String(url).match(/^data:[^;,]*(;base64)?,(.*)$/s);
	if (!m) throw new HttpError(400, "bad data_url");
	return m[1] ? Buffer.from(m[2], "base64") : Buffer.from(decodeURIComponent(m[2]));
}

function isDir(p: string): boolean {
	try {
		return fs.statSync(p).isDirectory();
	} catch {
		return false;
	}
}

function gitRoot(cwd: string): string | null {
	for (let d = cwd; d && d !== path.dirname(d); d = path.dirname(d)) if (fs.existsSync(path.join(d, ".git"))) return d;
	return null;
}

async function readBody(req: http.IncomingMessage): Promise<any> {
	const chunks: Buffer[] = [];
	for await (const c of req) chunks.push(c as Buffer);
	const text = Buffer.concat(chunks).toString("utf8");
	if (!text) return undefined;
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}

const server = http.createServer(async (req, res) => {
	const url = new URL(req.url ?? "/", "http://localhost");
	const send = (status: number, body: any) => {
		res.writeHead(status, { "Content-Type": "application/json" });
		res.end(JSON.stringify(body));
	};
	if (req.headers["x-hermes-session-token"] !== token && url.searchParams.get("token") !== token) return send(401, { detail: "unauthorized" });
	try {
		const r = req as Req;
		r.query = url.searchParams;
		r.path = url.pathname;
		r.body = await readBody(req);
		if (req.method === "GET" && url.pathname === "/api/fs/download") {
			const p = expand(url.searchParams.get("path") ?? "");
			const buf = fs.readFileSync(p).subarray(0, 50_000_000);
			res.writeHead(200, { "Content-Type": "application/octet-stream" });
			return res.end(buf);
		}
		const fixed = routes[`${req.method} ${url.pathname}`];
		if (fixed) return send(200, await fixed(r));
		for (const [method, re, handler] of dynamic) {
			const m = url.pathname.match(re);
			if (m && method === req.method) return send(200, await handler(r, m));
		}
		send(404, { detail: `no route: ${req.method} ${url.pathname}` });
	} catch (e) {
		const status = e instanceof HttpError ? e.status : (e as any)?.code === "ENOENT" ? 404 : 500;
		send(status, { detail: String((e as Error)?.message ?? e) });
	}
});

const wss = new WebSocketServer({ noServer: true });
server.on("upgrade", (req, socket, head) => {
	const url = new URL(req.url ?? "/", "http://localhost");
	if (url.pathname !== "/api/ws" || url.searchParams.get("token") !== token) {
		socket.write("HTTP/1.1 403 Forbidden\r\n\r\n");
		return socket.destroy();
	}
	wss.handleUpgrade(req, socket, head, (ws) => {
		sockets.add(ws);
		ws.on("close", () => sockets.delete(ws));
		ws.on("message", async (data) => {
			let msg: any;
			try {
				msg = JSON.parse(String(data));
			} catch {
				return;
			}
			const handler = methods[msg.method];
			try {
				if (!handler) throw new Error(`method not found: ${msg.method}`);
				const result = await handler(msg.params ?? {});
				ws.send(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: result ?? {} }));
			} catch (e) {
				ws.send(JSON.stringify({ jsonrpc: "2.0", id: msg.id, error: { code: -32000, message: String((e as Error)?.message ?? e) } }));
			}
		});
	});
});

// ---- cron ----------------------------------------------------------------

/** A job as Bench's cron list reads it. */
function cronView(j: Job) {
	return {
		id: j.id,
		name: j.name,
		prompt: j.prompt,
		schedule: { expr: j.schedule, display: describe(j.schedule) },
		schedule_display: describe(j.schedule),
		enabled: j.enabled,
		state: j.enabled ? "scheduled" : "paused",
		next_run_at: j.next_run ? new Date(j.next_run).toISOString() : null,
		last_run_at: j.last_run ? new Date(j.last_run).toISOString() : null,
		last_status: j.last_status,
		last_output: (j.last_output ?? "").slice(0, 400),
		last_session: j.last_session,
		deliver: j.deliver,
	};
}

// ---- start ---------------------------------------------------------------

server.listen(argPort || prior.port || 0, "127.0.0.1", () => {
	const port = (server.address() as any).port;
	fs.mkdirSync(AGENT_HOME, { recursive: true });
	fs.writeFileSync(STATE, JSON.stringify({ port, token, pid: process.pid, version: pkg.version, build: process.env.BENCH_AGENT_BUILD ?? null }), { mode: 0o600 });
	console.log(`BENCH_AGENT_READY port=${port}`);
});
server.on("error", (e: any) => {
	// The saved port is taken: take any free one.
	if (e.code === "EADDRINUSE" && !argPort) server.listen(0, "127.0.0.1");
	else throw e;
});

void syncTelegram(rt).catch((e) => console.error("[telegram]", e));

if (!process.env.BENCH_AGENT_NO_MCP) void connectMcp("default").then(() => {
	rt.reloadMcp();
	console.log(`[mcp] ${mcpStatus().map((s) => `${s.agent}/${s.name}:${s.error ? "error" : s.tools}`).join(" ")}`);
});

for (const sig of ["SIGINT", "SIGTERM"])
	process.on(sig, async () => {
		process.exit(0);
	});
