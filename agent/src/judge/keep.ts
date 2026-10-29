// The evaluator at a request that would overflow the model's window (`builtin/window.ts`): of the
// earlier results and call arguments worth cutting, which does the agent's next step still need?
// The window keeps those and cuts the others first — before, the largest went first, needed or not.

import { cut } from "../builtin/turn.ts";
import { ask, judgeActive } from "./client.ts";
import { baseState } from "./point.ts";
import { turnOf } from "../builtin/turn.ts";

/** How many blocks one request weighs, and how much of each the evaluator reads. */
const PER_REQUEST = 12;
const SEEN = 700;

const NEXT_STEP = "The conversation is too long for the model. Which of these earlier results (in the state, under earlier) does the agent's next step most need to keep? none, if it needs none of them.";
const AFTER_SUMMARY =
	"The conversation is being summed up to go on in a shorter window. Which of these earlier results (in the state, under earlier) will the rest of the work need word for word — exact content, paths, ids, numbers a summary would lose? none, if it needs none of them.";

/** Each block's probability of being the one the next step needs (by batch); undefined: no evaluator or no answer. */
export async function neededness(conversation: string, blocks: string[], signal?: AbortSignal, instructions = NEXT_STEP): Promise<number[] | undefined> {
	if (!judgeActive(conversation) || blocks.length < 2) return undefined;
	const out: number[] = [];
	for (let from = 0; from < blocks.length; from += PER_REQUEST) {
		const batch = blocks.slice(from, from + PER_REQUEST);
		const answers = await ask(
			conversation,
			"keep",
			{ ...baseState(turnOf(conversation), 8), earlier: Object.fromEntries(batch.map((b, i) => [`b${from + i}`, cut(b, SEEN)])) },
			{
				keep: {
					type: "choice",
					instructions,
					criteria: { ...Object.fromEntries(batch.map((_, i) => [`b${from + i}`, `Result b${from + i} of earlier`])), none: "The next step needs none of them." },
				},
			},
			signal,
		);
		if (!answers) return undefined;
		const probs = answers.keep?.probabilities ?? {};
		batch.forEach((_, i) => out.push(Number(probs[`b${from + i}`] ?? 0)));
	}
	return out;
}

/** At compaction: how many of the latest results are weighed, the least one must have, how much is kept. */
const WEIGHED = 48;
const KEEP_AT = 0.3;
const KEEP_CHARS = 8000;
const KEEP_ONE = 3000;

/**
 * The results a summary would lose that the work still needs, word for word, as a section to add to
 * it; "" when none, or no evaluator. `blocks` are the results in order, each a call and its text.
 */
export async function keptThrough(conversation: string, blocks: string[], signal?: AbortSignal): Promise<string> {
	const from = Math.max(0, blocks.length - WEIGHED);
	const weighed = blocks.slice(from);
	const p = await neededness(conversation, weighed, signal, AFTER_SUMMARY).catch(() => undefined);
	if (!p) return "";
	const chosen: number[] = [];
	let room = KEEP_CHARS;
	for (const i of p.map((_, i) => i).filter((i) => p[i] >= KEEP_AT).sort((a, b) => p[b] - p[a])) {
		const size = Math.min(weighed[i].length, KEEP_ONE);
		if (size > room) continue;
		room -= size;
		chosen.push(i);
	}
	if (!chosen.length) return "";
	return `\n\n## Kept word for word\nResults the evaluator judged the rest of the work needs exactly:\n\n${chosen
		.sort((a, b) => a - b)
		.map((i) => cut(weighed[i], KEEP_ONE))
		.join("\n\n")}`;
}
