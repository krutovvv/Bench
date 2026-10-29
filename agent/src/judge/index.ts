// The evaluator's points and their moments: the default extension's hooks call `atResult` and
// `atAnswer`, and every point of that moment that has something to ask now is asked in one
// request — one state (the turn's, plus what each point adds), every question evaluated against
// it in parallel. A point's answers come back under its own names; its effect is applied in the
// order of `POINTS` (a reordered result is what the next point sees).
//
// Off, quiet after a failure, or with nothing to ask: nothing happens and the turn goes the
// classic way (`judgeActive`).

import { host } from "../host.ts";
import { turnOf } from "../builtin/turn.ts";
import { ask, judgeActive, type ChoiceAnswer, type Question } from "./client.ts";
import { baseState, type Asked, type Effect, type Moment, type Point, type Seen } from "./point.ts";
import { search } from "./search.ts";
import { course } from "./drift.ts";
import { plan } from "./plan.ts";
import { done } from "./done.ts";
import { start } from "./start.ts";
import { failure } from "./failure.ts";
import { risk } from "./risk.ts";
import { repeat } from "./repeat.ts";
import { listing } from "./listing.ts";
import { claims } from "./claims.ts";

/** Every point, in the order their effects apply. A new idea: a file with a `Point`, a line here. */
export const POINTS: Point[] = [start, repeat, risk, search, listing, failure, plan, course, claims, done];

/** Ask the points of `moment` that have a question now; their effects, in `POINTS` order. */
async function at(moment: Moment, seen: Seen, signal?: AbortSignal): Promise<Effect[]> {
	if (!judgeActive(seen.conversation)) return [];
	const asking: [Point, Asked][] = [];
	for (const p of POINTS.filter((p) => p.moment === moment)) {
		const a = await p.ask(seen);
		if (a) asking.push([p, a]);
	}
	if (!asking.length) return [];
	const state = Object.assign(baseState(seen.turn), ...asking.map(([, a]) => a.state ?? {}));
	const questions: Record<string, Question> = {};
	for (const [p, a] of asking) for (const [k, q] of Object.entries(a.questions)) questions[`${p.name}.${k}`] = q;
	const answers = await ask(seen.conversation, asking.map(([p]) => p.name).join("+"), state, questions, signal);
	if (!answers) return [];
	const effects: Effect[] = [];
	for (const [p] of asking) {
		const own: Record<string, ChoiceAnswer> = {};
		for (const [k, v] of Object.entries(answers)) if (k.startsWith(`${p.name}.`)) own[k.slice(p.name.length + 1)] = v;
		const e = p.apply(seen, own);
		if (!e) continue;
		if (e.result) seen.result = e.result;
		if (e.plan) seen.turn.plan = e.plan;
		if (e.write) host().write(seen.conversation, e.write);
		host().count(seen.conversation, "nudge");
		effects.push(e);
	}
	return effects;
}

/** After a top-level call (`afterTool`): the result the model reads, and lines for it. */
export async function atResult(conversation: string, call: Seen["call"], result: any, signal?: AbortSignal): Promise<{ result?: any; notes: string[] }> {
	const seen: Seen = { conversation, turn: turnOf(conversation), call, result };
	const effects = await at("result", seen, signal);
	const changed = effects.some((e) => e.result);
	return { ...(changed ? { result: seen.result } : {}), notes: effects.map((e) => e.note).filter((n): n is string => !!n) };
}

/** Before a request to the model (`beforeRequest`): lines for it with this request only. */
export async function atRequest(conversation: string, signal?: AbortSignal): Promise<string[]> {
	const effects = await at("request", { conversation, turn: turnOf(conversation), thread: host().thread(conversation) }, signal);
	return effects.map((e) => e.hint).filter((h): h is string => !!h);
}

/** Before a top-level call runs (`beforeTool`): refuse it (a repeat), or ask the person first (a risk, approvals on). */
export async function atCall(conversation: string, call: Seen["call"], approvals: boolean, signal?: AbortSignal): Promise<{ block?: string; approve?: string }> {
	const effects = await at("call", { conversation, turn: turnOf(conversation), call, approvals }, signal);
	return { block: effects.find((e) => e.block)?.block, approve: effects.find((e) => e.approve)?.approve };
}

/** At the model's final answer (`onYield`): the message that sends it back, if any. */
export async function atAnswer(conversation: string, answer: any, signal?: AbortSignal): Promise<string | undefined> {
	const effects = await at("answer", { conversation, turn: turnOf(conversation), answer }, signal);
	return effects.find((e) => e.continue)?.continue;
}
