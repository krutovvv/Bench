// The evaluator on when to compact. Pi compacts at a share of the window, wherever the work is:
// mid-edit, between a failed run and its fix, and the summary then cuts what the next step was
// about. Once the context is well on its way there, the evaluator is asked after a step whether a
// part of the work has just ended (a plan item finished, a sub-task done and checked, a question
// answered); sure of it, the conversation is compacted now, in the background — Pi places the
// summary at the next boundary and the work goes on meanwhile. Pi's own threshold stays the floor.

import { DEFAULT_COMPACTION_POLICY } from "@earendil-works/pi-durable";
import { compactionAt, compactionPolicy } from "../config.ts";
import { usedTokens, windowTokens } from "../turn/window.ts";
import { host } from "../host.ts";
import { slot } from "../turn/kept.ts";
import { sure, type Point } from "./point.ts";

/** From this share of Pi's threshold the evaluator is asked. */
const FROM = 0.6;
/** Below this share again (a compaction placed), it may compact once more. */
const REARM = 0.5;
/** How sure the evaluator must be that a part of the work just ended. */
const SURE = 0.8;
/** Steps between two questions. */
const EVERY = 3;

/** Per conversation: compacted at this band already (until it falls under `REARM`), and the step last asked at. */
const band = slot(() => ({ fired: false, askedAt: -EVERY }));

/** Where Pi compacts: its threshold, and where its background compaction starts — read from the
 *  policy it uses for the project (its share of the window, `compactionPolicy`, else Pi's default)
 *  as Pi reads it: the window less the reserve, less the background's lead. */
export function thresholds(agent: string, window = windowTokens()): { at: number; background: number } {
	const share = compactionAt(agent);
	const policy = share ? compactionPolicy(share, window) : DEFAULT_COMPACTION_POLICY;
	const at = window - policy.reserveTokens;
	return { at, background: at - policy.backgroundTokens };
}

export const boundary: Point = {
	name: "boundary",
	moment: "result",
	ask({ conversation, turn }) {
		const agent = host().thread(conversation)?.agent;
		if (!agent || !turn.request) return undefined;
		const used = usedTokens(conversation);
		const { at, background } = thresholds(agent);
		const b = band.get(conversation);
		if (used < at * REARM) b.fired = false;
		// Past where Pi's background compaction starts, Pi has it in hand.
		if (b.fired || used < at * FROM || used >= background) return undefined;
		if (turn.steps - b.askedAt < EVERY) return undefined;
		b.askedAt = turn.steps;
		return {
			questions: {
				boundary: {
					type: "choice",
					instructions: "The conversation will soon be summed up to fit the model's window. Judging by the plan and the last steps, is the agent in the middle of a part of the work, or has one just ended?",
					// "mid_work" first: the model leans to the first option, and leaving it to Pi's threshold is the safe side.
					criteria: {
						mid_work: "In the middle: an edit, a run and its fix, a search whose results it is reading — the next step leans on the last ones in detail.",
						ended: "A part just ended: a plan item finished, a sub-task done and checked, a question answered — the next step starts something that needs only its outcome.",
					},
				},
			},
		};
	},
	apply({ conversation }, answers) {
		if (sure(answers.boundary, SURE) !== "ended") return undefined;
		band.get(conversation).fired = true;
		host().compact(conversation, "A part of the work has just ended: sum up its outcome, and keep what the next part needs.");
		return {};
	},
};
