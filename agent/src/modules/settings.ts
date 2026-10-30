// The gear and the agent's own settings: providers and models, the config, keys in
// the environment, the general skills and MCP servers, the agent's status.

import YAML from "yaml";
import pkg from "../../package.json" with { type: "json" };
import { deepMerge, deleteProvider, getSetting, providers, readAgent, readConfig, readEnv, saveProvider, setEnv as storeEnv, setSetting, writeAgent } from "../config.ts";
import { listSkills as skillsOf } from "../builtin/skills.ts";
import { HttpError, agentOf, type Ctx, type Module } from "../http.ts";
import { mcpStatus } from "../mcp/mcp.ts";
import { checkJudge } from "../judge/client.ts";
import { deleteServer, evaluatorView, makeFallback, makeMain, saveServer } from "../judge/models.ts";
import { BUILD } from "../instance.ts";

/**
 * `PUT /api/config`: each top-level key merges into its setting. The reasoning level and the
 * approvals are the agent's own (a project's thread reads `project:<id>`); the model is one for
 * every thread and goes to the general agent; the fallback is a setting like the rest.
 */
function patchConfig(agent: string, patch: any): void {
	for (const [k, v] of Object.entries(patch ?? {})) {
		const m = v as any;
		if (k === "model" && m && typeof m === "object") {
			const p = readAgent("default");
			writeAgent("default", { ...p, defaultProvider: m.provider ?? p.defaultProvider, defaultModel: m.default ?? m.name ?? p.defaultModel });
		} else if (k === "agent" && m && typeof m === "object" && m.reasoning_effort) {
			writeAgent(agent, { ...readAgent(agent), defaultThinkingLevel: String(m.reasoning_effort) });
		} else if (k === "compaction" && m && typeof m === "object") {
			// A share of the window (0.9), or null: Pi's own policy again.
			const at = m.at === null || m.at === undefined ? undefined : Number(m.at);
			writeAgent(agent, { ...readAgent(agent), compaction: at });
		} else if (k === "evaluator" && m && typeof m === "object" && m.enabled !== undefined) {
			// The project's use of the evaluator (`judge/`); the gear's connection is the other half.
			writeAgent(agent, { ...readAgent(agent), evaluator: m.enabled ? { enabled: true } : undefined });
		} else if (k === "step_limit" && m && typeof m === "object") {
			// The most steps in a turn, or null: no limit (`turn/steps.ts`).
			const n = Math.floor(Number(m.steps));
			writeAgent(agent, { ...readAgent(agent), stepLimit: n > 0 ? n : undefined });
		} else if (k === "approvals" && m && typeof m === "object" && m.mode !== undefined) {
			writeAgent(agent, { ...readAgent(agent), approvals: String(m.mode) });
		} else setSetting(k, deepMerge(getSetting(k), v));
	}
}

/** A change to the evaluator's servers, answered with the list as it is now; a refusal as a 400. */
function judgeChange(change: () => unknown) {
	try {
		change();
	} catch (e) {
		throw new HttpError(400, (e as Error).message);
	}
	return evaluatorView();
}

function listSkills(agent: string) {
	const disabled = new Set<string>(readConfig(agent)?.skills?.disabled ?? []);
	return skillsOf(agent).map((k) => ({ name: k.name, category: k.category, description: k.description, enabled: !disabled.has(k.name) }));
}

function setEnv(key: string, value: string | undefined): void {
	if (!/^[A-Z_][A-Z0-9_]*$/i.test(key)) throw new HttpError(400, "invalid key");
	storeEnv(key, value);
}

export default ({ rt }: Ctx): Module => ({
	routes: {
		"GET /api/status": () => ({
			version: `bench-agent ${pkg.version}`,
			// The sources it runs (Bench installs its own on a server when they differ).
			build: BUILD,
			// Threads with a run going: a restart waits for none of them, but Bench says so.
			active: [...rt.live.values()].filter((l) => l.busy).length,
			mcp: mcpStatus(),
		}),
		// The config Bench reads, built from Bench's own settings (read-only).
		"GET /api/config/raw": (req) => ({ yaml: YAML.stringify(readConfig(agentOf(req))) }),
		"PUT /api/config": (req) => {
			patchConfig(agentOf(req), req.body?.config ?? {});
			return { ok: true };
		},
		"GET /api/skills": (req) => listSkills(agentOf(req)),
		"POST /api/providers/custom-endpoints": (req) => {
			const b = req.body ?? {};
			const id = String(b.id || b.name || "").trim();
			if (!id || !b.base_url) throw new HttpError(400, "id and base_url are required");
			const prev = providers().find((p) => p.id === id);
			const models = { ...(prev?.models ?? {}), ...(b.model ? { [b.model]: b.context_length ? { context_length: Number(b.context_length) } : {} } : {}) };
			saveProvider({ id, name: b.name ?? id, base_url: b.base_url, api_key: b.api_key || prev?.api_key, api_mode: prev?.api_mode ?? "chat_completions", models, context_length: b.context_length ? Number(b.context_length) : prev?.context_length, parallel: b.parallel === undefined ? prev?.parallel : Number(b.parallel) > 0 ? Math.floor(Number(b.parallel)) : undefined });
			rt.slots.changed();
			return { ok: true, id };
		},
		"POST /api/providers/custom-endpoints/validate": async (req) => {
			const b = req.body ?? {};
			// Bench sends the key only when it was changed: an unchanged one is the saved one.
			const key = b.api_key || providers().find((p) => p.id === b.id)?.api_key;
			try {
				const r = await fetch(`${String(b.base_url).replace(/\/$/, "")}/models`, { headers: key ? { Authorization: `Bearer ${key}` } : {}, signal: AbortSignal.timeout(10000) });
				if (!r.ok) return { ok: false, message: `HTTP ${r.status}`, models: [] };
				const j: any = await r.json();
				const models = (j.data ?? j.models ?? []).map((m: any) => m.id ?? m.name).filter(Boolean);
				return { ok: true, message: `${models.length} models`, models };
			} catch (e) {
				return { ok: false, message: String((e as Error).message), models: [] };
			}
		},
		// The evaluator's Check in the gear, with the settings as typed (`judge/client.ts`).
		"POST /api/judge/check": (req) => checkJudge(req.body ?? {}),
		// The evaluator's servers in the gear (`judge/models.ts`): a list, one main, one fallback.
		"GET /api/judge": () => evaluatorView(),
		"PUT /api/judge/server": (req) => judgeChange(() => saveServer(req.body ?? {})),
		"DELETE /api/judge/server": (req) => judgeChange(() => deleteServer(String(req.body?.id ?? ""))),
		"POST /api/judge/main": (req) => judgeChange(() => makeMain(String(req.body?.id ?? ""))),
		"POST /api/judge/fallback": (req) => judgeChange(() => makeFallback(req.body?.id ? String(req.body.id) : null)),
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
		"GET /api/env": (req) => {
			const env = readEnv(agentOf(req));
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
				rt.slots.changed();
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
	],
});
