// The evaluator on the way: every so many steps of a turn it is asked whether the last steps still
// work on the person's request, have gone off to something else, go round in circles, or the work
// is done. Anything but "on track", when it is sure, is said to the model with the result of the
// step it came after. Unsure but leaning the same way off course at two checks running — drifting
// at 0.42–0.47 four times over twenty minutes, said nothing at 0.7 — it is a softer line with the
// person's request, a turn of the head rather than a stop. The model decides what to do.

import { cut } from "../builtin/turn.ts";

import { sure, type Point } from "./point.ts";

/** Steps between two checks: a local model that wanders loses minutes a step. */
export const DRIFT_EVERY = 5;
/** How sure the evaluator must be to speak. */
const SURE = 0.7;
/** Leaning off course this much, at two checks running, is a softer line. */
const LEAN = 0.4;

const SAY: Record<string, string> = {
	drifting:
		"[Evaluator: your last steps work on something the person did not ask for. Re-read their request; go back to it, or answer them now with what is done and why you went further.]",
	looping:
		"[Evaluator: your last steps repeat the same actions without getting closer. Stop repeating them: try another way, or tell the person where you are stuck.]",
	done: "[Evaluator: what the person asked for looks done. Unless something they asked for is still missing, stop and give them your final answer.]",
};

const LEANING: Record<string, (request: string) => string> = {
	drifting: (r) => `[Evaluator: your last steps may be moving away from what the person asked: «${r}». Check that your next step serves it.]`,
	looping: (r) => `[Evaluator: your last steps may be going round without getting closer to: «${r}». If the next step is like the last ones, take another way.]`,
};

export const course: Point = {
	name: "course",
	moment: "result",
	ask({ turn }) {
		if (!turn.request || turn.steps - turn.checkedAt < DRIFT_EVERY) return undefined;
		turn.checkedAt = turn.steps;
		return {
			questions: {
				course: {
					type: "choice",
					instructions: "An agent is working on the person's request. Judging by its last steps, where is its work now?",
					// "on_track" first: the model leans to the first option, and staying quiet is the safe side.
					criteria: {
						on_track: "The last steps work directly toward what the person asked for.",
						drifting:
							"The last steps work on something the person did not ask for: a side problem, a helper or test tool of the agent's own, polishing beyond the request.",
						looping: "The last steps repeat the same actions, commands or edits without getting closer.",
						done: "What the person asked for already looks done; the agent goes on checking or polishing instead of answering.",
					},
				},
			},
		};
	},
	apply({ turn }, answers) {
		const c = sure(answers.course, SURE);
		if (c && SAY[c]) {
			turn.leaning = "";
			return { note: SAY[c] };
		}
		const p = answers.course?.probabilities ?? {};
		const lean = (["drifting", "looping"] as const).find((k) => Number(p[k] ?? 0) >= LEAN) ?? "";
		const again = !!lean && turn.leaning === lean;
		turn.leaning = again ? "" : lean;
		return again ? { note: LEANING[lean](cut(turn.request, 300)) } : undefined;
	},
};
