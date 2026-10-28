// Bench's own home: ~/.bench (BENCH_HOME).
//
//   bench.sqlite          settings, keys (env), model providers, the session index
//   pi.sqlite             Pi Durable's conversations
//   agents/<name>/        an agent, laid out like Pi's agent directory (see below)
//   cron/, attachments/, cache/, server.json, server.log, app/ (the installed server)
//
// `readConfig()` answers in one config object assembled from the settings and the agent, for the
// tools that read it.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

export const BENCH_HOME = process.env.BENCH_HOME ?? path.join(os.homedir(), ".bench");
/** Kept as a name for the server's own files; it is the same folder. */
export const AGENT_HOME = BENCH_HOME;

let database: DatabaseSync | undefined;

export function db(): DatabaseSync {
	if (database) return database;
	fs.mkdirSync(BENCH_HOME, { recursive: true });
	database = new DatabaseSync(path.join(BENCH_HOME, "bench.sqlite"));
	database.exec(`
		PRAGMA journal_mode = WAL;
		CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
		CREATE TABLE IF NOT EXISTS env (key TEXT PRIMARY KEY, value TEXT NOT NULL);
		CREATE TABLE IF NOT EXISTS providers (
			id TEXT PRIMARY KEY, name TEXT, base_url TEXT NOT NULL, api_key TEXT, api_mode TEXT DEFAULT 'chat_completions',
			models TEXT DEFAULT '{}', context_length INTEGER
		);
	`);
	try {
		database.exec("ALTER TABLE providers ADD COLUMN parallel INTEGER");
	} catch {}
	return database;
}

// ---- settings ----------------------------------------------------------------

export function getSetting<T = any>(key: string, fallback?: T): T {
	const r = db().prepare("SELECT value FROM settings WHERE key = ?").get(key) as any;
	return r ? JSON.parse(r.value) : (fallback as T);
}

