// The evaluator at a command, with approvals on ("manual"): a command the patterns of the default
// extension let through (`DANGEROUS`) is judged too — reads only, changes the project's files, or
// can do harm beyond them (deletes, overwrites, rewrites history, reaches other machines or
// accounts). Harm, when it is fairly sure, asks the person first, as a pattern's hit does. It only
// adds questions: a pattern's hit is asked whatever the evaluator says.

import { cut } from "../builtin/turn.ts";
import { sure, type Point } from "./point.ts";

/** How sure of harm the evaluator must be to ask the person (asking costs little). */
const SURE = 0.5;

export const risk: Point = {
	name: "risk",
	moment: "call",
	ask({ call }) {
		const command = String(call?.arguments?.command ?? "");
		if (!command) return undefined;
		return {
			state: { command: cut(command, 3000) },
			questions: {
				risk: {
					type: "choice",
					instructions: "The agent is about to run this shell command. What can it do?",
					criteria: {
						reads: "It only reads or reports: files, state, the network; nothing changes.",
						changes: "It changes the project's own files or builds them, in a way that is easy to undo.",
						harm: "It can do harm that is hard to undo: deletes or overwrites data, rewrites git history, changes the system or its services, other machines, accounts, money, or sends something out.",
					},
				},
			},
		};
	},
	apply(_seen, answers) {
		return sure(answers.risk, SURE) === "harm" ? { approve: "the evaluator judged it hard to undo" } : undefined;
	},
};
