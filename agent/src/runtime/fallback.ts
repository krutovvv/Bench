// The fallback model: when the main model fails, the same request goes to a second one.
//
// Pi retries a failed request (settings.retry) but always with the same model, and neither Pi
// Durable nor pi-ai has a fallback. So the fallback is a pi-ai provider of our own: its one model
// stands for the pair, and its stream asks the main model first and, when that ends in an error
// (a refused connection, a 5xx, a broken stream) or goes silent, asks the fallback with the same
// context and options. Pi sees one response and keeps its own retries, history and usage as they
// are. A false switch costs one answer from the cheap fallback, so the watch before the first
// event is short. Once the answer flows, a pause is long: some servers (TabbyAPI with qwen3_coder)
// send a tool call only whole, and a file written in one call keeps them silent for minutes.

import { createAssistantMessageEventStream, createProvider, type AssistantMessageEvent, type Model, type Models, type Provider } from "@earendil-works/pi-ai";
import { isContextOverflow } from "@earendil-works/pi-ai/utils/overflow";

/** How long the main model may stay silent: before its first event, then between two events;
 *  before the first, also as long as reading the prompt takes (`prefillMs`). */
export const WATCH: { firstMs: number; idleMs: number; prefill?: boolean } = { firstMs: 120_000, idleMs: 300_000, prefill: true };

/** A local server reads a prompt it has no cache for at ~1 100 tokens a second (a token ~3
 *  characters): a long one, after a compaction, takes 100 s before its first event. Silence that
 *  short is reading, not failure — falling over then costs the fallback's answer and, back on the
 *  main model, the whole prompt read again. */
const PREFILL_CHARS_PER_MS = 2;
export function prefillMs(context: any): number {
	const chars = JSON.stringify(context?.messages ?? []).length + String(context?.systemPrompt ?? "").length + JSON.stringify(context?.tools ?? []).length;
	return Math.ceil(chars / PREFILL_CHARS_PER_MS) + 30_000;
}

/** A provider whose model `main.id` answers with `main`, or with `fallback` when `main` fails. */
export function fallbackProvider(id: string, models: Models, main: Model<any>, fallback: Model<any>, watch = WATCH, onFallback: (why: string) => void = () => {}): Provider {
	const model: Model<any> = { ...main, provider: id };
	const both = (_m: Model<any>, context: any, options: any = {}) => {
		const out = createAssistantMessageEventStream();
		// Models put this provider's (empty) auth into the options; each model gets its own again.
		const { apiKey: _k, headers: _h, ...rest } = options;
		void (async () => {
			const first = await attempt(models, main, context, rest, { ...watch, firstMs: Math.max(watch.firstMs, watch.prefill ? prefillMs(context) : 0) }, out);
			if (first === undefined) return;
			console.error(`[model] ${main.provider}/${main.id} failed (${first}); answering with ${fallback.provider}/${fallback.id}`);
			onFallback(first);
			const second = models.streamSimple(fallback, context, rest);
			for await (const e of second) out.push(e);
			out.end(await second.result());
		})().catch((e) => {
			out.push({ type: "error", reason: "error", error: failed(main, String((e as Error)?.message ?? e)) } as any);
			out.end();
		});
		return out;
	};
	return createProvider({
		id,
		name: `${main.provider} → ${fallback.provider}`,
		baseUrl: main.baseUrl,
		auth: { apiKey: { name: id, resolve: async () => ({ auth: {} }) } },
		models: [model],
		api: { stream: both, streamSimple: both } as any,
	} as any);
}

/**
 * Streams `main` into `out`. Returns undefined when it answered (`out` is ended), or why it failed:
 * then nothing final went to `out`, and the fallback's events replace its partial answer.
 */
async function attempt(models: Models, main: Model<any>, context: any, options: any, watch: typeof WATCH, out: ReturnType<typeof createAssistantMessageEventStream>): Promise<string | undefined> {
	const outer: AbortSignal | undefined = options.signal;
	const own = new AbortController();
	const stop = () => own.abort(outer?.reason);
	outer?.addEventListener("abort", stop, { once: true });
	let silent = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const arm = (ms: number) => {
		clearTimeout(timer);
		timer = setTimeout(() => {
			silent = true;
			own.abort(new Error("the model went silent"));
		}, ms);
	};
	try {
		arm(watch.firstMs);
		const stream = models.streamSimple(main, context, { ...options, signal: own.signal });
		let last: AssistantMessageEvent | undefined;
		for await (const e of stream) {
			arm(watch.idleMs);
			if (e.type === "done" || e.type === "error") last = e;
			else out.push(e);
		}
		const message = await stream.result();
		if (silent) return `no answer in ${Math.round((message.content.length ? watch.idleMs : watch.firstMs) / 1000)} s`;
		// The person's Stop is theirs, not a failure; a context overflow is Pi's to compact.
		if (message.stopReason !== "error" || outer?.aborted || isContextOverflow(message, main.contextWindow)) {
			if (last) out.push(last);
			out.end(message);
			return undefined;
		}
		return message.errorMessage || "error";
	} finally {
		clearTimeout(timer);
		outer?.removeEventListener("abort", stop);
	}
}

function failed(model: Model<any>, text: string): any {
	return {
		role: "assistant",
		content: [],
		api: model.api,
		provider: model.provider,
		model: model.id,
		usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
		stopReason: "error",
		errorMessage: text,
		timestamp: Date.now(),
	};
}
