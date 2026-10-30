// The evaluator on the way: at every step of a turn from the second (near free, in the step's own
// request) it is asked whether the last steps still work on the person's request, have gone off to
// something else, go round in circles, or the work is done. It speaks by what has gathered, as one
// step cannot tell a side trip from a needed detour. Sure, it is said at once. Unsure but leaning the
// same way off course at three of the last four checks — drifting at 0.42–0.47 four times over twenty
// minutes, said nothing at 0.7 — it is a softer line with the person's request. Off course again after
// either, a few steps on so the agent could turn, it is told to stop and answer. The model decides
// what to do.

import { cut } from "../turn/turn.ts";
import { limitReached } from "../turn/steps.ts";

import { sure, type Point } from "./point.ts";

/** The first step of a turn that is judged: one step says nothing of a course. */
const FIRST_CHECK = 2;
/** How sure the evaluator must be to speak. */
const SURE = 0.7;
/** Leaning off course this much counts toward a softer line. */
const LEAN = 0.4;
/** The checks a leaning is counted over, and how many of them must lean the same way. */
const LEAN_WINDOW = 4;
const LEAN_ENOUGH = 3;
/** Steps after a word from it before another: the agent gets the room to turn. */
const ROOM_TO_TURN = 3;

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
		// Past the step limit the limit has told it to answer (`turn/steps.ts`): nothing to add.
		if (!turn.request || turn.steps < FIRST_CHECK || turn.steps === turn.checkedAt || limitReached(conversation)) return undefined;
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
		turn.leanings = [...turn.leanings, lean].slice(-LEAN_WINDOW);
		if (turn.saidAt && turn.steps - turn.saidAt < ROOM_TO_TURN) return undefined;
		const say = (note: string) => {
			turn.saidAt = turn.steps;
			turn.leanings = [];
			return { note };
		};
		// Told once already, and still off: the word becomes a stop.
		if (turn.warned && (c === "drifting" || c === "looping" || lean)) return say(STOP(cut(turn.request, 300)));
		if (c && SAY[c]) {
			if (c !== "done") turn.warned = true;
			return say(SAY[c]);
		}
		if (lean && turn.leanings.filter((l) => l === lean).length >= LEAN_ENOUGH) {
			turn.warned = true;
			return say(LEANING[lean](cut(turn.request, 300)));
		}
		return undefined;
	},
};
