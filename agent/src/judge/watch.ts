// The evaluator at a background process's log (`process log`): the agent polls a build or a server
// and reads the tail each time, a step a poll. Asked what the tail shows — still working, finished,
// failed, stuck — a sure answer is said with it: go on with the result, fix it, stop it, or leave
// it be and do other work instead of polling again at once.

import { cut, resultText } from "../builtin/turn.ts";
import { sure, type Point } from "./point.ts";

/** How sure the evaluator must be to speak. */
const SURE = 0.8;

const SAY: Record<string, string> = {
	working: "[Evaluator: the process is still at work and nothing is wrong: do other work, or wait, before reading its log again.]",
	finished: "[Evaluator: its output shows the work is done: go on with its result; no need to watch it more.]",
	failed: "[Evaluator: its output shows it failed: read the error, fix it and start it again, rather than waiting on it.]",
	stuck: "[Evaluator: its output has stopped short of its end (a hang, a prompt waiting for input, a retry loop): stop it or find out why.]",
};

export const watch: Point = {
	name: "watch",
	moment: "result",
	ask({ call, result }) {
		if (call?.name !== "process" || call.arguments?.action !== "log" || result?.isError) return undefined;
		const text = resultText(result);
		// Only a process still running: a finished one says so itself.
		if (!/"state":"running"/.test(text)) return undefined;
		return {
			state: { process_log: cut(text.slice(-4000), 4000) },
			questions: {
				watch: {
					type: "choice",
					instructions: "This is the tail of a background process's output (process_log), still running. What does it show?",
					criteria: {
						working: "It is at work and going well: progress lines, a server serving.",
						finished: "Its work is done (a build or test run ended, a server ready and waiting), though the process is still up.",
						failed: "It failed: an error, a traceback, a failed build or test.",
						stuck: "It stopped short of its end: no progress, a prompt waiting for input, the same retry again and again.",
					},
				},
			},
		};
	},
	apply(_seen, answers) {
		const c = sure(answers.watch, SURE);
		return c && SAY[c] ? { note: SAY[c] } : undefined;
	},
};
