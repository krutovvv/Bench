// The step limit: a project's most steps in one turn (Settings → Model → Step limit; none by
// default). A brake, not a judgement — it ends a run that may have lost its way, with an answer:
// past the limit every call is refused with the reason, and every request to the model carries
// the instruction to write the person what is done and what is left. A model that keeps calling
// tools all the same is stopped outright a few calls later.

import { readAgent } from "../config.ts";
import { host } from "../host.ts";
import { countStep, turnOf } from "./turn.ts";

/** Refused calls after which the run is stopped without an answer. */
const STOP_AFTER = 6;

/** The limit the conversation's project sets; undefined: none. */
export function stepLimit(conversation: string): number | undefined {
	const info = host().thread(conversation);
	const n = info ? readAgent(info.agent).stepLimit : undefined;
	return typeof n === "number" && n > 0 ? n : undefined;
}

const reached = (limit: number) =>
	`[Step limit: this project allows ${limit} steps in one turn and they are used. Call no more tools. ` +
	`Write the person your final answer now, from what you already know: what is done, what is not, and what you would do next.]`;

/** Before a top-level call: count it, or refuse it once the limit is used (`beforeTool`). */
export function stepGate(conversation: string): string | undefined {
	const limit = stepLimit(conversation);
	const t = turnOf(conversation);
	if (!limit || t.steps < limit) {
		countStep(conversation);
		return undefined;
	}
	if (++t.refused >= STOP_AFTER) {
		const info = host().thread(conversation);
		if (info) host().stop(info.id);
	}
	return reached(limit);
}

/** Past the limit, the model is told so with every request until it answers (`beforeRequest`). */
export function stepNote(conversation: string): { role: "user"; content: string; timestamp: number } | undefined {
	const limit = stepLimit(conversation);
	if (!limit || turnOf(conversation).steps < limit) return undefined;
	return { role: "user", content: reached(limit), timestamp: Date.now() };
}

/** The limit stopped this turn's work: the evaluator does not send it back (`judge/done.ts`). */
export function limitReached(conversation: string): boolean {
	const limit = stepLimit(conversation);
	return !!limit && turnOf(conversation).steps >= limit;
}
