// What every place the evaluator is asked at is: a `Point`. It says at which moment of a turn it
// may ask, decides whether to ask now and what (typed questions, and what it adds to the state),
// and turns the answers into an effect — a result reordered, a note to the model, a turn sent
// back, an entry for the thread. The points of one moment are asked together, one request with
// one state (`index.ts`); a new idea is a new file with a `Point` and a line in `POINTS`.

import type { PlanStep } from "../protocol.gen.ts";
import { cut, planLines, type Turn } from "../builtin/turn.ts";
import type { ChoiceAnswer, Question } from "./client.ts";

/**
 * When a point may ask: `result` after every top-level tool call (before the model reads the
 * result), `answer` when the model gives its final answer. New moments (before a call, before a
 * request to the model) get a dispatcher in `index.ts` and a hook in `extensions/assistant.ts`.
 */
export type Moment = "result" | "answer";

/** What a point sees at its moment. */
export interface Seen {
	conversation: string;
	turn: Turn;
	/** At `result`: the call and what it returned (as the points before this one left it). */
	call?: { id?: string; name: string; arguments?: any };
	result?: any;
	/** At `answer`: the model's final message. */
	answer?: any;
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
}

export interface Point {
	name: string;
	moment: Moment;
	/** The questions to ask now, or undefined: nothing to ask at this moment. */
	ask(seen: Seen): Asked | undefined;
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
