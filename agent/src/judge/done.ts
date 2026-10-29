// The evaluator at the end of a turn: when the agent gives its answer after real work, it is asked
// whether the request is done. Sure that it is not — parts left undone, code changed and never
// run, work on something else — the agent is sent back once, with the reason, by Pi's `onYield`
// continuation: a message of the evaluator's own (`<evaluator>`), which the thread does not show
// and which does not start a new turn. A question to the person, a turn stopped by the step limit
// or a short one are left as they are.

import { EVAL_CLOSE, EVAL_OPEN, textOf } from "../runtime/journal.ts";
import { cut } from "../turn/turn.ts";
import { limitReached } from "../turn/steps.ts";
import { sure, type Point } from "./point.ts";

/** A turn with fewer steps is a conversation, not work: no check. */
const MIN_STEPS = 5;
/** How sure the evaluator must be to send the agent back. */
const SURE = 0.8;

const BACK: Record<string, string> = {
	unfinished: "Parts of the person's request, or of your plan, are not done though you can do them. Do them now; if one truly cannot be done, say which and why.",
	unchecked: "You changed code and did not build, test or run it after the last change. Check it now the way this project checks code, fix what fails, then answer.",
	off_target: "Your work does something other than what the person asked. Re-read their request and do what it asks.",
};

const answerText = (answer: any) => textOf(answer?.content ?? []).trim();

export const done: Point = {
	name: "done",
	moment: "answer",
	ask({ conversation, turn, answer }) {
		if (!turn.request || turn.continued || turn.steps < MIN_STEPS || limitReached(conversation)) return undefined;
		const text = answerText(answer);
		if (!text) return undefined;
		return {
			state: { final_answer: cut(text, 4000) },
			questions: {
				verdict: {
					type: "choice",
					instructions: "An agent worked on the person's request and wrote this final answer. Is the request done?",
					// "done" and "question" first: the model leans to the first options, and letting the answer stand is the safe side.
					criteria: {
						done: "The request is done, and the steps support what the answer says.",
						question: "The answer asks the person something, or reports a blocker only the person can resolve.",
						unfinished: "Parts of the request or of the plan are left undone though the agent could do them.",
						unchecked: "Code was changed but not built, tested or run after the last change.",
						off_target: "The work does something other than what the person asked.",
					},
				},
			},
		};
	},
	apply({ turn }, answers) {
		const v = sure(answers.verdict, SURE);
		if (!v || !BACK[v]) return undefined;
		turn.continued = true;
		return { continue: `${EVAL_OPEN}\n${BACK[v]}\n${EVAL_CLOSE}` };
	},
};
