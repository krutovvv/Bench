// How many requests the model's server takes at once: its streams, the main provider's `parallel`
// (the gear's "Streams", as the server's own config says it; none: no limit). Whatever asks the model
// takes one. A conversation takes one at its first request and keeps it to the end of its run, so ten
// threads started together run one or two at a time, in the order they asked, each to its answer, and
// a local model keeps its prompt cache; a sub-agent, being a conversation, would take one of its own.
// A title, a review or a compaction outside a run takes one for its one request.
//
// Pi has no such limit between conversations, so it is ours, in Pi's terms: a gate in the
// `beforeRequest` hook, which Pi runs again when it resumes a run, so a restart finds the line anew.
// A conversation that only waits for something else gives its stream up and asks again at its next
// request: the person (a question, an approval, a pause), and, when there are sub-agents, its own
// sub-agent, or a parent would hold the stream its child waits for.

import { providers, readAgent } from "../config.ts";
import type { Runtime } from "./runtime.ts";

/** The line itself, with no Pi in it: who holds a stream, who waits, in order. */
export class Line {
	private holders = new Set<string>();
	private waiting: { id: string; side: boolean; go: () => void }[] = [];
	/** The place each waiter was last told it has. */
	private told = new Map<string, number>();
	private limit: () => number;
	private passes: (id: string) => boolean;
	private tell: (id: string, place: number | null) => void;

	/** `limit`: streams, 0 for no limit. `passes`: one that, under a limit, takes no stream for now
	 * and lets others go before it (held by the person). `tell`: a place in the line changed (null:
	 * out of it). */
	constructor(limit: () => number, passes: (id: string) => boolean, tell: (id: string, place: number | null) => void) {
		this.limit = limit;
		this.passes = passes;
		this.tell = tell;
	}

	/** Whether `id` holds a stream. */
	holds(id: string): boolean {
		return this.holders.has(id);
	}

	/** Resolves when `id` holds a stream; rejects when `signal` aborts while it waits. `first`: it
	 * goes before those in the line (one short request, not a run). */
	enter(id: string, signal?: AbortSignal, first = false): Promise<void> {
		if (this.holders.has(id)) return Promise.resolve();
		return new Promise((resolve, reject) => {
			if (signal?.aborted) return reject(signal.reason ?? new Error("aborted"));
			const abort = () => {
				this.waiting = this.waiting.filter((w) => w !== waiter);
				reject(signal?.reason ?? new Error("aborted"));
				this.pump();
			};
			const waiter = {
				id,
				side: first,
				go: () => {
					signal?.removeEventListener("abort", abort);
					resolve();
				},
			};
			signal?.addEventListener("abort", abort, { once: true });
			if (first) this.waiting.unshift(waiter);
			else this.waiting.push(waiter);
			this.pump();
		});
	}

	/** `id` holds no stream any more: its run ended, or it waits for something else. */
	leave(id: string): void {
		if (this.holders.delete(id)) this.pump();
	}

	/** Who can go, goes; the others hear their place. After any change of who holds or waits, of the
	 * limit, of a hold. */
	pump(): void {
		const limit = this.limit();
		for (const w of [...this.waiting]) {
			if (limit > 0 && this.holders.size >= limit) break;
			if (limit > 0 && this.passes(w.id)) continue;
			this.waiting = this.waiting.filter((x) => x !== w);
			this.holders.add(w.id);
			w.go();
		}
		// Only those that will run have a place to be told: a request on the side is over in a moment.
		const places = new Map(this.waiting.filter((w) => !w.side).map((w, k) => [w.id, k + 1]));
		for (const [id, place] of places) {
			if (this.told.get(id) !== place) {
				this.told.set(id, place);
				this.tell(id, place);
			}
		}
		for (const id of [...this.told.keys()]) {
			if (!places.has(id)) {
				this.told.delete(id);
				this.tell(id, null);
			}
		}
	}

	/** A thread's place in the line, from 1; undefined when it is not in it. */
	placeOf(id: string): number | undefined {
		return this.told.get(id);
	}
}

/** The agent's main provider's limit. */
function limit(): number {
	const main = readAgent("default").defaultProvider;
	return providers().find((p) => p.id === main)?.parallel ?? 0;
}

/** The runtime's streams (`rt.streams`). Who holds one is a conversation, by its id, or a request
 * on the side by a name of its own. */
export class Streams {
	readonly line: Line;
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
		const thread = (holder: string) => rt.byConversation.get(holder)?.row.id;
		this.line = new Line(
			limit,
			(holder) => {
				const id = thread(holder);
				return !!id && rt.holds.isPaused(id);
			},
			(holder, place) => {
				const id = thread(holder);
				if (id) rt.tell(id, { kind: "line", value: place });
			},
		);
	}

	enter(conversation: string, signal?: AbortSignal): Promise<void> {
		return this.line.enter(conversation, signal);
	}

	/** A conversation gives its stream up: its run ended, or it waits for something else. */
	leave(conversation: string | number): void {
		this.line.leave(String(conversation));
	}

	/** One request outside a run: before those in the line, its stream back after it. `holder`
	 * already holding one (a compaction in its own run) just goes on with it. */
	async during<T>(holder: string, ask: () => Promise<T>, signal?: AbortSignal): Promise<T> {
		const had = this.line.holds(holder);
		await this.line.enter(holder, signal, true);
		try {
			return await ask();
		} finally {
			if (!had) this.line.leave(holder);
		}
	}

	/** The limit or a hold changed. */
	changed(): void {
		this.line.pump();
	}

	/** A thread's place in the line, from 1. */
	placeOf(threadId: string): number | undefined {
		const live = this.rt.live.get(threadId);
		return live && this.line.placeOf(String(live.conversation.id));
	}
}
