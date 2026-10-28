// The model every thread runs with, as pi-ai knows it: one provider per agent settings and model
// (`<settings>.<provider>`), the fallback paired with it as a provider of ours (`fallback.ts`),
// each project's compaction under its own reference, and the thread's settings as Bench shows
// them (its effort; the fallback answering, and why).

import { createModels, createProvider, type Model } from "@earendil-works/pi-ai";
import { anthropicMessagesApi } from "@earendil-works/pi-ai/api/anthropic-messages.lazy";
import { openAICompletionsApi } from "@earendil-works/pi-ai/api/openai-completions.lazy";
import { projects } from "./builtin/projects.ts";
import { agents, compactionAt, compactionPolicy, resolveFallback, resolveModel, type ModelChoice } from "./config.ts";
import { fallbackProvider } from "./fallback.ts";
import type { ThreadInfo } from "./protocol.gen.ts";
import type { ThreadRow } from "./store.ts";

/** A model as `configure()` names it. */
export type ModelRef = { provider: string; modelId: string };

export class Models {
	/** pi-ai's registry, the Harness's models. */
	readonly pi = createModels();
	private registered = new Set<string>();
	/** Why the main model last failed over to the fallback (the status bar says it). */
	private fallbackWhy = "";

	/**
	 * Registers the model every thread runs with (and its fallback, when one is set) and returns
	 * the reference for `configure()`: the pair's own provider, or the model alone.
	 */
	ref(settings: string, choice = resolveModel(), fallback = resolveFallback()): ModelRef {
		const main = this.register(settings, choice);
		if (!fallback || (fallback.provider === choice.provider && fallback.model === choice.model)) return main;
		const spare = this.register(settings, fallback);
		const id = `${main.provider}~${spare.provider}.${spare.modelId}`.replace(/[^a-zA-Z0-9._~-]/g, "_");
		const key = `${id}|${main.modelId}|${choice.baseUrl}|${choice.contextLength}|${fallback.baseUrl}`;
		if (!this.registered.has(key)) {
			this.registered.add(key);
			const get = (r: ModelRef) => this.pi.getModel(r.provider, r.modelId)!;
			this.pi.setProvider(fallbackProvider(id, this.pi, get(main), get(spare), undefined, (why) => (this.fallbackWhy = why)));
		}
		return { provider: id, modelId: main.modelId };
	}

	/** The model a reference names, to call it directly (a title). */
	model(ref: ModelRef): Model<any> {
		return this.pi.getModel(ref.provider, ref.modelId)!;
	}

	/**
	 * Pi's compaction per model reference: each project that set a share gets its own policy under
	 * its own provider (`project_<id>.<provider>`), so projects differ on one Harness.
	 */
	compactionOverrides(): Record<string, ReturnType<typeof compactionPolicy>> {
		const out: Record<string, ReturnType<typeof compactionPolicy>> = {};
		const choice = resolveModel();
		for (const settings of [...agents(), ...projects.list().map((p) => `project:${p.id}`)]) {
			const at = compactionAt(settings);
			if (at === undefined) continue;
			try {
				const ref = this.ref(settings, choice);
				out[`${ref.provider}/${ref.modelId}`] = compactionPolicy(at, choice.contextLength);
			} catch {}
		}
		return out;
	}

	/** The thread's settings as Bench shows them; with the message just answered, whether the
	 * fallback model wrote it (and why the main one did not). */
	info(row: ThreadRow, msg?: any): ThreadInfo {
		const spare = resolveFallback();
		const fell = msg && spare && msg.provider === this.providerId(row.agent, spare) && msg.model === spare.model;
		return { effort: row.effort ?? "", ...(fell ? { fallback: { model: spare.model, reason: this.fallbackWhy } } : {}) };
	}

	private providerId(settings: string, choice: ModelChoice): string {
		return `${settings}.${choice.provider}`.replace(/[^a-zA-Z0-9._-]/g, "_");
	}

	/** Registers one model's provider with pi-ai. */
	private register(settings: string, choice: ModelChoice): ModelRef {
		const id = this.providerId(settings, choice);
		const key = `${id}|${choice.model}|${choice.baseUrl}|${choice.contextLength}|${choice.apiKey ? "k" : ""}`;
		if (!this.registered.has(key)) {
			this.registered.add(key);
			const anthropic = choice.apiMode === "anthropic_messages";
			const model: Model<any> = {
				id: choice.model,
				name: choice.model,
				api: anthropic ? "anthropic-messages" : "openai-completions",
				provider: id,
				baseUrl: choice.baseUrl,
				reasoning: true,
				input: ["text", "image"],
				cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
				contextWindow: choice.contextLength,
				maxTokens: Math.min(32000, Math.floor(choice.contextLength / 4)),
				compat: anthropic ? undefined : { supportsDeveloperRole: false },
			} as any;
			const apiKey = choice.apiKey;
			this.pi.setProvider(
				createProvider({
					id,
					name: choice.provider,
					baseUrl: choice.baseUrl,
					auth: { apiKey: { name: choice.provider, resolve: async () => ({ auth: apiKey ? { apiKey } : {} }) } },
					models: [model],
					api: anthropic ? anthropicMessagesApi() : openAICompletionsApi(),
				} as any),
			);
		}
		return { provider: id, modelId: choice.model };
	}
}

/** Pi's thinking level for a reasoning effort ("" and unknown ones: medium). */
export function thinking(effort: string): any {
	const e = (effort || "medium").toLowerCase();
	if (e === "none" || e === "off") return "off";
	return ["minimal", "low", "medium", "high", "xhigh"].includes(e) ? e : "medium";
}
