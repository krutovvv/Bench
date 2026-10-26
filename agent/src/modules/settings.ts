// The gear and the agent's own settings: providers and models, profiles, the config, keys in
// the environment, the general skills and MCP servers, the agent's status.

import fs from "node:fs";
import path from "node:path";
import YAML from "yaml";
import pkg from "../../package.json" with { type: "json" };
import { agentDir, agents, deepMerge, deleteProvider, getSetting, providers, readAgent, readConfig, readEnv, resolveModel, saveProvider, setEnv as storeEnv, setSetting, writeAgent, writeAgentMcp } from "../config.ts";
import { findSkill, listSkills as skillsOf } from "../builtin/skills.ts";
import { HttpError, profileOf, type Ctx, type Module } from "../http.ts";
import { connectMcp, mcpStatus } from "../mcp.ts";

/**
 * `PUT /api/config`: each top-level key merges into its setting. The reasoning level and the
 * approvals are the agent's own (a project's thread reads `project:<id>`); the model is one for
 * every thread and goes to the general agent; the fallback is a setting like the rest.
 */
function patchConfig(profile: string, patch: any): void {
	for (const [k, v] of Object.entries(patch ?? {})) {
		const m = v as any;
		if (k === "model" && m && typeof m === "object") {
			const p = readAgent("default");
			writeAgent("default", { ...p, defaultProvider: m.provider ?? p.defaultProvider, defaultModel: m.default ?? m.name ?? p.defaultModel });
		} else if (k === "agent" && m && typeof m === "object" && m.reasoning_effort) {
			writeAgent(profile, { ...readAgent(profile), defaultThinkingLevel: String(m.reasoning_effort) });
		} else if (k === "approvals" && m && typeof m === "object" && m.mode !== undefined) {
			writeAgent(profile, { ...readAgent(profile), approvals: String(m.mode) });
		} else setSetting(k, deepMerge(getSetting(k), v));
	}
}

function listSkills(profile: string) {
	const disabled = new Set<string>(readConfig(profile)?.skills?.disabled ?? []);
	return skillsOf(profile).map((k) => ({ name: k.name, category: k.category, description: k.description, enabled: !disabled.has(k.name) }));
}

function setEnv(key: string, value: string | undefined): void {
	if (!/^[A-Z_][A-Z0-9_]*$/i.test(key)) throw new HttpError(400, "invalid key");
	storeEnv(key, value);
}

