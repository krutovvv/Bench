// The person's pause of a run (src/thread/SPEC.md). Pi has no pause of a conversation, so
// the pause is ours, in Pi's terms: a conversation document `bench.hold` with the run's holds,
// changed in commits, and a gate in the `beforeTool` hook that reads it. Pi runs `beforeTool`
// before it records a call's intent, so a run that resumes after a restart meets the gate
// again: a held run stays held. The document is the truth; the waiters here only wake the gate.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { defineDoc, type ConversationId } from "@earendil-works/pi-durable";
// A stretch the person held the run, ms; `to` absent while it still holds (crates/protocol).
import type { Hold } from "../protocol.gen.ts";
import type { Live, Runtime } from "./runtime.ts";

const ctx = BACKGROUND_CONTEXT;

export type { Hold };

export const HoldDoc = defineDoc<{ holds: Hold[] }>({
	kind: "bench.hold",
	version: 1,
	scope: "conversation",
	history: "latest",
	// A forked thread starts free: a pause is the person's, of the run they paused.
	fork: "initial",
	initial: () => ({ holds: [] }),
	checkpointWhen: () => true,
});

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

/** The holds of the runtime's threads: the person's pause and resume, the gate before each call. */
export class Holds {
	/** Wake-ups of the calls waiting at a held run's gate. */
	private gates = new Gates();

	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** Stand the run at its next step boundary: the answer being written is let through —
	 * stopping mid-sentence cuts Pi's turn — but no new call begins until the person lets it
	 * go on. The actions phase is where the minutes go; that is what the hold answers to. */
	async pause(id: string): Promise<void> {
		const live = await this.rt.thread(id);
		if (!live?.busy) return;
		await this.change(live, (h) => !held(h) && h.push({ from: Date.now() }) > 0);
		// A held run gives its stream up, and lets those behind it in the line go first.
		this.rt.streams.leave(live.conversation.id);
		this.rt.streams.changed();
	}

	/** Let the held run take its next step. */
	async resume(id: string): Promise<void> {
		const live = await this.rt.thread(id);
		if (!live) return;
		await this.change(live, (h) => closeHold(h, Date.now()));
		this.gates.wake(String(live.conversation.id), true);
		this.rt.streams.changed();
	}

	/** A stop outranks a hold: the call waiting at the gate is told to stop, not to go on. */
	async stop(live: Live): Promise<void> {
		await this.change(live, (h) => closeHold(h, Date.now()));
		this.gates.wake(String(live.conversation.id), false);
	}

	/** The run ended: whatever the person held lets go, nobody waits on it. */
	end(live: Live): void {
		if (held(live.holds)) void this.change(live, (h) => closeHold(h, Date.now())).catch(() => {});
		this.gates.wake(String(live.conversation.id), false);
	}

	/** A fresh turn starts free: the holds belong to the turn they were made in. */
	async clear(live: Live): Promise<void> {
		await this.change(live, (h) => h.splice(0).length > 0);
	}

	/** Called before every tool call: waits while the
	 * run's document says it is held; false when the person stopped it meanwhile. */
	async wait(threadId: string, signal?: AbortSignal): Promise<boolean> {
		const row = this.rt.store.get(threadId);
		if (!row) return true;
		const conversation = Number(row.conversation);
		for (;;) {
			const doc = await this.rt.harness.snapshot(HoldDoc, conversation as ConversationId, ctx);
			if (!held(doc?.holds)) return true;
			if (!(await this.gates.next(String(conversation), signal))) return false;
		}
	}

	isPaused(id: string): boolean {
		return held(this.rt.live.get(id)?.holds);
	}

	/** Change the run's holds in a commit of their document; Bench hears the change. */
	private async change(live: Live, change: (holds: Hold[]) => boolean): Promise<void> {
		let changed = false;
		const holds = await live.conversation.commit(async (tx) => {
			const doc = await tx.doc(HoldDoc, live.conversation.id);
			changed = change(doc.holds);
			return JSON.parse(JSON.stringify(doc.holds)) as Hold[];
		}, ctx);
		live.holds = holds;
		if (changed) this.rt.tell(live.row.id, { kind: "holds", value: holds });
	}
}
