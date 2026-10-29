// A project's environment as one file, for a backup or a move to another project (another
// machine, another agent): its soul, prompts, skills, MCP and language servers, its settings
// (compaction, approvals, the step limit), memories, extensions, the MCP servers' sign-ins, its
// scheduled jobs and its Telegram bot. Not its threads and not its work files (`files/`, its
// folder): those stay where the work is.
//
// Exported, it is JSON Bench saves where the person says; imported into a project, everything in
// it is added and what has the same name (a file, a server, a job) is replaced.

import fs from "node:fs";
import path from "node:path";
import { AGENT_HOME, getSetting, setSetting } from "../config.ts";
import { arm, jobs, newJob } from "../builtin/cron.ts";
import { projects } from "../builtin/projects.ts";
import { HttpError, type Ctx, type Module } from "../http.ts";
import { reconnectTelegram, syncTelegram } from "../telegram.ts";
import { envOf } from "./projects.ts";

/** What the file says it is; anything else is refused on import. */
const KIND = "bench.environment";
const VERSION = 1;

/** The environment's entries a file carries; the rest of its folder (work files, caches) stays. */
const ENTRIES = ["SYSTEM.md", "settings.json", "mcp.json", "lsp.json", "skills", "prompts", "memories", "extensions"];
/** Never carried, wherever they are. */
const SKIP = new Set([".DS_Store", "node_modules", ".git", "__pycache__"]);
/** A bigger file is left out (and said so): the environment is texts and small scripts. */
const MAX_FILE = 8 * 1024 * 1024;
/** The keys of the general agent's settings that are the machine's, not the environment's. */
const MACHINE_KEYS = ["defaultProvider", "defaultModel"];

export interface Bundle {
	kind: typeof KIND;
	version: number;
	/** The project it was exported from, as it was named. */
	name: string;
	exported: string;
	/** Path in the environment → its text, or its bytes in base64 when it is not text. */
	files: Record<string, { text: string } | { base64: string }>;
	/** The MCP servers' sign-ins: server name → pi-mcp's credentials file. */
	oauth: Record<string, unknown>;
	cron: { name: string; prompt: string; schedule: string; deliver: string; enabled: boolean }[];
	telegram: { token: string; allowed_users: string[]; proxy?: string } | null;
	/** Files left out (too big, unreadable). */
	skipped?: string[];
}

/** What an import did, for Bench to say. */
export interface Imported {
	files: number;
	skills: number;
	prompts: number;
	mcp: number;
	lsp: number;
	cron: number;
	telegram: boolean;
	/** The bot was another project's here: it moved (a bot answers in one place). */
	bot_moved_from?: string;
}

const oauthPrefix = (agent: string) => `${agent.replace(/[^\w.-]/g, "_")}__`;
const oauthDir = () => path.join(AGENT_HOME, "mcp-oauth");

function walk(root: string, rel: string, out: Bundle["files"], skipped: string[]): void {
	const full = path.join(root, rel);
	let st: fs.Stats;
	try {
		st = fs.statSync(full);
	} catch {
		return;
	}
	if (st.isDirectory()) {
		for (const n of fs.readdirSync(full).sort()) if (!SKIP.has(n)) walk(root, path.posix.join(rel, n), out, skipped);
		return;
	}
	if (!st.isFile()) return;
	if (st.size > MAX_FILE) return void skipped.push(rel);
	const bytes = fs.readFileSync(full);
	const text = bytes.toString("utf8");
	// Text when it reads back as the same bytes; anything else as base64.
	out[rel] = Buffer.from(text, "utf8").equals(bytes) && !text.includes("\0") ? { text } : { base64: bytes.toString("base64") };
}

/** The project `id`'s environment as a file. */
export function pack(id: string): Bundle {
	const { agent, dir, project } = envOf(id);
	const files: Bundle["files"] = {};
	const skipped: string[] = [];
	for (const e of ENTRIES) walk(dir, e, files, skipped);
	// The model is the machine's (the gear), never a project's.
	const settings = files["settings.json"];
	if (settings && "text" in settings) {
		try {
			const s = JSON.parse(settings.text);
			for (const k of MACHINE_KEYS) delete s[k];
			files["settings.json"] = { text: JSON.stringify(s, null, 2) + "\n" };
		} catch {}
	}
	const oauth: Bundle["oauth"] = {};
	const prefix = oauthPrefix(agent);
	if (fs.existsSync(oauthDir()))
		for (const f of fs.readdirSync(oauthDir()))
			if (f.startsWith(prefix) && f.endsWith(".json"))
				try {
					oauth[f.slice(prefix.length, -5)] = JSON.parse(fs.readFileSync(path.join(oauthDir(), f), "utf8"));
				} catch {}
	const tg = project.telegram;
	return {
		kind: KIND,
		version: VERSION,
		name: project.name,
		exported: new Date().toISOString(),
		files,
		oauth,
		cron: jobs.list(agent).map((j) => ({ name: j.name, prompt: j.prompt, schedule: j.schedule, deliver: j.deliver, enabled: j.enabled })),
		telegram: tg?.token ? { token: tg.token, allowed_users: tg.allowed_users ?? [], proxy: tg.proxy } : null,
		...(skipped.length ? { skipped } : {}),
	};
}

/** A JSON file's servers object (`mcpServers` of mcp.json, `servers` of lsp.json). */
function servers(text: string | undefined, key: string): Record<string, unknown> {
	try {
		return text ? (JSON.parse(text)[key] ?? {}) : {};
	} catch {
		return {};
	}
}

