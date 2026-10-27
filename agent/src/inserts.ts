// A thread is a document: the person's messages, the agent's answers, and the person's inserts —
// tools put into the conversation with "/" (the terminal, the task lists, the utilities, the
// browser's tab). Each insert is a piece of the person's side of the thread, in
// Pi's terms:
//
// - an entry `bench.insert` {id, key} in the transcript, written where it was put (a write: the
//   model doesn't see it, it is never queued, so it lands at once even mid-run);
// - its live state in the conversation's document `bench.inserts` (which list, which
//   tool, which tab; folded; removed), changed in commits.
//
// The entry fixes where it is; the document what it is now. A removed insert keeps its entry (the
// transcript is immutable) and is marked removed in the document. The model learns what is
// inserted from the `inserts` prompt section, and what each one shows from the <block-state>
// Bench sends with each message.

import { defineDoc, defineEntry } from "@earendil-works/pi-durable";
import type { JsonObject, JsonValue } from "@earendil-works/chord";

export interface InsertState {
	key: string;
	state: JsonObject;
	folded: boolean;
	removed: boolean;
	/**
	 * Where it sits, when that is not where its entry was written: an insert put while a message
	 * still waited in the composer belongs after that message, and Pi's transcript cannot move an
	 * entry. The document remembers the place; without it the entry's own place decides.
	 */
	after?: number;
}

export const InsertEntry = defineEntry<{ id: string; key: string }>("bench.insert");

export const Inserts = defineDoc<{ items: Record<string, InsertState> }>({
	kind: "bench.inserts",
	version: 1,
	scope: "conversation",
	history: "latest",
	fork: "current",
	initial: () => ({ items: {} }),
	// Small: every change is a complete base.
	checkpointWhen: () => true,
} as any);

/** An insert as Bench draws it: where it sits (after how many of the person's messages) and its state. */
export interface InsertView extends InsertState {
	id: string;
	after: number;
}

/** The inserts of a transcript, in order, with their state; removed ones left out. */
export function insertsOf(entries: readonly any[], doc: { items: Record<string, InsertState> } | undefined): InsertView[] {
	const out: InsertView[] = [];
	let users = 0;
	for (const e of entries) {
		if (e.kind === "pi.user") users++;
		if (e.kind !== InsertEntry.kind && e.kind !== "bench.insert") continue;
		const data = e.data as { id: string; key: string };
		const s = doc?.items?.[data.id];
		if (s?.removed) continue;
		const moved = typeof s?.after === "number" && Number.isInteger(s.after) && s.after >= 0;
		out.push({
			id: data.id,
			key: s?.key ?? data.key,
			state: s?.state ?? {},
			folded: s?.folded ?? false,
			removed: false,
			after: moved ? (s as any).after : users,
		});
	}
	return out;
}

/** A partial change of one insert's state, as Bench sends it. Unknown fields are ignored. */
export function patchInsert(doc: { items: Record<string, InsertState> }, id: string, patch: { key?: unknown; state?: unknown; folded?: unknown; removed?: unknown; after?: unknown }): boolean {
	const cur = doc.items[id];
	if (!cur) return false;
	if (typeof patch.key === "string" && patch.key) cur.key = patch.key;
	if (patch.state && typeof patch.state === "object" && !Array.isArray(patch.state)) cur.state = patch.state as JsonObject;
	if (typeof patch.folded === "boolean") cur.folded = patch.folded;
	if (patch.removed === true) cur.removed = true;
	if (typeof patch.after === "number" && Number.isInteger(patch.after) && patch.after >= 0) cur.after = patch.after;
	return true;
}

/** The kind of an insert's key: "ssh:local" → "ssh". */
export const kindOf = (key: string) => key.split(":")[0];

export type { JsonValue };
