// One-time move from Hermes to Bench's own home. Runs at the server's first start when
// ~/.hermes exists (and again with `node src/import-hermes.ts --force`); never writes to ~/.hermes.
//
//   config.yaml   → settings (agent, memory, mcp_servers, stt, tts, web, timezone, …) and providers
//   .env          → keys (env); API keys of the providers go into their rows
//   SOUL.md, memories/, skills/ (only the ones that do not ship with Hermes), for every profile
//   profiles/<name>/cron/jobs.json → cron jobs, paused (Hermes may still be running them)
//   ~/.bench-agent (the first bench-agent's home) → sessions, transcripts, Telegram bot

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import YAML from "yaml";
import { agentDir, db, getSetting, HERMES_ROOT, saveProvider, setEnv, setSetting, writeAgent, writeAgentMcp, type Provider } from "./config.ts";
import { BENCH_HOME } from "./config.ts";
import { jobs } from "./builtin/cron.ts";
import { Store } from "./store.ts";

const SETTINGS = ["agent", "memory", "skills", "stt", "tts", "web", "timezone", "terminal", "tool_output", "file_read_max_chars", "auxiliary", "tools", "approvals", "compression"];

function parseEnv(text: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const raw of text.split("\n")) {
		const line = raw.trim().replace(/^export\s+/, "");
		if (!line || line.startsWith("#")) continue;
		const eq = line.indexOf("=");
		if (eq <= 0) continue;
		let v = line.slice(eq + 1).trim();
		if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) v = v.slice(1, -1);
		else v = v.replace(/\s+#.*$/, "");
		out[line.slice(0, eq).trim()] = v;
	}
	return out;
}

const read = (f: string) => (fs.existsSync(f) ? fs.readFileSync(f, "utf8") : "");

/** Skill folder names that ship with Hermes (its bundled and optional sets). */
function bundledSkills(): Set<string> {
	const names = new Set<string>();
	const walk = (d: string, depth: number) => {
		if (depth > 4 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory()) continue;
			if (fs.existsSync(path.join(d, e.name, "SKILL.md"))) names.add(e.name);
			else walk(path.join(d, e.name), depth + 1);
		}
	};
	for (const sub of ["skills", "optional-skills"]) walk(path.join(HERMES_ROOT, "hermes-agent", sub), 0);
	return names;
}

/** Copies the profile's own skills, keeping their category folders (and the categories' DESCRIPTION.md). */
function copySkills(from: string, to: string, bundled: Set<string>): string[] {
	const kept: string[] = [];
	const walk = (d: string, rel: string, depth: number) => {
		if (depth > 4 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory() || e.name.startsWith(".")) continue;
			const src = path.join(d, e.name);
			if (fs.existsSync(path.join(src, "SKILL.md"))) {
				if (bundled.has(e.name)) continue;
				const dst = path.join(to, rel, e.name);
				fs.mkdirSync(path.dirname(dst), { recursive: true });
				fs.cpSync(src, dst, { recursive: true });
				const desc = path.join(d, "DESCRIPTION.md");
				if (rel && fs.existsSync(desc)) fs.copyFileSync(desc, path.join(to, rel, "DESCRIPTION.md"));
				kept.push(path.join(rel, e.name));
			} else walk(src, path.join(rel, e.name), depth + 1);
		}
	};
	walk(from, "", 0);
	return kept;
}