export function setSetting(key: string, value: unknown): void {
	if (value === undefined || value === null) db().prepare("DELETE FROM settings WHERE key = ?").run(key);
	else db().prepare("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(key, JSON.stringify(value));
}

export function allSettings(): Record<string, any> {
	return Object.fromEntries((db().prepare("SELECT key, value FROM settings").all() as any[]).map((r) => [r.key, JSON.parse(r.value)]));
}

// ---- keys ------------------------------------------------------------------

export function readEnv(_profile?: string): Record<string, string> {
	return Object.fromEntries((db().prepare("SELECT key, value FROM env").all() as any[]).map((r) => [r.key, r.value]));
}

export function setEnv(key: string, value: string | undefined): void {
	if (value === undefined) db().prepare("DELETE FROM env WHERE key = ?").run(key);
	else db().prepare("INSERT INTO env (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(key, value);
}

/** The environment a profile's tools and providers see: the process env, then the stored keys. */
export function profileEnv(profile: string): Record<string, string> {
	return { ...(process.env as Record<string, string>), ...readEnv(profile) };
}

// ---- providers ---------------------------------------------------------------

export interface Provider {
	id: string;
	name: string;
	base_url: string;
	api_key?: string;
	api_mode: string;
	/** model id -> { context_length? } */
	models: Record<string, { context_length?: number }>;
	context_length?: number;
	/** How many requests its server answers at once (a local model's slots); unset: as many as asked. */
	parallel?: number;
}

export function providers(): Provider[] {
	return (db().prepare("SELECT * FROM providers ORDER BY id").all() as any[]).map((r) => ({ ...r, models: JSON.parse(r.models || "{}"), api_key: r.api_key ?? undefined, context_length: r.context_length ?? undefined, parallel: r.parallel ?? undefined }));
}

export function saveProvider(p: Provider): void {
	db()
		.prepare(
			`INSERT INTO providers (id, name, base_url, api_key, api_mode, models, context_length, parallel) VALUES (?,?,?,?,?,?,?,?)
			 ON CONFLICT(id) DO UPDATE SET name=excluded.name, base_url=excluded.base_url, api_key=excluded.api_key,
			 api_mode=excluded.api_mode, models=excluded.models, context_length=excluded.context_length, parallel=excluded.parallel`,
		)
		.run(p.id, p.name ?? p.id, p.base_url, p.api_key ?? null, p.api_mode ?? "chat_completions", JSON.stringify(p.models ?? {}), p.context_length ?? null, p.parallel ?? null);
}

export function deleteProvider(id: string): void {
	db().prepare("DELETE FROM providers WHERE id = ?").run(id);
}

// ---- agents ---------------------------------------------------------------------
//
// An agent is a folder laid out like Pi's agent directory (`~/.pi/agent`, docs/configuration.md):
//   settings.json   defaultProvider, defaultModel, defaultThinkingLevel; Bench adds extensions, approvals
//   mcp.json        { "mcpServers": { … } } — this agent's MCP servers, with Pi's `exposure`
//   SYSTEM.md       who it is (replaces the default identity); AGENTS.md: standing instructions
//   skills/, prompts/, extensions/   as in Pi; memories/ (MEMORY.md, USER.md) is Bench's
// The protocol still calls an agent a "profile" (Bench's wire names).

export interface AgentSettings {
	defaultProvider: string;
	defaultModel: string;
	defaultThinkingLevel?: string;
	/**
	 * When its conversations are summarized, as a share of the model's window (0.6: at 60%); unset:
	 * Pi's own policy. One number; the rest follows from it (`compactionPolicy`).
	 */
	compaction?: number;
	/** Extensions by name: this agent's extensions/<name>.ts or the built-in ones; default ["assistant"]. */
	extensions?: string[];
	/** "off" | "manual": ask before dangerous commands. */
	approvals?: string;
	/**
	 * Code mode: its threads also get Pi's `codemode` tool, a script that calls the other tools
	 * (`extensions.ts`). A project's choice; its threads take it from their next turn.
	 */
	codeMode?: boolean;
}

/** A project's environment is an agent directory too ("project:<id>"), in projects/<id>/. */
export const isProject = (agent: string): boolean => agent.startsWith("project:");

/** The keys only the general agent has: the model is one for every thread (the gear). */
const GLOBAL_KEYS = ["defaultProvider", "defaultModel"] as const;

/** What a project's threads share with every other thread: the general agent's memory, model, settings. */
export const homeAgent = (agent: string): string => (isProject(agent) ? "default" : agent);

export function agentDir(agent: string): string {
	if (isProject(agent)) return path.join(BENCH_HOME, "projects", agent.slice(8));
	return path.join(BENCH_HOME, "agents", agent || "default");
}

export function agents(): string[] {
	const dir = path.join(BENCH_HOME, "agents");
	const names = fs.existsSync(dir) ? fs.readdirSync(dir).filter((n) => fs.existsSync(path.join(dir, n, "settings.json"))) : [];
	return names.includes("default") ? ["default", ...names.filter((n) => n !== "default").sort()] : names.sort();
}

export function readAgent(agent: string): AgentSettings {
	// A project runs with the general agent's model and settings (its own settings.json may refine them).
	if (isProject(agent)) {
		let own: Partial<AgentSettings> = {};
		try {
			own = JSON.parse(fs.readFileSync(path.join(agentDir(agent), "settings.json"), "utf8"));
		} catch {}
		for (const k of GLOBAL_KEYS) delete own[k];
		return { ...readAgent("default"), ...own };
	}
	try {
		return JSON.parse(fs.readFileSync(path.join(agentDir(agent), "settings.json"), "utf8"));
	} catch {
		const first = providers()[0];
		return { defaultProvider: first?.id ?? "", defaultModel: Object.keys(first?.models ?? {})[0] ?? "" };
	}
}

export function writeAgent(agent: string, settings: AgentSettings): void {
	const dir = agentDir(agent);
	// A project's file never keeps the model: readAgent hands it the general one, written back here.
	if (isProject(agent)) {
		settings = { ...settings };
		for (const k of GLOBAL_KEYS) delete settings[k];
	}
	fs.mkdirSync(dir, { recursive: true });
	for (const sub of ["skills", "prompts", "extensions", "memories"]) fs.mkdirSync(path.join(dir, sub), { recursive: true });
	fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2) + "\n");
}

