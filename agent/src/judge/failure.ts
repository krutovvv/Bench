// The evaluator at a failed step: a tool's error is judged — a passing failure worth one more try,
// a call to fix from the error, a way that does not work here, or something only the person can
// give — and, when sure, said to the model with the error. Loops start here: the same failing
// command again, a blocked site tried a fifth way.

import { cut, resultText } from "../builtin/turn.ts";
import { sure, type Point } from "./point.ts";

/** How sure the evaluator must be to speak. */
const SURE = 0.6;

const SAY: Record<string, string> = {
	retry: "[Evaluator: a passing failure (a timeout, the network): trying the same once more is reasonable — once.]",
	fix: "[Evaluator: the call itself was wrong (its arguments, a path, the syntax): fix it from the error before trying again.]",
	other_way: "[Evaluator: this way does not work here (blocked, missing, unsupported): do not repeat it or vary it; take another way.]",
	ask: "[Evaluator: this needs the person (access, a key, a login, a decision): ask them instead of trying more.]",
};

export const failure: Point = {
	name: "failure",
	moment: "result",
	ask({ call, result }) {
		if (!call || !result?.isError) return undefined;
		return {
			state: { failed_call: `${call.name} ${cut(JSON.stringify(call.arguments ?? {}), 600)}`, error: cut(resultText(result), 1500) },
			questions: {
				failure: {
					type: "choice",
					instructions: "The agent's last call failed with this error. What should the agent do about it?",
					// "go_on" first: the model leans to the first option, and saying nothing is the safe side.
					criteria: {
						go_on: "Nothing to add: the agent can see what to do from the error itself.",
						retry: "A passing failure (a timeout, a dropped connection): the same call may work once more.",
						fix: "The call was wrong (its arguments, a path, the syntax): fixed from the error, it will work.",
						other_way: "This way does not work here (blocked, missing, not supported), and the last steps already tried it: another way is needed.",
						ask: "Only the person can unblock it (access, a key, a login, a decision).",
					},
				},
			},
		};
	},
	apply(_seen, answers) {
		const c = sure(answers.failure, SURE);
		return c && SAY[c] ? { note: SAY[c] } : undefined;
	},
};
