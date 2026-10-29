// The evaluator before a call: when the agent calls a tool it called just before, it is asked
// whether this call repeats one that failed, or one whose result it already has, with nothing it
// depends on changed since — the loop of a run that drives the same probe again and again. Sure of
// it, the call is refused with the reason, once: the same call again goes through, as the agent
// may know better.

import { cut } from "../builtin/turn.ts";
import type { Point } from "./point.ts";

/** How sure the evaluator must be to stop a call: a wrong stop costs a step, so it must be very. */
const SURE = 0.85;
/** How far back a call of the same tool makes the evaluator look. */
const BACK = 3;

const SAY: Record<string, string> = {
	same_failed: "[Evaluator: this call repeats one that just failed, and nothing it depends on has changed since. It is not run: change the call, take another way, or tell the person what blocks you.]",
	same_result: "[Evaluator: this call repeats one whose result you already have, and nothing it depends on has changed since. It is not run: use that result, or change something first.]",
};

const keyOf = (call: { name: string; arguments?: any }) => `${call.name} ${JSON.stringify(call.arguments ?? {})}`;

export const repeat: Point = {
	name: "repeat",
	moment: "call",
	ask({ call, turn }) {
		if (!call || call.name === "todo" || turn.stopped === keyOf(call)) return undefined;
		if (!turn.recent.slice(-BACK).some((l) => l.startsWith(`${call.name} `))) return undefined;
		return {
			state: { next_call: `${call.name} ${cut(JSON.stringify(call.arguments ?? {}), 800)}`, recent_results: turn.evidence.slice(-BACK - 1) },
			questions: {
				repeat: {
					type: "choice",
					instructions: "The agent is about to make the next call (next_call). Judging by its last steps and their results, what is this call?",
					// "new" first: the model leans to the first option, and letting the call run is the safe side.
					criteria: {
						new: "A new step: different from the last calls, or something it depends on changed since (a file edited, a page moved on).",
						same_failed: "The same as a call that just failed, with nothing changed since: it will fail the same way.",
						same_result: "The same as a call whose result the agent already has, with nothing changed since: it will give the same.",
					},
				},
			},
		};
	},
	apply({ call, turn }, answers) {
		// Both kinds are a repeat: their sum is how sure it is one (0.79 + 0.17 for the same probe
		// run again on the local server), the likelier of the two says which.
		const p = answers.repeat?.probabilities ?? {};
		const failed = Number(p.same_failed ?? 0);
		const known = Number(p.same_result ?? 0);
		const c = failed >= known ? "same_failed" : "same_result";
		if (!call || failed + known < SURE) return undefined;
		turn.stopped = keyOf(call);
		return { block: SAY[c] };
	},
};
