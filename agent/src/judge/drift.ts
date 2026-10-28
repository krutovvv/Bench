// The evaluator on the way: every so many steps of a turn it is asked whether the last steps still
// work on the person's request, have gone off to something else, go round in circles, or the work
// is done. Anything but "on track", when it is sure, is said to the model with the result of the
// step it came after, as the plan's reminder is (`builtin/ask.ts`). The model decides what to do.

import { cut, turnOf } from "../builtin/turn.ts";
import { ask } from "./client.ts";
import { host } from "../host.ts";

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

/** A note for the model when it is time to check and the course is off (`afterTool`). */
export async function checkCourse(conversation: string, signal?: AbortSignal): Promise<string | undefined> {
	const t = turnOf(conversation);
	if (!t.request || t.steps - t.checkedAt < DRIFT_EVERY) return undefined;
	t.checkedAt = t.steps;
	const answers = await ask(
		conversation,
		"drift",
		{ request: cut(t.request, 4000), plan: t.plan, steps_so_far: t.steps, last_steps: t.recent.slice(-DRIFT_EVERY) },
		{
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
		signal,
	);
	const a = answers?.course;
	const p = a?.probabilities?.[a.choice] ?? 0;
	if (!a || a.choice === "on_track" || p < SURE || !SAY[a.choice]) return undefined;
	host().count(conversation, "nudge");
	return SAY[a.choice];
}
