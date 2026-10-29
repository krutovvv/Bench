// The evaluator at a message the person sends while the agent works. Bench queues it for when the
// run answers; when it corrects the work under way ("not that file", "in the other project", "stop
// using X"), the run goes on the wrong way to its end first — on a local model, many minutes of it.
// Asked whether the message corrects the current work or is a next task, a sure "corrects" joins
// the run after its current tools instead (Pi's steer), as the person's "send now" would.

import { turnOf } from "../turn/turn.ts";
import { ask, judgeActive } from "./client.ts";
import { baseState, sure } from "./point.ts";
import { cut } from "../turn/turn.ts";

/** How sure the evaluator must be: a wrong steer breaks into work the person meant to let finish. */
const SURE = 0.8;

/** Whether `text`, sent while the conversation's run works, corrects that work. */
export async function correctsTheWork(conversation: string, text: string, signal?: AbortSignal): Promise<boolean> {
	if (!judgeActive(conversation) || !text.trim()) return false;
	const answers = await ask(
		conversation,
		"correction",
		{ ...baseState(turnOf(conversation), 10), new_message: cut(text, 2000) },
		{
			correction: {
				type: "choice",
				instructions: "While the agent works on the request (its last steps are in the state), the person sent a new message (new_message). What is it to the work under way?",
				// "next" first: the model leans to the first option, and waiting for the run to answer is what the person chose.
				criteria: {
					next: "Something for after: a next task, a question, an addition that does not change what is being done now.",
					corrects: "A correction of the work under way: it says the agent is doing something wrong now, in the wrong place, or should do it otherwise.",
				},
			},
		},
		signal,
	);
	return sure(answers?.correction, SURE) === "corrects";
}
