// The evaluator: a System One model (TypeSafe's Jev, or a server with the same API) that answers
// typed questions about a state with calibrated probabilities, in a fraction of a second. The
// agent asks it at points of a turn (`index.ts`); the thread's own model never sees it, only
// results it changed. It is Pi's classifier model `bench/evaluator` (`models.ts`): the gear's main
// server, or its fallback — the same model code mode scripts ask with `models.classify()`.
//
// No server in the gear, the project's switch off, or both servers failing: every point does
// nothing and the turn goes the classic way. A failure keeps that server quiet for a minute, so a
// server that is down costs one wait, not one per step.

import fs from "node:fs";
import path from "node:path";
import type { ClassifierQuestion } from "@earendil-works/pi-ai";
import { BENCH_HOME, readAgent } from "../config.ts";
import { host } from "../host.ts";
import { answering, classifyOn, classifyPair, evaluatorReady, evaluatorSettings, type EvaluatorServer } from "./models.ts";

export interface Choice {
	type: "choice";
	instructions: string;
	/** Option name → what it means. */
	criteria: Record<string, string>;
}

/** A yes/no question: the answer is the probability of yes (`noul`). */
export interface Noul {
	type: "noul";
	instructions: string;
}

export type Question = Choice | Noul;

export interface ChoiceAnswer {
	choice: string;
	confidence: number;
	probabilities: Record<string, number>;
}

/** The most of a state sent, in characters (a local server's window is ~16K tokens; Russian and
 *  code take more tokens a character than English: 24 000 overflowed it). */
export const STATE_CAP = 20_000;

/**
 * The state within `cap` characters: its largest member shrinks until it fits — a list loses its
 * oldest items first (the latest results matter most), a text its end. What a point put in stays;
 * only how much of it.
 */
export function fitState(state: unknown, cap = STATE_CAP): unknown {
	if (!state || typeof state !== "object" || JSON.stringify(state).length <= cap) return state;
	const s: Record<string, any> = { ...(state as Record<string, any>) };
	for (let guard = 0; guard < 200 && JSON.stringify(s).length > cap; guard++) {
		const [k] = Object.entries(s).map(([k, v]) => [k, JSON.stringify(v ?? "").length] as const).sort((a, b) => b[1] - a[1])[0];
		const v = s[k];
		if (Array.isArray(v) && v.length > 1) s[k] = v.slice(Math.ceil(v.length / 4));
		else if (Array.isArray(v)) s[k] = v.map((x) => (typeof x === "string" ? x.slice(0, Math.floor(x.length / 2)) : x));
		else if (typeof v === "string") s[k] = v.slice(0, Math.floor(v.length / 2));
		// Named parts (a listing's items, a result's parts) keep their names: each text shrinks.
		else if (v && typeof v === "object") s[k] = Object.fromEntries(Object.entries(v).map(([n, x]) => [n, typeof x === "string" ? x.slice(0, Math.floor(x.length / 2)) : x]));
		else break;
	}
	return s;
}

/** The one place that says whether a project's (or Host's) threads use the evaluator: the gear has
 *  a server, and the project's switch is on (its Settings → Model → Evaluator). The prompt's line on
 *  it, the thread's meters and code mode's `models.classify()` ask it; the points ask `judgeActive`. */
export function evaluatorFor(agent: string): boolean {
	return evaluatorReady() && readAgent(agent).evaluator?.enabled === true;
}

/** The conversation's project uses the evaluator, and some server is not quiet after a failure: the
 *  points, the excerpt and the window's cut may ask now. */
export function judgeActive(conversation: string): boolean {
	const agent = host().thread(conversation)?.agent;
	return !!agent && evaluatorFor(agent) && answering();
}

/** Our questions as pi-ai's: a yes/no is its `bool` (sent as `noul` again). */
function asPi(questions: Record<string, Question>): Record<string, ClassifierQuestion> {
	return Object.fromEntries(Object.entries(questions).map(([id, q]) => [id, (q.type === "noul" ? { type: "bool", instructions: q.instructions, criteria: { true: "yes", false: "no" } } : q) as ClassifierQuestion]));
}

