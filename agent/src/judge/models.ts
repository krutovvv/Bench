// The evaluator as Pi knows it: a classifier model. The gear lists System One servers (Jev's API:
// TypeSafe, OpenRouter, a local one), one of them the main, another its fallback; pi-ai's provider
// `bench` has one classifier model, `evaluator`, that answers with the main server or, when it
// fails or is quiet after a failure, with the fallback — as `runtime/fallback.ts` pairs the chat
// models. The points (`client.ts`) and code mode scripts (`models.classify()`) ask the same model.
//
// The setting `judge`: `{ servers: [{ id, name, base_url, model, api_key }], main, fallback }`.
// The earlier one-server shape (`{ enabled, base_url, model, api_key }`) reads as that one server.

import { createProvider, type ClassifierContext, type ClassifierModel, type ClassifierOptions, type ClassifierResult, type Provider } from "@earendil-works/pi-ai";
import { classify as systemOne } from "@earendil-works/pi-ai/api/typesafe-system-one";
import { getSetting, setSetting } from "../config.ts";

export interface EvaluatorServer {
	id: string;
	name?: string;
	base_url: string;
	model?: string;
	api_key?: string;
}

export interface EvaluatorSettings {
	servers: EvaluatorServer[];
	main?: string;
	fallback?: string;
}

/** The provider and the model scripts and points ask: `models.getModelOfType("classifier", "bench", "evaluator")`. */
export const PROVIDER = "bench";
export const MODEL = "evaluator";

/** A local server starts its container and loads its model on the first request (90 s measured
 *  behind a GPU balancer that first stops speech to text): that one may take this long. */
export const TIMEOUT_MS = 150_000;
/** After a failure a server is left alone this long. */
const QUIET_MS = 60_000;
const quietUntil = new Map<string, number>();

/** The gear's servers, the earlier one-server shape read as one. */
export function evaluatorSettings(): EvaluatorSettings {
	const s: any = getSetting("judge") ?? {};
	if (Array.isArray(s.servers)) return { servers: s.servers.filter((x: any) => x?.id && String(x.base_url ?? "").trim()), main: s.main, fallback: s.fallback };
	if (String(s.base_url ?? "").trim()) return { servers: [{ id: "evaluator", name: "Evaluator", base_url: s.base_url, model: s.model, api_key: s.api_key }], main: "evaluator" };
	return { servers: [] };
}

/** The main server and the fallback (another one), as set; the first server when none is main. */
export function pair(s = evaluatorSettings()): { main?: EvaluatorServer; fallback?: EvaluatorServer } {
	const main = s.servers.find((x) => x.id === s.main) ?? s.servers[0];
	const fallback = s.servers.find((x) => x.id === s.fallback && x.id !== main?.id);
	return { main, fallback };
}

/** The gear has a server: the evaluator can be asked at all. */
export function evaluatorReady(): boolean {
	return !!pair().main;
}

/** A server that failed less than a minute ago. */
export function quiet(server: EvaluatorServer): boolean {
	return Date.now() < (quietUntil.get(server.base_url) ?? 0);
}

/** Some server of the pair may be asked now. */
export function answering(): boolean {
	const { main, fallback } = pair();
	return [main, fallback].some((s) => s && !quiet(s));
}

/** An address given with or without `/v1`, as pi-ai's System One API wants it (it adds `systemone`). */
export function baseOf(base: string): string {
	return `${base.trim().replace(/\/+$/, "").replace(/\/v1$/, "")}/v1`;
}

/** The server as pi-ai's classifier model. */
export function modelOf(server: EvaluatorServer): ClassifierModel<"typesafe-system-one"> {
	return {
		type: "classifier",
		id: server.model?.trim() || "jev-latest",
		name: server.name || server.id,
		api: "typesafe-system-one",
		provider: PROVIDER,
		baseUrl: baseOf(server.base_url),
		input: ["text"],
		cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
		contextWindow: 16_384,
	};
}

/** One server asked. pi-ai sends a key always; a local server without one is sent a stand-in. */
export function classifyOn(server: EvaluatorServer, context: ClassifierContext, options: ClassifierOptions = {}): Promise<ClassifierResult> {
	return systemOne(modelOf(server), context, { timeoutMs: TIMEOUT_MS, ...options, apiKey: server.api_key || "none" });
}

