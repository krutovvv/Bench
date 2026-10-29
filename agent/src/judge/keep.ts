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

/** Each block's probability of being the one the next step needs (by batch); undefined: no evaluator or no answer. */
export async function neededness(conversation: string, blocks: string[], signal?: AbortSignal): Promise<number[] | undefined> {
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
					instructions: "The conversation is too long for the model. Which of these earlier results (in the state, under earlier) does the agent's next step most need to keep? none, if it needs none of them.",
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