export default ({ rt, broadcast }: Ctx): Module => ({
	rpc: {
		"profiles.list": () => ({
			profiles: agents().map((name) => ({ name, display_name: name, model: resolveModel(name).model })),
		}),
		"config.set": async ({ key, value, session_id, profile }) => {
			if (key === "reasoning" && session_id) {
				rt.store.update(session_id, { effort: String(value) });
				broadcast(session_id, "session.info", { reasoning_effort: value });
			} else if (key === "approvals.mode") {
				writeAgent(profile || "default", { ...readAgent(profile || "default"), approvals: String(value) });
			}
			return { ok: true };
		},
	},
	routes: {
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
		// The config Bench reads, built from Bench's own settings (read-only).
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
			const k = findSkill(profileOf(req), req.query.get("name") ?? "");
			const f = k && path.join(k.dir, "SKILL.md");
			if (!f) throw new HttpError(404, "skill not found");
			return { content: fs.readFileSync(f, "utf8"), path: f };
		},
		"PUT /api/skills/content": (req) => {
			const k = findSkill(profileOf(req), req.body?.name ?? "");
			const f = k ? path.join(k.dir, "SKILL.md") : path.join(agentDir(profileOf(req)), "skills", req.body?.name, "SKILL.md");
			fs.mkdirSync(path.dirname(f), { recursive: true });
			fs.writeFileSync(f, String(req.body?.content ?? ""));
			return { ok: true, path: f };
		},
		"POST /api/providers/custom-endpoints": (req) => {
			const b = req.body ?? {};
			const id = String(b.id || b.name || "").trim();
			if (!id || !b.base_url) throw new HttpError(400, "id and base_url are required");
			const prev = providers().find((p) => p.id === id);
			const models = { ...(prev?.models ?? {}), ...(b.model ? { [b.model]: b.context_length ? { context_length: Number(b.context_length) } : {} } : {}) };
			saveProvider({ id, name: b.name ?? id, base_url: b.base_url, api_key: b.api_key || prev?.api_key, api_mode: prev?.api_mode ?? "chat_completions", models, context_length: b.context_length ? Number(b.context_length) : prev?.context_length, parallel: b.parallel === undefined ? prev?.parallel : Number(b.parallel) > 0 ? Math.floor(Number(b.parallel)) : undefined });
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
		"GET /api/providers/custom-endpoints": () => {
			const current = readAgent("default").defaultProvider;
			const endpoints = providers().map((p) => ({
				id: p.id,
				name: p.name,
				base_url: p.base_url,
				model: Object.keys(p.models)[0] ?? "",
				models: Object.keys(p.models),
				context_length: p.context_length ?? null,
				parallel: p.parallel ?? null,
				has_api_key: Boolean(p.api_key),
				// Shown whole: this is the person's own machine and keys (a masked one saved back would break it).
				api_key_preview: p.api_key ?? "",
				is_current: p.id === current,
			}));
			return { endpoints, current };
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
		"GET /api/env": (req) => {
			const env = readEnv(profileOf(req));
			return Object.fromEntries(Object.entries(env).map(([k, v]) => [k, { is_set: Boolean(v), redacted_value: v ?? "", description: "", category: "", channel_managed: false, custom: true }]));
		},
		"PUT /api/env": (req) => {
			setEnv(String(req.body.key), String(req.body.value ?? ""));
			return { ok: true };
		},
		"DELETE /api/env": (req) => {
			setEnv(String(req.body.key), undefined);
			return { ok: true };
		},
	},
	dynamic: [
		[
			"POST",
			/^\/api\/providers\/custom-endpoints\/([^/]+)\/activate$/,
			(_req, m) => {
				const id = decodeURIComponent(m[1]);
				const p = providers().find((x) => x.id === id);
				if (!p) throw new HttpError(404, `no provider ${id}`);
				const g = readAgent("default");
				writeAgent("default", { ...g, defaultProvider: id, defaultModel: Object.keys(p.models)[0] ?? g.defaultModel });
				return { ok: true };
			},
		],
		[
			"DELETE",
			/^\/api\/providers\/custom-endpoints\/([^/]+)$/,
			(_req, m) => {
				deleteProvider(decodeURIComponent(m[1]));
				return { ok: true };
			},
		],
		[
			"GET",
			/^\/api\/profiles\/([^/]+)\/soul$/,
			(_req, m) => {
				const f = path.join(agentDir(decodeURIComponent(m[1])), "SYSTEM.md");
				return { content: fs.existsSync(f) ? fs.readFileSync(f, "utf8") : "" };
			},
		],
		[
			"PUT",
			/^\/api\/profiles\/([^/]+)\/soul$/,
			(req, m) => {
				const f = path.join(agentDir(decodeURIComponent(m[1])), "SYSTEM.md");
				fs.mkdirSync(path.dirname(f), { recursive: true });
				fs.writeFileSync(f, String(req.body?.content ?? ""));
				return { ok: true };
			},
		],
		[
			"PATCH",
			/^\/api\/profiles\/([^/]+)$/,
			(req, m) => {
				const from = agentDir(decodeURIComponent(m[1]));
				const name = String(req.body?.new_name ?? "");
				if (!/^[a-z0-9][a-z0-9_-]*$/i.test(name) || m[1] === "default") throw new HttpError(400, "invalid rename");
				fs.renameSync(from, agentDir(name));
				return { name };
			},
		],
		[
			"DELETE",
			/^\/api\/profiles\/([^/]+)$/,
			(_req, m) => {
				if (m[1] === "default") throw new HttpError(400, "the default profile cannot be deleted");
				fs.rmSync(agentDir(decodeURIComponent(m[1])), { recursive: true, force: true });
				return { ok: true };
			},
		],
	],
});
