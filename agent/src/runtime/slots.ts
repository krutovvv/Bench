// How many threads work at once: the main provider's `parallel` (the gear's "Threads at once"; none:
// all of them). A thread takes a slot at its first request to the model and keeps it to the end of
// its run, so ten threads started together run through one or two at a time, in the order they
// asked, each to its answer. Pi has no such limit between conversations, so it is ours, in Pi's
// terms: a gate in the `beforeRequest` hook, which Pi runs again when it resumes a run, so a restart
// finds the line anew. A thread that waits for the person (a question, an approval, a pause) gives
// its slot up and asks again at its next request: nobody's absence stops the rest.

import { providers, readAgent } from "../config.ts";
import type { Runtime } from "./runtime.ts";

/** The line itself, with no Pi in it: who holds a slot, who waits, in order. */
export class Line {
	private holders = new Set<string>();
	private waiting: { id: string; go: () => void }[] = [];
	/** The place each waiter was last told it has. */
	private told = new Map<string, number>();
	private limit: () => number;
	private passes: (id: string) => boolean;
	private tell: (id: string, place: number | null) => void;

	/** `limit`: slots, 0 for no limit. `passes`: a thread that, under a limit, takes no slot for now
	 * and lets others go before it (held by the person). `tell`: a thread's place in the line changed (null: it is out of it). */
	constructor(limit: () => number, passes: (id: string) => boolean, tell: (id: string, place: number | null) => void) {
		this.limit = limit;
		this.passes = passes;
		this.tell = tell;
	}

	/** Resolves when `id` holds a slot; rejects when `signal` aborts while it waits. */
	enter(id: string, signal?: AbortSignal): Promise<void> {
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
				go: () => {
					signal?.removeEventListener("abort", abort);
					resolve();
				},
			};
			signal?.addEventListener("abort", abort, { once: true });
			this.waiting.push(waiter);
			this.pump();
		});
	}

	/** `id` holds no slot any more: its run ended, or it waits for the person. */
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
		const places = new Map(this.waiting.map((w, k) => [w.id, k + 1]));
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

/** The runtime's line of threads (`rt.slots`). */
export class Slots {
	readonly line: Line;

	constructor(rt: Runtime) {
		this.line = new Line(
			limit,
			(id) => rt.holds.isPaused(id),
			(id, place) => rt.tell(id, { kind: "line", value: place }),
		);
	}

	enter(id: string, signal?: AbortSignal): Promise<void> {
		return this.line.enter(id, signal);
	}

	leave(id: string): void {
		this.line.leave(id);
	}

	/** The limit or a hold changed. */
	changed(): void {
		this.line.pump();
	}

	placeOf(id: string): number | undefined {
		return this.line.placeOf(id);
	}
}
