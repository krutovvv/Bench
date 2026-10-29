// The evaluator at a result that did not fail: a command that exited 0 but printed an error, a
// search that found nothing it was meant to find, a traceback inside a log. The model reads "ok"
// and builds on it; three steps later it finds out. Sure that the result shows a failure, it is
// said with the result, before the next step leans on it.

import { cut, resultText } from "../builtin/turn.ts";
import { sure, type Point } from "./point.ts";

/** How sure the evaluator must be to speak. */
const SURE = 0.8;
/** The tools whose results say success by their words, not by an error. */
const TOOLS = new Set(["bash", "ssh_run", "grep", "glob"]);

export const quiet: Point = {
	name: "quiet",
	moment: "result",
	ask({ call, result }) {
		if (!call || result?.isError || !TOOLS.has(call.name)) return undefined;
		return {
			state: { call: `${call.name} ${cut(JSON.stringify(call.arguments ?? {}), 600)}`, output: cut(resultText(result), 3000) || "(nothing)" },
			questions: {
				quiet: {
					type: "choice",
					instructions: "The agent's call (call) did not fail as a call. Does its output (output) show that what it was meant to do worked?",
					// "worked" first: the model leans to the first option, and saying nothing is the safe side.
					criteria: {
						worked: "Yes, or there is nothing to tell: the output is what such a call gives when it works.",
						failed: "No: the output shows a failure — an error message, a traceback, 'not found', 'permission denied', a failed test — or it is empty where the call was meant to find or print something.",
					},
				},
			},
		};
	},
	apply(_seen, answers) {
		return sure(answers.quiet, SURE) === "failed"
			? { note: "[Evaluator: this output shows a failure, though the call did not fail: read it as one before the next step leans on it.]" }
			: undefined;
	},
};