/** pi-ai's answers in the server's own shape, as the points read them: a yes/no as `noul`. */
function asWire(answers: Record<string, any>): Record<string, any> {
	return Object.fromEntries(Object.entries(answers).map(([id, a]) => [id, a?.type === "bool" ? { noul: a.probability } : a]));
}

/**
 * Ask the evaluator one or more choices about `state`; undefined when it is off, quiet after a
 * failure, or fails now. `point` names the asking place in the log (`~/.bench/judge.log`).
 */
export async function ask(
	conversation: string,
	point: string,
	state: unknown,
	questions: Record<string, Question>,
	signal?: AbortSignal,
): Promise<Record<string, ChoiceAnswer> | undefined> {
	if (!judgeActive(conversation)) return undefined;
	const started = Date.now();
	const once = (cap: number) => classifyPair({ state: fitState(state, cap) as any, questions: asPi(questions) }, { signal });
	let r = await once(STATE_CAP);
	// Over the server's window still (a text dense in tokens): once more with half as much.
	if (r.stopReason === "error" && /422/.test(r.errorMessage ?? "") && /context/i.test(r.errorMessage ?? "")) r = await once(STATE_CAP / 2);
	if (r.stopReason !== "stop") {
		if (r.stopReason === "error") log({ conversation, point, ms: Date.now() - started, error: r.errorMessage });
		return undefined;
	}
	const answers = asWire(r.answers);
	host().count(conversation, "call");
	log({ conversation, point, server: r.server, ms: Date.now() - started, answers: brief(answers) });
	return answers;
}

/**
 * The gear's Check: one yes/no question to a server with these settings (the saved key when none
 * is given), what it answered and how fast. The first request may wait while the server loads.
 */
export async function checkJudge(s: Partial<EvaluatorServer>): Promise<{ ok: boolean; message: string }> {
	if (!s.base_url?.trim()) return { ok: false, message: "Enter the server's address" };
	const key = s.api_key || evaluatorSettings().servers.find((x) => x.id === s.id)?.api_key;
	const started = Date.now();
	const r = await classifyOn({ id: s.id || "check", base_url: s.base_url, model: s.model, api_key: key }, {
		state: { message: "I was charged twice for my subscription this month." },
		questions: { billing: { type: "bool", instructions: "Is this message about billing?", criteria: { true: "it is about billing", false: "it is not" } } },
	});
	if (r.stopReason !== "stop") {
		const m = r.errorMessage ?? "error";
		return { ok: false, message: /did not return|unexpected response/.test(m) ? "It answered, but not as a System One server" : m.slice(0, 200) };
	}
	const ms = Date.now() - started;
	return { ok: true, message: `Works · ${r.model} · ${ms < 2000 ? `${ms} ms` : `${Math.round(ms / 1000)} s, loading first`}` };
}

/** The options of an answer from the likeliest. */
export function ranked(a: ChoiceAnswer | undefined): [string, number][] {
	return Object.entries(a?.probabilities ?? {}).sort((x, y) => y[1] - x[1]);
}

/** Every answer in the log in a line: the choice and its probability (a ranking's top three). */
function brief(answers: Record<string, any>): Record<string, string> {
	return Object.fromEntries(
		Object.entries(answers).map(([k, a]) => [k, typeof a?.noul === "number" ? `yes:${a.noul.toFixed(2)}` : ranked(a).slice(0, 3).map(([o, p]) => `${o}:${p.toFixed(2)}`).join(" ")]),
	);
}

/** One line per request: what to measure the evaluator by later. */
function log(entry: Record<string, unknown>): void {
	try {
		fs.appendFileSync(path.join(BENCH_HOME, "judge.log"), JSON.stringify({ at: new Date().toISOString(), ...entry }) + "\n");
	} catch {}
}
