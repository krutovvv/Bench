// The evaluator on the way: every so many steps of a turn it is asked whether the last steps still
// work on the person's request, have gone off to something else, go round in circles, or the work
// is done. Anything but "on track", when it is sure, is said to the model with the result of the
// step it came after. Unsure but leaning the same way off course at two checks running — drifting
// at 0.42–0.47 four times over twenty minutes, said nothing at 0.7 — it is a softer line with the
// person's request, a turn of the head rather than a stop. Off course again after either, it is
// told to stop and answer. When the message wanted words, not work (`intent.ts`), the course is
// checked every 3 steps. The model decides what to do.

import { cut } from "../turn/turn.ts";
import { limitReached } from "../turn/steps.ts";

import { sure, type Point } from "./point.ts";

/** Steps between two checks: a local model that wanders loses minutes a step. */
export const DRIFT_EVERY = 5;
/** How sure the evaluator must be to speak. */
const SURE = 0.7;
/** Leaning off course this much, at two checks running, is a softer line. */
const LEAN = 0.4;
/** Steps between checks when the message wanted words, not work (`intent.ts`): a few steps are already many. */
const WORDS_EVERY = 3;
const WORDS = new Set(["question", "approval", "choice"]);

/** Off course again after a word about it: the agent did not turn, so it is told to stop. */
const STOP = (r: string) =>
	`[Evaluator: you were told your steps leave what the person asked («${r}»), and they still do. Stop here: answer the person now — what is done, and what more you would do, as an offer.]`;

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
	ask({ conversation, turn }) {
		const every = WORDS.has(turn.intent) ? WORDS_EVERY : DRIFT_EVERY;
		// Past the step limit the limit has told it to answer (`turn/steps.ts`): nothing to add.
		if (!turn.request || turn.steps - turn.checkedAt < every || limitReached(conversation)) return undefined;
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
		const p = answers.course?.probabilities ?? {};
		const lean = (["drifting", "looping"] as const).find((k) => Number(p[k] ?? 0) >= LEAN) ?? "";
		// Told once already, and still off: the word becomes a stop.
		if (turn.warned && (c === "drifting" || c === "looping" || lean)) return { note: STOP(cut(turn.request, 300)) };
		if (c && SAY[c]) {
			turn.leaning = "";
			if (c !== "done") turn.warned = true;
			return { note: SAY[c] };
		}
		const again = !!lean && turn.leaning === lean;
		turn.leaning = again ? "" : lean;
		if (again) turn.warned = true;
		return again ? { note: LEANING[lean](cut(turn.request, 300)) } : undefined;
	},
};
