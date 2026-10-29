// The evaluator at every message of the person, before the agent's first step on it: what is the
// message — and so how much work it asks for. An agent that does everything right does too much:
// asked nothing new, it re-checks finished work and builds what it thinks would be better.
// In a thread on Things 3 the agent said "the tools are final"; the person answered "the dash for
// duration is fine, the deadline that won't clear doesn't matter, the trash I'll empty myself" and
// named what mattered most to them — and the agent spent 36 steps building three things nobody
// asked for, all of them right. The message was an approval; it wanted a few lines.
//
// So each message is read first, against the agent's answer it follows, as one of a few kinds a
// person sends an agent; a sure reading is a line with the request saying how far the turn goes,
// and the turn keeps it: every later question of the evaluator sees it (`baseState`), and the
// course is checked sooner when the message wanted words, not work (`drift.ts`).

import { cut } from "../turn/turn.ts";
import type { Point } from "./point.ts";

/** How sure the evaluator must be of the kind. */
const SURE = 0.6;

/** What each kind asks for: the evaluator's later questions read it, the model reads its line. */
export const INTENT_SCOPE: Record<string, string> = {
	work: "a task: do what it names, all of it, and nothing it does not name",
	question: "a question: answer it in words; look something up only when the answer needs a fact not yet known",
	approval: "an approval or feedback on work already done: acknowledge it in a few lines; change nothing, start nothing",
	correction: "a correction of the last work: change exactly what it names, nothing more",
	choice: "the person's answer to the agent's question or offer: do what they chose, no more",
};

const SAY: Record<string, string> = {
	work: "it is a task: do what it names, all of it — and nothing it does not name. Something worth doing beyond it goes into one line at the end of your answer, as an offer, not done",
	question: "it is a question: answer it in words, from what you know and what this thread found; a tool only for a fact the answer needs and you lack. No changes",
	approval: "it approves or comments on work already done: acknowledge it in a few lines — what is settled, what is left if anything. Do not re-check, extend or improve the work; offer it in a line if it matters",
	correction: "it corrects your last work: change exactly what it names and check that; leave the rest as it is",
	choice: "it answers your question or offer: do what the person chose, as they chose it, and no more",
};

export const intent: Point = {
	name: "intent",
	moment: "request",
	ask({ turn }) {
		if (!turn.request || turn.steps > 0 || turn.intentAsked) return undefined;
		turn.intentAsked = true;
		return {
			state: { message: cut(turn.request, 4000), agent_answer_before_it: turn.previous || "(none: the message opens the thread)" },
			questions: {
				intent: {
					type: "choice",
					instructions:
						"The person sent this message (message) to their agent, after the agent's answer (agent_answer_before_it). What is the message, by what the person wants from the agent now?",
					// "work" first: the model leans to the first option, and working as asked is the classic way.
					criteria: {
						work: "A task: it asks the agent to make, change, fix, find or set up something new.",
						question: "A question: it asks to explain or tell something (what, why, how, is it), not to change anything.",
						approval: "An approval or feedback on what the agent did or proposed: agreeing, accepting its limits, saying what matters or what is fine — it asks for nothing new to be done.",
						correction: "A correction: it says the last work is wrong or must differ in a named way.",
						choice: "An answer to the agent's question or offer: a choice among its options, a yes or a no.",
					},
				},
			},
		};
	},
	apply({ turn }, answers) {
		const a = answers.intent;
		const p = a?.probabilities?.[a.choice] ?? 0;
		if (!a || p < SURE || !SAY[a.choice]) return undefined;
		turn.intent = a.choice;
		return { hint: `[Evaluator, for this message: ${SAY[a.choice]}.]` };
	},
};
