// The evaluator at the answer's facts: when the agent's final answer gives numbers (prices, sizes,
// dates, counts) after work, it is asked whether what the turn's steps found supports them. Sure
// that some are not — none of the results says them — the agent is sent back once to check them
// or mark them unverified. Deep research's guard against a price written from memory.

import { EVAL_CLOSE, EVAL_OPEN, textOf } from "../runtime/journal.ts";
import { cut } from "../turn/turn.ts";
import { sure, type Point } from "./point.ts";

/** How sure the evaluator must be to send the answer back. */
const SURE = 0.8;
/** A turn with fewer steps found too little to check against. */
const MIN_STEPS = 3;

export const claims: Point = {
	name: "claims",
	moment: "answer",
	ask({ turn, answer }) {
		const text = textOf(answer?.content ?? []).trim();
		if (turn.continued || turn.steps < MIN_STEPS || !turn.found.length || !/\d/.test(text)) return undefined;
		return {
			// All the turn found; the evaluator's server keeps the parts nearest the question when it is more.
			state: { final_answer: cut(text, 4000), found: turn.found },
			questions: {
				claims: {
					type: "choice",
					instructions: "Do the results the agent's steps found (found) support the numbers and facts its final answer (final_answer) gives?",
					// "supported" first: the model leans to the first option, and letting the answer stand is the safe side.
					criteria: {
						supported: "Yes: what the answer states is in what the steps found, or is plainly the agent's own reasoning.",
						unsupported: "No: some numbers or facts in the answer are in none of the results.",
					},
				},
			},
		};
	},
	apply({ turn }, answers) {
		if (sure(answers.claims, SURE) !== "unsupported") return undefined;
		turn.continued = true;
		return {
			continue: `${EVAL_OPEN}\nSome numbers or facts in your answer are in none of the results your steps found. Check each against a source now; what you cannot confirm, mark as unverified. Then answer again.\n${EVAL_CLOSE}`,
		};
	},
};
