// A request's own settings, decided just before it (`beforeRequest`), that Pi's hook cannot set: it
// returns only the messages. The model's API is wrapped where the model is registered
// (`models.ts`): a request whose provider session (Pi's `ProviderDoc`, one per conversation) has a
// word waiting takes it, once. Today the one word is "no reasoning for this step"
// (`judge/step.ts`): a step that reads, lists or runs needs none, and a local model spends its
// slowest tokens there. Reasoning off changes only the end of the prompt (the template's
// generation switch), never its head; and every request renders the history's answers without
// reasoning as they were generated (`thinking.ts` `emptyThinking`), so the cache stays whole.

import { emptyThinking, noThinking } from "./thinking.ts";

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

/** A model's API whose requests take their session's word (`steer`). */
export function steered<A extends { stream?: any; streamSimple?: any }>(api: A): A {
	const wrap = (fn: any) =>
		fn &&
		((model: any, context: any, options: any = {}) => {
			const w = take(options?.sessionId);
			const plain = !!w?.plain && !!options.reasoning;
			const { reasoning, onPayload, ...rest } = options;
			return fn(model, context, {
				...rest,
				...(plain || reasoning === undefined ? {} : { reasoning }),
				// Every request: answers without reasoning rendered as generated (`emptyThinking`), so
				// the history the cache holds is the history sent.
				onPayload: async (payload: unknown, m: any) => {
					let p = (await onPayload?.(payload, m)) ?? payload;
					p = emptyThinking(p, m) ?? p;
					return plain ? (noThinking(p, m) ?? p) : p;
				},
			});
		});
	return { ...api, stream: wrap(api.stream), streamSimple: wrap(api.streamSimple) };
}
