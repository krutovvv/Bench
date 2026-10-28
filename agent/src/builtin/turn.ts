// What a conversation's current turn is about, kept by the default extension's hooks: the
// person's request (the last message they wrote), the plan the agent keeps (`todo`), the turn's
// steps (top-level tool calls, a script's nested calls not counted) and the last few of them in a
// line each. The step limit (`steps.ts`) and the evaluator (`judge/`) read it; nothing else does.
// In memory only: after a restart a turn starts counting again, which errs on the side of working.

import { EVAL_OPEN, planOf, stripBlock, textOf } from "../runtime/journal.ts";
import type { PlanStep } from "../protocol.gen.ts";

/** How many steps a turn remembers, a line each. */
const RECENT = 24;

export interface Turn {
	/** The person's request: their last message's text. */
	request: string;
	/** The plan as the last `todo` call wrote it, with the evaluator's marks since (`judge/plan.ts`). */
	plan: PlanStep[];
	/** The id of the `todo` call that wrote the plan: the plan's id in the thread. */
	planId: string;
	/** The steps at which the evaluator last read the plan. */
	planAt: number;
	/** Top-level tool calls in this turn. */
	steps: number;
	/** The last steps, a line each: the call and the start of its result. */
	recent: string[];
	/** Calls refused because the step limit was reached. */
	refused: number;
	/** The evaluator sent the model back to work once in this turn. */
	continued: boolean;
	/** The steps at which the evaluator last checked the course. */
	checkedAt: number;
}

const turns = new Map<string, Turn>();

const fresh = (request: string, plan: PlanStep[] = [], planId = ""): Turn => ({ request, plan, planId, planAt: 0, steps: 0, recent: [], refused: 0, continued: false, checkedAt: 0 });

/** The conversation's turn; one with nothing known yet when it was never seen. */
export function turnOf(conversation: string): Turn {
	let t = turns.get(conversation);
	if (!t) turns.set(conversation, (t = fresh("")));
	return t;
}

/** Every request to the model: a person's message at its end begins a new turn (the plan carries
 *  over: it is the thread's, not the turn's). The evaluator's own message does not. */
export function seeRequest(conversation: string, messages: readonly any[]): void {
	const last = messages[messages.length - 1];
	if (last?.role !== "user") return;
	const text = stripBlock(textOf(last.content)).trim();
	if (!text || text.startsWith(EVAL_OPEN)) return;
	const t = turns.get(conversation);
	if (t?.request === text && t.steps === 0) return;
	turns.set(conversation, fresh(text, t?.plan, t?.planId));
}

/** A top-level call is about to run. */
export function countStep(conversation: string): number {
	return ++turnOf(conversation).steps;
}

/** A call finished: remember it in a line, and the plan when it was `todo`. */
export function seeResult(conversation: string, call: { id?: string; name: string; arguments?: any }, result: any): void {
	const t = turnOf(conversation);
	if (call.name === "todo" && !result?.isError) {
		t.plan = planOf(call.arguments);
		t.planId = String(call.id ?? "");
		t.planAt = t.steps;
	}
	const args = JSON.stringify(call.arguments ?? {});
	const out = resultText(result).replace(/\s+/g, " ").trim();
	t.recent.push(`${call.name} ${cut(args, 200)}${result?.isError ? " → ERROR " : " → "}${cut(out, 160)}`);
	if (t.recent.length > RECENT) t.recent.splice(0, t.recent.length - RECENT);
}

/** The text a tool returned. */
export function resultText(result: any): string {
	return (result?.output ?? [])
		.filter((b: any) => b?.type === "text")
		.map((b: any) => String(b.text ?? ""))
		.join("\n");
}

export const cut = (s: string, n: number): string => (s.length > n ? `${s.slice(0, n)}…` : s);

/** The plan in a line per item, as the evaluator reads it. */
export const planLines = (plan: PlanStep[]): string[] => plan.map((p) => `[${p.status}] ${p.content}`);