/** The agent's MCP servers (mcp.json's `mcpServers`). */
export function agentMcp(agent: string): Record<string, any> {
	try {
		return JSON.parse(fs.readFileSync(path.join(agentDir(agent), "mcp.json"), "utf8")).mcpServers ?? {};
	} catch {
		return {};
	}
}

export function writeAgentMcp(agent: string, servers: Record<string, any>): void {
	fs.mkdirSync(agentDir(agent), { recursive: true });
	fs.writeFileSync(path.join(agentDir(agent), "mcp.json"), JSON.stringify({ mcpServers: servers }, null, 2) + "\n");
}

/**
 * A project once had its own model; now every thread runs with the general agent's. The keys
 * leave the projects' settings.json, each file copied to backups/ first.
 */
export function migrateProjectModels(): string[] {
	const dir = path.join(BENCH_HOME, "projects");
	const log: string[] = [];
	if (!fs.existsSync(dir)) return log;
	for (const id of fs.readdirSync(dir)) {
		const file = path.join(dir, id, "settings.json");
		let own: any;
		try {
			own = JSON.parse(fs.readFileSync(file, "utf8"));
		} catch {
			continue;
		}
		if (!GLOBAL_KEYS.some((k) => k in own)) continue;
		const backup = path.join(BENCH_HOME, "backups", `project-model-${new Date().toISOString().slice(0, 10)}`, id);
		fs.mkdirSync(backup, { recursive: true });
		fs.copyFileSync(file, path.join(backup, "settings.json"));
		for (const k of GLOBAL_KEYS) delete own[k];
		fs.writeFileSync(file, JSON.stringify(own, null, 2) + "\n");
		log.push(id);
	}
	return log;
}

/** The first layout (profiles/<name>/profile.json, SOUL.md, a global mcp_servers setting) moves to agents/. */
export function migrateProfiles(): string[] {
	const old = path.join(BENCH_HOME, "profiles");
	if (!fs.existsSync(old)) return [];
	const log: string[] = [];
	const globalMcp = getSetting<Record<string, any>>("mcp_servers", {}) ?? {};
	for (const name of fs.readdirSync(old)) {
		const from = path.join(old, name);
		if (!fs.statSync(from).isDirectory() || fs.existsSync(path.join(agentDir(name), "settings.json"))) continue;
		fs.mkdirSync(path.dirname(agentDir(name)), { recursive: true });
		fs.renameSync(from, agentDir(name));
		const dir = agentDir(name);
		let p: any = {};
		try {
			p = JSON.parse(fs.readFileSync(path.join(dir, "profile.json"), "utf8"));
		} catch {}
		writeAgent(name, { defaultProvider: p.provider ?? "", defaultModel: p.model ?? "", ...(p.reasoning_effort ? { defaultThinkingLevel: p.reasoning_effort } : {}), extensions: p.extensions ?? ["assistant"], ...(p.approvals ? { approvals: p.approvals } : {}) });
		fs.rmSync(path.join(dir, "profile.json"), { force: true });
		if (fs.existsSync(path.join(dir, "SOUL.md")) && !fs.existsSync(path.join(dir, "SYSTEM.md"))) fs.renameSync(path.join(dir, "SOUL.md"), path.join(dir, "SYSTEM.md"));
		if (!fs.existsSync(path.join(dir, "mcp.json"))) writeAgentMcp(name, globalMcp);
		log.push(name);
	}
	if (!fs.readdirSync(old).length) fs.rmdirSync(old);
	setSetting("mcp_servers", undefined);
	return log;
}

// ---- the config view the tool ports read --------------------------------------

