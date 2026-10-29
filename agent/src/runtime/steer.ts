// A request's own settings, decided just before it (`beforeRequest`), that Pi's hook cannot set: it
// returns only the messages. The model's API is wrapped where the model is registered
// (`models.ts`): a request whose provider session (Pi's `ProviderDoc`, one per conversation) has a
// word waiting takes it, once. Today the one word is "no reasoning for this step"
// (`judge/step.ts`): a step that reads, lists or runs needs none, and a local model spends its
// slowest tokens there. It is held (`judge/step.ts` `PLAIN_STEPS`) while the model server's
// template puts its reasoning line at the prompt's start: a step without reasoning then rewrites
// the head, and the next step with it rewrites it back.

import { noThinking } from "./thinking.ts";

export interface Word {
	/** No reasoning for this request. */
	plain?: boolean;
}

const waiting = new Map<string, Word>();

/** The next request of this provider session takes `word`. */
export function steer(session: string, word: Word): void {
	waiting.set(session, word);
}

/** The word for this request, once. */
function take(session: unknown): Word | undefined {
	if (typeof session !== "string") return undefined;
	const w = waiting.get(session);
	waiting.delete(session);
	return w;
}

/**
 * A model's API whose requests take their session's word (`steer`), and whose requests without
 * reasoning say so. Pi sends nothing for a thread whose level is "off", and a server that thinks by
 * default thinks anyway; so such a request carries the template's switch (`noThinking`). The
 * same for every request of the thread keeps the prompt's head the same: on the dev server's
 * template the switch also takes the reasoning line out of the system prompt's start, so a thread
 * that went from reasoning to none and back read its whole prompt again each time (measured: 0
 * of 7 303 tokens from the cache), while one that stays either way keeps it all.
 */
export function steered<A extends { stream?: any; streamSimple?: any }>(api: A): A {
	const wrap = (fn: any) =>
		fn &&
		((model: any, context: any, options: any = {}) => {
			const w = take(options?.sessionId);
			const plain = !!w?.plain || options.reasoning === undefined;
			if (!plain) return fn(model, context, options);
			const { reasoning: _r, onPayload, ...rest } = options;
			return fn(model, context, {
				...rest,
				onPayload: async (payload: unknown, m: any) => {
					const p = (await onPayload?.(payload, m)) ?? payload;
					return noThinking(p, m) ?? p;
				},
			});
		});
	return { ...api, stream: wrap(api.stream), streamSimple: wrap(api.streamSimple) };
}