/** The answer, with which server gave it: the main one, else the fallback; a failure keeps that server quiet a minute. */
export async function classifyPair(context: ClassifierContext, options: ClassifierOptions = {}): Promise<ClassifierResult & { server?: string }> {
	const { main, fallback } = pair();
	let last: ClassifierResult | undefined;
	for (const server of [main, fallback]) {
		if (!server || quiet(server)) continue;
		const r = await classifyOn(server, context, options);
		if (r.stopReason !== "error") return { ...r, provider: PROVIDER, model: MODEL, server: server.id };
		if (options.signal?.aborted) return { ...r, stopReason: "aborted" };
		quietUntil.set(server.base_url, Date.now() + QUIET_MS);
		console.error(`[judge] ${server.id}: ${r.errorMessage}`);
		last = r;
	}
	return last ?? { api: "typesafe-system-one", provider: PROVIDER, model: MODEL, answers: {}, stopReason: "error", errorMessage: main ? "the evaluator's servers are quiet after a failure" : "no evaluator server is set in the gear", timestamp: Date.now() };
}

/** pi-ai's provider `bench` with the model `evaluator`, for the Harness's models (code mode's `models.*`). */
export function evaluatorProvider(): Provider {
	// One model for the pair: which servers answer is read at each call, so the gear's changes
	// need no new provider. Its address and window stand for the pair, not used to send.
	const model: ClassifierModel<any> = { ...modelOf({ id: MODEL, base_url: "http://127.0.0.1" }), id: MODEL, name: "Evaluator", api: "bench-evaluator" };
	return createProvider({
		id: PROVIDER,
		name: "Evaluator",
		// Configured when the gear has a server: `getAvailableOfType("classifier")` lists it then.
		auth: { apiKey: { name: PROVIDER, resolve: async () => (evaluatorReady() ? { auth: {} } : undefined) } },
		models: [model],
		filterAllModels: (models: readonly any[]) => (evaluatorReady() ? models : []),
		classifiers: { "bench-evaluator": { classify: (_m: any, context: ClassifierContext, options?: ClassifierOptions) => classifyPair(context, options) } },
	} as any);
}

/** The gear's list as Bench shows it: the servers, which is main and which the fallback. */
export function evaluatorView(): { servers: EvaluatorServer[]; main: string | null; fallback: string | null } {
	const s = evaluatorSettings();
	const { main, fallback } = pair(s);
	return { servers: s.servers, main: main?.id ?? null, fallback: fallback?.id ?? null };
}

function store(s: EvaluatorSettings): void {
	setSetting("judge", { servers: s.servers, main: s.main, fallback: s.fallback });
}

/** A server added or changed; a new one gets an id from its name, the first one is main. An empty
 *  key keeps the saved one (Bench sends a key only when it was typed). */
export function saveServer(b: Partial<EvaluatorServer>): string {
	const s = evaluatorSettings();
	if (!String(b.base_url ?? "").trim()) throw new Error("the server's address is required");
	const prev = s.servers.find((x) => x.id === b.id);
	let id = prev?.id;
	if (!id) {
		const stem = String(b.name || b.model || "evaluator").trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "evaluator";
		id = stem;
		for (let n = 2; s.servers.some((x) => x.id === id); n++) id = `${stem}-${n}`;
	}
	const server: EvaluatorServer = { id, name: String(b.name ?? prev?.name ?? id).trim() || id, base_url: String(b.base_url).trim(), model: String(b.model ?? "").trim() || undefined, api_key: b.api_key || prev?.api_key || undefined };
	const servers = prev ? s.servers.map((x) => (x.id === id ? server : x)) : [...s.servers, server];
	store({ servers, main: s.main && servers.some((x) => x.id === s.main) ? s.main : servers[0].id, fallback: s.fallback });
	return id;
}

/** The main server: the fallback stops being it. */
export function makeMain(id: string): void {
	const s = evaluatorSettings();
	if (!s.servers.some((x) => x.id === id)) throw new Error(`no server ${id}`);
	store({ ...s, main: id, fallback: s.fallback === id ? undefined : s.fallback });
}

/** The fallback (another than the main one), or none. */
export function makeFallback(id: string | null): void {
	const s = evaluatorSettings();
	if (id && !s.servers.some((x) => x.id === id)) throw new Error(`no server ${id}`);
	if (id && id === pair(s).main?.id) throw new Error("the main server cannot be its own fallback");
	store({ ...s, main: pair(s).main?.id, fallback: id ?? undefined });
}

/** A server removed: the next one is main when it was; no fallback when it was. */
export function deleteServer(id: string): void {
	const s = evaluatorSettings();
	const servers = s.servers.filter((x) => x.id !== id);
	const main = pair(s).main?.id === id ? servers.find((x) => x.id !== s.fallback)?.id ?? servers[0]?.id : pair(s).main?.id;
	store({ servers, main, fallback: s.fallback === id || s.fallback === main ? undefined : s.fallback });
}
