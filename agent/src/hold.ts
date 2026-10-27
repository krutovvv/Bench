// The person's pause of a run (docs/thread-objects.md). Pi has no pause of a conversation, so
// the pause is ours, in Pi's terms: a conversation document `bench.hold` with the run's holds,
// changed in commits, and a gate in the `beforeTool` hook that reads it. Pi runs `beforeTool`
// before it records a call's intent, so a run that resumes after a restart meets the gate
// again: a held run stays held. The document is the truth; the waiters here only wake the gate.

import { defineDoc } from "@earendil-works/pi-durable";

/** A stretch the person held the run, ms; `to` absent while it still holds. */
export interface Hold {
	from: number;
	to?: number;
}

export const HoldDoc = defineDoc<{ holds: Hold[] }>({
	kind: "bench.hold",
	version: 1,
	scope: "conversation",
	history: "latest",
	// A forked thread starts free: a pause is the person's, of the run they paused.
	fork: "initial",
	initial: () => ({ holds: [] }),
	checkpointWhen: () => true,
} as any);

/** Whether the last hold is still open. */
export const held = (holds: readonly Hold[] | undefined) => {
	const last = holds?.at(-1);
	return Boolean(last && last.to === undefined);
};

/** Close the open hold at `at`; false when none was open. */
export function closeHold(holds: Hold[], at: number): boolean {
	const last = holds.at(-1);
	if (!last || last.to !== undefined) return false;
	last.to = at;
	return true;
}

/** Wake-ups of the gates waiting on a conversation's holds: `true` go on, `false` stopped. */
export class Gates {
	private waiting = new Map<string, Set<(go: boolean) => void>>();

	/** Resolves at the next change of the conversation's holds; rejects when `signal` aborts
	 * (Pi ending the invocation: the Harness closing), so the wait never outlives its call. */
	next(conversation: string, signal?: AbortSignal): Promise<boolean> {
		return new Promise((resolve, reject) => {
			if (signal?.aborted) return reject(signal.reason ?? new Error("aborted"));
			const set = this.waiting.get(conversation) ?? new Set();
			const done = (go: boolean) => {
				signal?.removeEventListener("abort", abort);
				resolve(go);
			};
			const abort = () => {
				set.delete(done);
				reject(signal?.reason ?? new Error("aborted"));
			};
			signal?.addEventListener("abort", abort, { once: true });
			set.add(done);
			this.waiting.set(conversation, set);
		});
	}

	wake(conversation: string, go: boolean): void {
		const set = this.waiting.get(conversation);
		this.waiting.delete(conversation);
		for (const r of set ?? []) r(go);
	}
}
