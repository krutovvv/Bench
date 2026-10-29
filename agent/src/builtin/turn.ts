// What a conversation's current turn is about, kept by the default extension's hooks: the
// person's request (the last message they wrote), the plan the agent keeps (`todo`), the turn's
// steps (top-level tool calls, a script's nested calls not counted) and the last few of them in a
// line each. The step limit (`steps.ts`) and the evaluator (`judge/`) read it; nothing else does.
// In memory only: after a restart a turn starts counting again, which errs on the side of working.

import { EVAL_OPEN, planOf, stripBlock, textOf } from "../runtime/journal.ts";
import type { PlanStep } from "../protocol.gen.ts";

/** How many steps a turn remembers, a line each. */
const RECENT = 24;
/** How many results, and of the agent's own words, a turn keeps at more length (`EVIDENCE`). */
const KEPT = 8;
const EVIDENCE = 600;
/** What a turn found, for its answer's facts: this many results, this much of each (the evaluator's
 *  server keeps the parts nearest each question when it is more than it reads). */
const FOUND_KEPT = 40;
const FOUND = 2000;

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
	/** The last results at more length than `recent` (what was found, what was done). */
	evidence: string[];
	/** The agent's own last words between its steps (what it says it did and found). */
	said: string[];
	/** The evaluator looked at the turn's start (`judge/start.ts`). */
	started: boolean;
	/** Every result of the turn at more length still, for checking the answer's facts (`judge/claims.ts`). */
	found: string[];
	/** MCP tools to offer from the next step: the servers the evaluator named at the start. */
	load: string[];
	/** The last call the evaluator stopped as a repeat (`judge/repeat.ts`): the same again goes through. */
	stopped: string;
	/** The last steps whole: each call's arguments in full, whether it failed or was never run. */
	calls: Call[];
	/** The ids of calls refused before they ran (a repeat, the step limit, the person): not failures. */
	notRun: Set<string>;
	/** The thread's first message: nothing said before it that the request could lean on. */
	opening: boolean;
	/** The course the evaluator leaned to, unsure, at its last check (`judge/drift.ts`): "" on track. */
	leaning: string;
}

/** A step as `judge/repeat.ts` compares it: its arguments whole, not a line's start. */
export interface Call {
	name: string;
	args: any;
	failed: boolean;
	notRun: boolean;
	out: string;
}

const turns = new Map<string, Turn>();

const fresh = (request: string, plan: PlanStep[] = [], planId = ""): Turn => ({ request, plan, planId, planAt: 0, steps: 0, recent: [], refused: 0, continued: false, checkedAt: 0, evidence: [], said: [], started: false, found: [], load: [], stopped: "", calls: [], notRun: new Set(), opening: false, leaning: "" });

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
	const next = fresh(text, t?.plan, t?.planId);
	next.opening = messages.filter((m) => m?.role === "user").length === 1;
	turns.set(conversation, next);
}

/** A top-level call is about to run. */
export function countStep(conversation: string): number {
	return ++turnOf(conversation).steps;
}

/** A call was refused before it ran: its result is the reason, not a failure of the call. */
export function markNotRun(conversation: string, id: unknown): void {
	if (id != null) turnOf(conversation).notRun.add(String(id));
}

/** A call finished: remember it in a line, and the plan when it was `todo`. */
export function seeResult(conversation: string, call: { id?: string; name: string; arguments?: any }, result: any): void {
	const t = turnOf(conversation);
	const notRun = call.id != null && t.notRun.delete(String(call.id));
	if (call.name === "todo" && !result?.isError) {
		t.plan = planOf(call.arguments);
		t.planId = String(call.id ?? "");
		t.planAt = t.steps;
	}
	const args = JSON.stringify(call.arguments ?? {});
	const out = resultText(result).replace(/\s+/g, " ").trim();
	t.calls.push({ name: call.name, args: call.arguments ?? {}, failed: !notRun && !!result?.isError, notRun, out: cut(out, EVIDENCE) });
	if (t.calls.length > RECENT) t.calls.splice(0, t.calls.length - RECENT);
	// A refused call never ran: said as such, so that the next one is not read as repeating a failure.
	if (notRun) {
		t.recent.push(`${call.name} ${cut(args, 200)} → NOT RUN (refused before it ran) ${cut(out, 160)}`);
		if (t.recent.length > RECENT) t.recent.splice(0, t.recent.length - RECENT);
		return;
	}
	t.recent.push(`${call.name} ${cut(args, 200)}${result?.isError ? " → ERROR " : " → "}${cut(out, 160)}`);
	if (t.recent.length > RECENT) t.recent.splice(0, t.recent.length - RECENT);
	t.evidence.push(`${call.name} ${cut(args, 120)} → ${cut(out, EVIDENCE)}`);
	if (t.evidence.length > KEPT) t.evidence.splice(0, t.evidence.length - KEPT);
	if (!result?.isError) {
		t.found.push(`${call.name} ${cut(args, 160)} → ${cut(out, FOUND)}`);
		if (t.found.length > FOUND_KEPT) t.found.splice(0, t.found.length - FOUND_KEPT);
	}
}

/** A script's nested call finished (codemode): kept whole for `judge/repeat.ts`, not a step of the turn. */
export function seeNestedResult(conversation: string, call: { id?: string; name: string; arguments?: any }, result: any): void {
	const t = turnOf(conversation);
	const notRun = call.id != null && t.notRun.delete(String(call.id));
	t.calls.push({ name: call.name, args: call.arguments ?? {}, failed: !notRun && !!result?.isError, notRun, out: cut(resultText(result).replace(/\s+/g, " ").trim(), EVIDENCE) });
	if (t.calls.length > RECENT) t.calls.splice(0, t.calls.length - RECENT);
}

/** The model's message (`afterResponse`): its words between steps, kept for the evaluator. */
export function seeAnswer(conversation: string, message: any): void {
	const text = (message?.content ?? [])
		.filter((b: any) => b?.type === "text")
		.map((b: any) => String(b.text ?? ""))
		.join(" ")
		.replace(/\s+/g, " ")
		.trim();
	if (!text) return;
	const t = turnOf(conversation);
	t.said.push(cut(text, EVIDENCE));
	if (t.said.length > KEPT) t.said.splice(0, t.said.length - KEPT);
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