export function importHermes(force = false): string[] {
	const log: string[] = [];
	if (!fs.existsSync(HERMES_ROOT)) return log;
	if (getSetting("imported_from_hermes") && !force) return log;
	new Store(); // its tables, before the old index is copied into them
	const cfg: any = YAML.parse(read(path.join(HERMES_ROOT, "config.yaml")) || "{}") ?? {};
	const env = parseEnv(read(path.join(HERMES_ROOT, ".env")));

	for (const k of SETTINGS) if (cfg[k] !== undefined) setSetting(k, cfg[k]);
	for (const [k, v] of Object.entries(env)) setEnv(k, v);
	log.push(`settings: ${SETTINGS.filter((k) => cfg[k] !== undefined).length}, keys: ${Object.keys(env).length}`);

	// Providers: config `providers:` and `custom_providers:`, keys resolved from .env.
	const seen = new Map<string, Provider>();
	const keyOf = (p: any) => (p?.key_env && env[p.key_env]) || p?.api_key || undefined;
	for (const [id, p] of Object.entries<any>(cfg.providers ?? {})) {
		const models = Array.isArray(p.models) ? Object.fromEntries(p.models.map((m: string) => [m, {}])) : (p.models ?? {});
		if (p.model && !models[p.model]) models[p.model] = {};
		seen.set(id, { id, name: p.name ?? id, base_url: p.base_url ?? p.api, api_key: keyOf(p), api_mode: p.api_mode ?? "chat_completions", models, context_length: p.context_length });
	}
	for (const p of cfg.custom_providers ?? []) {
		if (!p?.name || seen.has(p.name)) continue;
		seen.set(p.name, { id: p.name, name: p.name, base_url: p.base_url, api_key: keyOf(p), api_mode: "chat_completions", models: p.model ? { [p.model]: {} } : {} });
	}
	if (env.OPENROUTER_API_KEY && !seen.has("openrouter"))
		seen.set("openrouter", { id: "openrouter", name: "OpenRouter", base_url: "https://openrouter.ai/api/v1", api_key: env.OPENROUTER_API_KEY, api_mode: "chat_completions", models: Object.fromEntries((cfg.fallback_providers ?? []).filter((f: any) => f.provider === "openrouter").map((f: any) => [f.model, {}])) });
	for (const p of seen.values()) if (p.base_url) saveProvider(p);
	log.push(`providers: ${[...seen.keys()].join(", ")}`);

	// Profiles: default (the root) and profiles/*.
	const bundled = bundledSkills();
	const homes: [string, string][] = [["default", HERMES_ROOT]];
	const pdir = path.join(HERMES_ROOT, "profiles");
	if (fs.existsSync(pdir)) for (const n of fs.readdirSync(pdir)) if (!n.startsWith(".") && fs.statSync(path.join(pdir, n)).isDirectory()) homes.push([n, path.join(pdir, n)]);
	for (const [name, home] of homes) {
		const pc: any = name === "default" ? cfg : (YAML.parse(read(path.join(home, "config.yaml")) || "{}") ?? {});
		const m = typeof pc.model === "string" ? { default: pc.model } : (pc.model ?? cfg.model ?? {});
		const effort = pc.agent?.reasoning_effort ?? cfg.agent?.reasoning_effort;
		writeAgent(name, { defaultProvider: m.provider ?? "", defaultModel: m.default ?? m.name ?? "", ...(effort ? { defaultThinkingLevel: effort } : {}), extensions: ["assistant"] });
		const dst = agentDir(name);
		const soul = read(path.join(home, "SOUL.md"));
		if (soul) fs.writeFileSync(path.join(dst, "SYSTEM.md"), soul);
		writeAgentMcp(name, (name === "default" ? cfg.mcp_servers : pc.mcp_servers) ?? {});
		for (const f of ["MEMORY.md", "USER.md"]) {
			const t = read(path.join(home, "memories", f));
			if (t) fs.writeFileSync(path.join(dst, "memories", f), t);
		}
		const kept = copySkills(path.join(home, "skills"), path.join(dst, "skills"), bundled);
		if (fs.existsSync(path.join(home, "prompts"))) fs.cpSync(path.join(home, "prompts"), path.join(dst, "prompts"), { recursive: true });
		log.push(`profile ${name}: ${m.default ?? "?"}, ${kept.length} skills`);

		// Cron jobs, paused.
		const jobsFile = path.join(home, "cron", "jobs.json");
		if (fs.existsSync(jobsFile)) {
			const d = JSON.parse(read(jobsFile));
			const jobs = (Array.isArray(d) ? d : (d.jobs ?? [])).map((j: any) => ({ ...j, profile: name, enabled: false, paused_reason: "imported from Hermes" }));
			const ours = path.join(BENCH_HOME, "cron", "jobs.json");
			fs.mkdirSync(path.dirname(ours), { recursive: true });
			const prev = fs.existsSync(ours) ? JSON.parse(read(ours)) : { jobs: [] };
			const ids = new Set((prev.jobs ?? []).map((j: any) => j.id));
			prev.jobs = [...(prev.jobs ?? []), ...jobs.filter((j: any) => !ids.has(j.id))];
			fs.writeFileSync(ours, JSON.stringify(prev, null, 2));
			log.push(`cron ${name}: ${jobs.length} jobs (paused)`);
		}
	}

	// The first bench-agent's home: its conversations, session index and Telegram bot.
	const old = path.join(os.homedir(), ".bench-agent");
	if (fs.existsSync(path.join(old, "pi.sqlite")) && !fs.existsSync(path.join(BENCH_HOME, "pi.sqlite"))) {
		for (const f of ["pi.sqlite", "pi.sqlite-wal", "pi.sqlite-shm"]) if (fs.existsSync(path.join(old, f))) fs.copyFileSync(path.join(old, f), path.join(BENCH_HOME, f));
		log.push("conversations: pi.sqlite");
	}
	if (fs.existsSync(path.join(old, "index.sqlite"))) {
		const d = db();
		d.exec(`ATTACH DATABASE '${path.join(old, "index.sqlite").replace(/'/g, "''")}' AS old`);
		try {
			d.exec("CREATE TABLE IF NOT EXISTS sessions AS SELECT * FROM old.sessions WHERE 0");
			d.exec("CREATE TABLE IF NOT EXISTS messages AS SELECT * FROM old.messages WHERE 0");
			d.exec("INSERT OR IGNORE INTO sessions SELECT * FROM old.sessions");
			d.exec("INSERT INTO messages SELECT * FROM old.messages WHERE session_id NOT IN (SELECT DISTINCT session_id FROM messages)");
			log.push("session index: copied");
		} finally {
			d.exec("DETACH DATABASE old");
		}
	}
	const tg = path.join(old, "telegram.json");
	if (fs.existsSync(tg) && !getSetting("telegram")) setSetting("telegram", JSON.parse(read(tg)));
	else if (!getSetting("telegram") && env.TELEGRAM_ALLOWED_USERS) setSetting("telegram", { token: "", allowed_users: env.TELEGRAM_ALLOWED_USERS.split(",").map((s) => s.trim()).filter(Boolean), profile: "default" });

	setSetting("imported_from_hermes", { at: new Date().toISOString(), log });
	return log;
}

if (import.meta.url === `file://${process.argv[1]}`) {
	for (const line of importHermes(process.argv.includes("--force"))) console.log(line);
}

/** Jobs the first import left in cron/jobs.json (Hermes' format) move into the jobs table, paused. */
export function migrateCronFile(): void {
	const file = path.join(BENCH_HOME, "cron", "jobs.json");
	if (!fs.existsSync(file)) return;
	const d = JSON.parse(read(file));
	for (const j of Array.isArray(d) ? d : (d.jobs ?? [])) {
		if (jobs.get(j.id)) continue;
		const expr = typeof j.schedule === "object" ? (j.schedule?.expr ?? j.schedule?.display) : j.schedule;
		const chat = j.origin?.platform === "telegram" && j.origin?.chat_id ? `telegram:${j.origin.chat_id}` : "";
		jobs.save({ id: j.id, profile: j.profile ?? "default", name: j.name ?? "job", prompt: j.prompt ?? "", schedule: String(expr ?? ""), deliver: chat, enabled: false, next_run: null, last_run: null, last_status: "imported from Hermes, paused", last_output: null, last_session: null, task_id: null, created: Date.now() });
	}
	fs.renameSync(file, file + ".imported");
}