/** One config object, assembled from the settings and the agent. */
export function readConfig(profile: string): any {
	const s = allSettings();
	const p = readAgent(profile);
	const provs = providers();
	return {
		...s,
		model: { provider: p.defaultProvider, default: p.defaultModel },
		mcp_servers: agentMcp(profile),
		providers: Object.fromEntries(provs.map((x) => [x.id, { name: x.name, base_url: x.base_url, api_key: x.api_key, api_mode: x.api_mode, models: x.models, context_length: x.context_length }])),
		custom_providers: provs.map((x) => ({ name: x.id, base_url: x.base_url, model: Object.keys(x.models)[0] })),
		agent: { ...(s.agent ?? {}), reasoning_effort: p.defaultThinkingLevel ?? s.agent?.reasoning_effort ?? "medium" },
		approvals: { ...(s.approvals ?? {}), mode: p.approvals ?? s.approvals?.mode ?? "off" },
	};
}

/** Deep merge (objects merge, everything else replaces). */
export function deepMerge(base: any, patch: any): any {
	if (typeof base !== "object" || base === null || Array.isArray(base)) return patch;
	if (typeof patch !== "object" || patch === null || Array.isArray(patch)) return patch;
	const out = { ...base };
	for (const [k, v] of Object.entries(patch)) out[k] = deepMerge(base[k], v);
	return out;
}

export interface ModelChoice {
	provider: string;
	model: string;
	baseUrl: string;
	apiKey?: string;
	contextLength: number;
	/** "chat_completions" (OpenAI-compatible) or "anthropic_messages". */
	apiMode: string;
}

/** The model every thread runs with: the general agent's (the gear), the same for every project. */
export function resolveModel(_profile?: string): ModelChoice {
	const p = readAgent("default");
	return choiceOf(p.defaultProvider, p.defaultModel);
}

/** The model that answers when the main one fails, if one is set: the gear's `fallback_providers`, first one. */
export function resolveFallback(): ModelChoice | undefined {
	const f = (getSetting<any[]>("fallback_providers", []) ?? [])[0];
	if (!f?.model || !providers().some((x) => x.id === f.provider)) return undefined;
	return choiceOf(String(f.provider), String(f.model));
}

function choiceOf(providerId: string, model: string): ModelChoice {
	const prov = providers().find((x) => x.id === providerId) ?? providers()[0];
	const ctx = prov?.models?.[model]?.context_length ?? prov?.context_length ?? 131072;
	return {
		provider: prov?.id ?? providerId,
		model,
		baseUrl: String(prov?.base_url ?? "").replace(/\/$/, ""),
		apiKey: prov?.api_key || undefined,
		contextLength: Number(ctx) || 131072,
		apiMode: prov?.api_mode ?? "chat_completions",
	};
}

export function reasoningEffort(profile: string): string {
	return readAgent(profile).defaultThinkingLevel ?? getSetting<{ reasoning_effort?: string }>("agent", {})?.reasoning_effort ?? "medium";
}

/** A profile's compaction share, if it set one that makes sense. */
export function compactionAt(profile: string): number | undefined {
	const v = readAgent(profile).compaction;
	return typeof v === "number" && v >= 0.4 && v <= 0.95 ? v : undefined;
}

/**
 * Pi's compaction policy from one share of the model's window, so it holds when the model (and
 * its window) changes: the summary is made at `at` of the window (a background one starts 5% of the
 * window earlier), and 40% of what was there stays word for word — a code project's files are
 * still in front of the model after it.
 */
export function compactionPolicy(at: number, window: number): { reserveTokens: number; backgroundTokens: number; keepRecentTokens: number } {
	const threshold = Math.floor(window * at);
	return {
		reserveTokens: Math.max(4096, window - threshold),
		backgroundTokens: Math.floor(window * 0.05),
		keepRecentTokens: Math.floor(threshold * 0.4),
	};
}

export function approvalsMode(profile: string): string {
	return readAgent(profile).approvals ?? getSetting<{ mode?: string }>("approvals", {})?.mode ?? "off";
}

export function timezone(_profile?: string): string {
	return getSetting("timezone") || Intl.DateTimeFormat().resolvedOptions().timeZone;
}
