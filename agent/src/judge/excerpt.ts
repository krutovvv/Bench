// The evaluator at a long result: one larger than its share of the model's window
// (`turn/window.ts`) is cut into parts, and the evaluator is asked which of them holds what the
// request needs — the parts in the state, a choice over their names (and "none") per request, the
// parts its probabilities set apart kept (a part's text in a question's instructions or options
// was misread; in the state it is read whole); the model
// gets the needed parts in their order, the rest said in a line with their
// size. Without it, the middle of such a result was cut — and a page's price was as likely there
// as anywhere. Parts go in as many requests as the evaluator's window takes (a local server's is
// about 16K tokens), up to `MAX_REQUESTS`; beyond that the rest is left to the cut.

import { turnOf } from "../turn/turn.ts";
import { ask, judgeActive } from "./client.ts";
import { baseState } from "./point.ts";
import { host } from "../host.ts";

/** A part's length, and how much of parts one request carries: past the local server's context
 *  (`MAX_LEN` 16384) its shim shortens the state per question, and at 18 000 a part near the end
 *  was lost; at 9 000 it is read whole. */
const PART = 1500;
const PER_REQUEST = 9_000;
const MAX_REQUESTS = 16;
/** A part kept when the choice gives it at least this, and many times what its batch's parts
 *  usually get: the evaluator spreads little over the parts it sets apart (0.16 for the price
 *  among 0.01s, the rest to "none"), so standing out counts, not the number alone. */
const NEEDED = 0.05;
const STANDS_OUT = 5;

/** `text` cut to `budget` characters by what the request needs; undefined: no evaluator, or no answer. */
export async function excerpt(conversation: string, text: string, budget: number, signal?: AbortSignal): Promise<string | undefined> {
	if (!judgeActive(conversation)) return undefined;
	const parts = partsOf(text);
	const seen = parts.slice(0, Math.floor((PER_REQUEST * MAX_REQUESTS) / PART));
	const score: number[] = new Array(parts.length).fill(-1);
	const turn = turnOf(conversation);
	for (let from = 0; from < seen.length; from += Math.floor(PER_REQUEST / PART)) {
		const batch = seen.slice(from, from + Math.floor(PER_REQUEST / PART));
		// The parts in the state, short labels as the options: a part's own text as an option is read
		// from its start only (a price at its end went to the next part), the state is read whole.
		const answers = await ask(
			conversation,
			"excerpt",
			{ ...baseState(turn, 6), long_result: Object.fromEntries(batch.map((p, i) => [`p${from + i}`, p])) },
			{
				part: {
					type: "choice",
					instructions: "Which part of the tool's long result (in the state, under long_result) is the most useful for the person's request? none, if every part is navigation, boilerplate or off the subject.",
					// "none" last: first, it took nearly all the probability from a part that held the
					// very price asked for.
					criteria: { ...Object.fromEntries(batch.map((_, i) => [`p${from + i}`, `Part p${from + i} of long_result`])), none: "Every part is navigation, boilerplate or off the subject." },
				},
			},
			signal,
		);
		if (!answers) return undefined;
		const probs = answers.part?.probabilities ?? {};
		const got = batch.map((_, i) => Number(probs[`p${from + i}`] ?? 0));
		const usual = Math.max([...got].sort((a, b) => a - b)[Math.floor(got.length / 2)] ?? 0, 0.005);
		got.forEach((p, i) => {
			score[from + i] = p >= NEEDED && p >= usual * STANDS_OUT ? p : 0;
		});
	}
	// The likeliest parts until the budget is spent, then shown in their order.
	const keep = new Set<number>();
	let room = budget;
	for (const i of score.map((s, i) => [s, i] as const).filter(([s]) => s > 0).sort((a, b) => b[0] - a[0]).map(([, i]) => i)) {
		if (parts[i].length > room) continue;
		keep.add(i);
		room -= parts[i].length;
	}
	if (!keep.size) return undefined;
	host().count(conversation, "nudge");
	const out: string[] = [`[Evaluator: a long result (${text.length} characters); the ${keep.size} of its ${parts.length} parts the request needs, in their order]`];
	let skipped = 0;
	parts.forEach((p, i) => {
		if (keep.has(i)) {
			if (skipped) out.push(`[… ${skipped} characters left out …]`);
			skipped = 0;
			out.push(p);
		} else skipped += p.length;
	});
	if (skipped) out.push(`[… ${skipped} characters left out …]`);
	return out.join("\n");
}

/** The text in parts of about `PART` characters, cut at line ends where it can be. */
export function partsOf(text: string): string[] {
	const parts: string[] = [];
	let at = 0;
	while (at < text.length) {
		let end = Math.min(text.length, at + PART);
		if (end < text.length) {
			const nl = text.lastIndexOf("\n", end);
			if (nl > at + PART / 2) end = nl + 1;
		}
		parts.push(text.slice(at, end));
		at = end;
	}
	return parts;
}

