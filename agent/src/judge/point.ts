// What every place the evaluator is asked at is: a `Point`. It says at which moment of a turn it
// may ask, decides whether to ask now and what (typed questions, and what it adds to the state),
// and turns the answers into an effect — a result reordered, a note to the model, a turn sent
// back, an entry for the thread. The points of one moment are asked together, one request with
// one state (`index.ts`); a new idea is a new file with a `Point` and a line in `POINTS`.

import type { PlanStep } from "../protocol.gen.ts";
import { cut, planLines, type Turn } from "../builtin/turn.ts";
import type { ChoiceAnswer, Question } from "./client.ts";

/**
 * When a point may ask: `request` before a request to the model (`beforeRequest`), `call` before a
 * top-level call runs (`beforeTool`), `result` after
 * every top-level tool call (before the model reads the result), `answer` when the model gives its
 * final answer. A new moment (before a call, at compaction) gets a dispatcher in `index.ts` and a
 * hook in `extensions/assistant.ts`.
 */
export type Moment = "request" | "call" | "result" | "answer";

/** What a point sees at its moment. */
export interface Seen {
	conversation: string;
	turn: Turn;
	/** At `call` and `result`: the call; at `result`, what it returned (as the points before this one left it). */
	call?: { id?: string; name: string; arguments?: any };
	result?: any;
	/** At `answer`: the model's final message. */
	answer?: any;
	/** At `request`: what the thread is (its agent, folder, project), for what it can use. */
	thread?: import("../host.ts").ThreadFacts;
	/** At `call`: approvals are on and the patterns let this command through (`judge/risk.ts`). */
	approvals?: boolean;
}

export interface Asked {
	/** Its questions, by names of its own (the dispatcher keeps them apart from other points'). */
	questions: Record<string, Question>;
	/** What it adds to the turn's common state (`baseState`). */
	state?: Record<string, unknown>;
}

export interface Effect {
	/** The result the model reads instead (`result` only). */
	result?: any;
	/** A line for the model with the result (`result` only). */
	note?: string;
	/** Send the model back to work with this message (`answer` only; the first wins). */
	continue?: string;
	/** A display-only entry for the thread (the model never reads it). */
	write?: { kind: string; data: unknown };
	/** The plan as the evaluator now reads it: the turn keeps it, the thread shows it. */
	plan?: PlanStep[];
	/** A line for the model with this request only (`request` only). */
	hint?: string;
	/** No reasoning for this request: the step is plain (`request` only, `runtime/steer.ts`). */
	plain?: boolean;
	/** Ask the person before the call runs, saying why (`call` only). */
	approve?: string;
	/** Refuse the call, saying why to the model (`call` only). */
	block?: string;
}

export interface Point {
	name: string;
	moment: Moment;
	/** The questions to ask now, or undefined: nothing to ask at this moment. */
	ask(seen: Seen): Asked | undefined | Promise<Asked | undefined>;
	/** What the answers change; undefined: nothing (unsure, or the answer agrees with things as they are). */
	apply(seen: Seen, answers: Record<string, ChoiceAnswer>): Effect | undefined;
}

/** What every request carries about the turn: the person's request, the plan, the last steps. */
export function baseState(turn: Turn, steps = 20): Record<string, unknown> {
	return {
		request: cut(turn.request, 4000),
		...(turn.plan.length ? { plan: planLines(turn.plan) } : {}),
		steps_so_far: turn.steps,
		last_steps: turn.recent.slice(-steps),
	};
}

/** The answer's choice when it is at least this sure of it; undefined otherwise. */
export function sure(a: ChoiceAnswer | undefined, at: number): string | undefined {
	const p = a?.probabilities?.[a.choice] ?? 0;
	return a && p >= at ? a.choice : undefined;
}