/** Two JSON files with servers in one: the incoming one's win by name. */
function mergeServers(dir: string, file: string, key: string, incoming: string): number {
	const here = path.join(dir, file);
	const theirs = servers(incoming, key);
	const ours = servers(fs.existsSync(here) ? fs.readFileSync(here, "utf8") : undefined, key);
	let whole: any = {};
	try {
		whole = JSON.parse(fs.existsSync(here) ? fs.readFileSync(here, "utf8") : "{}");
	} catch {}
	whole[key] = { ...ours, ...theirs };
	fs.writeFileSync(here, JSON.stringify(whole, null, 2) + "\n");
	return Object.keys(theirs).length;
}

/** A path of the file, kept inside the environment. */
function inside(dir: string, rel: string): string {
	const full = path.resolve(dir, rel);
	if (!ENTRIES.some((e) => full === path.join(dir, e) || full.startsWith(path.join(dir, e) + path.sep))) throw new HttpError(400, `the file has a path outside the environment: ${rel}`);
	return full;
}

/** Check a file is an environment Bench wrote. */
export function check(b: any): asserts b is Bundle {
	if (b?.kind !== KIND) throw new HttpError(400, "this is not a Bench environment file");
	if (b.version > VERSION) throw new HttpError(400, "the file is from a newer Bench: update this one");
	if (typeof b.files !== "object") throw new HttpError(400, "the file has no files");
}

/** The file `b` into the project `id`'s environment. */
export async function unpack(id: string, b: Bundle, rt: Ctx["rt"]): Promise<Imported> {
	check(b);
	const { agent, dir, project } = envOf(id);
	fs.mkdirSync(dir, { recursive: true });
	const done: Imported = { files: 0, skills: 0, prompts: 0, mcp: 0, lsp: 0, cron: 0, telegram: false };
	const skills = new Set<string>();
	for (const [rel, f] of Object.entries(b.files)) {
		const full = inside(dir, rel);
		const text = "text" in f ? f.text : undefined;
		if (rel === "mcp.json") {
			done.mcp = mergeServers(dir, rel, "mcpServers", text ?? "");
			continue;
		}
		if (rel === "lsp.json") {
			done.lsp = mergeServers(dir, rel, "servers", text ?? "");
			continue;
		}
		if (rel === "settings.json") {
			// Its settings over ours, the machine's keys left as they are.
			let ours: any = {};
			let theirs: any = {};
			try {
				ours = JSON.parse(fs.readFileSync(full, "utf8"));
			} catch {}
			try {
				theirs = JSON.parse(text ?? "{}");
			} catch {}
			for (const k of MACHINE_KEYS) delete theirs[k];
			fs.writeFileSync(full, JSON.stringify({ ...ours, ...theirs }, null, 2) + "\n");
			continue;
		}
		fs.mkdirSync(path.dirname(full), { recursive: true });
		fs.writeFileSync(full, text !== undefined ? text : Buffer.from((f as { base64: string }).base64, "base64"));
		done.files++;
		if (rel.startsWith("skills/")) skills.add(rel.split("/")[1]!);
		if (rel.startsWith("prompts/") && rel.endsWith(".md")) done.prompts++;
	}
	done.skills = skills.size;
	// The MCP servers signed in as they were.
	const oauth = Object.entries(b.oauth ?? {});
	if (oauth.length) {
		fs.mkdirSync(oauthDir(), { recursive: true });
		for (const [name, cred] of oauth) fs.writeFileSync(path.join(oauthDir(), `${oauthPrefix(agent)}${name.replace(/[^\w.-]/g, "_")}.json`), JSON.stringify(cred, null, 2));
	}
	if (done.mcp || oauth.length) rt.reloadMcp(agent);
	// Its jobs: one of the same name is replaced.
	for (const c of b.cron ?? []) {
		const old = jobs.list(agent).find((j) => j.name === c.name);
		let job;
		try {
			job = { ...newJob({ agent, name: c.name, prompt: c.prompt, schedule: c.schedule, deliver: c.deliver }), ...(old ? { id: old.id, created: old.created } : {}) };
		} catch {
			continue;
		}
		if (!c.enabled) job.enabled = false;
		jobs.save(job);
		if (job.enabled) await arm(job);
		done.cron++;
	}
	// Its bot. A bot answers in one place: one of another project here moves to this one.
	if (b.telegram?.token) {
		const tg = { token: b.telegram.token, allowed_users: (b.telegram.allowed_users ?? []).map(String), proxy: b.telegram.proxy || undefined };
		const other = projects.list().find((p) => p.id !== project.id && p.telegram?.token === tg.token);
		if (other) {
			projects.save({ ...other, telegram: null });
			done.bot_moved_from = other.name;
		}
		if (project.id === "general") {
			setSetting("telegram", { ...(getSetting("telegram") ?? {}), ...tg });
		} else {
			if (!other && getSetting("telegram")?.token === tg.token) {
				setSetting("telegram", undefined);
				done.bot_moved_from = "Host";
			}
			projects.save({ ...(projects.get(project.id) as any), telegram: tg });
		}
		reconnectTelegram(project.id === "general" ? "" : project.id);
		await syncTelegram(rt);
		done.telegram = true;
	}
	return done;
}

const id1 = (m: RegExpMatchArray) => decodeURIComponent(m[1]!);

export default ({ rt, publish }: Ctx): Module => ({
	dynamic: [
		["GET", /^\/api\/projects\/([^/]+)\/export$/, (_req, m) => pack(id1(m))],
		[
			"POST",
			/^\/api\/projects\/([^/]+)\/import$/,
			async (req, m) => {
				const project = id1(m);
				const done = await unpack(project, req.body, rt);
				publish({ type: "env_changed", project });
				return done;
			},
		],
	],
});
