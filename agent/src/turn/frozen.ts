// The prompt's sections held as they were until the next compaction. pi-ai folds every change of
// a section into the leading system message on an OpenAI-compatible server, so a note added, a
// skill written or the date turning over rewrote the head of every later request — and on the
// model server, its whole prefix cache (on a local hybrid model, back to its checkpoint at zero:
// a minute of prefill). A compaction rewrites what follows the head anyway: there they are read
// anew. What changed meanwhile the agent mostly did itself (a note, a skill, SYSTEM.md); the
// person's own edits in Settings reach the thread at its next compaction or in a new thread.

import { slot } from "./kept.ts";

const held = slot(() => new Map<string, string | undefined>());

/** The section `name` of the conversation as first read since its last compaction. */
export async function frozen(conversation: string, name: string, read: () => string | undefined | Promise<string | undefined>): Promise<string | undefined> {
	const sections = held.get(conversation);
	if (!sections.has(name)) sections.set(name, await read());
	return sections.get(name);
}

/** A compaction was placed: the sections are read anew. */
export function thaw(conversation: string): void {
	held.drop(conversation);
}
