// The evaluator on the way: every so many steps of a turn it is asked whether the last steps still
// work on the person's request, have gone off to something else, go round in circles, or the work
// is done. Anything but "on track", when it is sure, is said to the model with the result of the
// step it came after. The model decides what to do.

import { sure, type Point } from "./point.ts";

/** Steps between two checks. */
export const DRIFT_EVERY = 20;
/** How sure the evaluator must be to speak. */
const SURE = 0.7;

const SAY: Record<string, string> = {
	drifting:
		"[Evaluator: your last steps work on something the person did not ask for. Re-read their request; go back to it, or answer them now with what is done and why you went further.]",
	looping:
		"[Evaluator: your last steps repeat the same actions without getting closer. Stop repeating them: try another way, or tell the person where you are stuck.]",
	done: "[Evaluator: what the person asked for looks done. Unless something they asked for is still missing, stop and give them your final answer.]",
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
	apply(_seen, answers) {
		const c = sure(answers.course, SURE);
		return c && SAY[c] ? { note: SAY[c] } : undefined;
	},
};
