// What the agent keeps in memory about a conversation between its hooks: the turn (`turn.ts`), the
// window's count (`window.ts`), the sections held (`frozen.ts`), the specs told (`extensions/assistant.ts`),
// when the evaluator last weighed a compaction (`judge/boundary.ts`). One map, by conversation: each
// part is a slot of it, and `forget` drops them all when the Runtime closes or deletes the thread.
// Nothing here is the thread's record (that is Pi's journal): after a restart each starts again.

const kept = new Map<string, Map<symbol, unknown>>();

/** One part of what is kept per conversation. */
export interface Slot<T> {
	/** The conversation's value, made with the slot's `make` when it has none. */
	get(conversation: string): T;
	set(conversation: string, value: T): void;
	/** The conversation's value dropped: the next `get` makes it anew. */
	drop(conversation: string): void;
}

/** A part kept per conversation; `make` gives a conversation's first value. */
export function slot<T>(make: () => T): Slot<T> {
	const key = Symbol();
	const own = (conversation: string) => {
		let parts = kept.get(conversation);
		if (!parts) kept.set(conversation, (parts = new Map()));
		return parts;
	};
	return {
		get(conversation) {
			const parts = own(conversation);
			if (!parts.has(key)) parts.set(key, make());
			return parts.get(key) as T;
		},
		set(conversation, value) {
			own(conversation).set(key, value);
		},
		drop(conversation) {
			kept.get(conversation)?.delete(key);
		},
	};
}

/** Everything kept about a conversation, dropped (its thread closed or deleted). */
export function forget(conversation: string): void {
	kept.delete(conversation);
}

/** How many conversations have something kept (tests). */
export function keptCount(): number {
	return kept.size;
}
