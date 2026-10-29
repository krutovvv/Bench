// The evaluator: a System One model (TypeSafe's Jev, or a server with the same API) that answers
// typed questions about a state with calibrated probabilities, in a fraction of a second. The
// agent asks it at three points of a turn (`search.ts`, `drift.ts`, `done.ts`); the thread's own
// model never sees it, only results it changed. Settings → Agents → Evaluator: on or off, the
// server's address, its model, a key when it asks for one (the `judge` setting).
//
// Off, unset or failing, every point does nothing and the turn goes the classic way. A failure
// keeps that address quiet for a minute, so a server that is down costs one wait, not one per step.
// API: POST <base>/v1/systemone {model, state, questions} → {answers} (docs.typesafe.ai/api).

import fs from "node:fs";
import path from "node:path";
import { BENCH_HOME, getSetting, readAgent } from "../config.ts";
import { host } from "../host.ts";

export interface JudgeSettings {
	enabled?: boolean;
	base_url?: string;
	model?: string;
	api_key?: string;
}

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

/** A local server starts its container and loads its model on the first request (90 s measured
 *  behind a GPU balancer that first stops speech to text): that one may take this long. */
const TIMEOUT_MS = 150_000;
/** After a failure the evaluator is left alone this long. */
const QUIET_MS = 60_000;
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

/** Each address's end of quiet after a failure. */
const quietUntil = new Map<string, number>();

/** The evaluator's settings when it is on and has an address. */
export function judgeSettings(): JudgeSettings | undefined {
	const s = getSetting("judge") as JudgeSettings | undefined;
	return s?.enabled && s.base_url?.trim() ? s : undefined;
}

/** The project (or Host) turned the evaluator on in its Settings: off unless it did. */
export function evaluatorOn(agent: string): boolean {
	return readAgent(agent).evaluator?.enabled === true;
}

/** The evaluator's settings for a conversation: the gear's connection on, and its project's switch. */
export function judgeFor(conversation: string): JudgeSettings | undefined {
	const s = judgeSettings();
	const agent = s ? host().thread(conversation)?.agent : undefined;
	return s && agent && evaluatorOn(agent) ? s : undefined;
}

/** On for this conversation and not quiet after a failure: the points may ask; otherwise the classic way. */
export function judgeActive(conversation: string): boolean {
	const s = judgeFor(conversation);
	return !!s && Date.now() >= (quietUntil.get(s.base_url!) ?? 0);
}

/** The endpoint for an address given with or without `/v1`. */
export function endpointOf(base: string): string {
	return `${base.trim().replace(/\/+$/, "").replace(/\/v1$/, "")}/v1/systemone`;
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
	const s = judgeFor(conversation);
	if (!s || Date.now() < (quietUntil.get(s.base_url!) ?? 0)) return undefined;
	const started = Date.now();
	const post = (cap: number) =>
		fetch(endpointOf(s.base_url!), {
			method: "POST",
			headers: { "Content-Type": "application/json", ...(s.api_key ? { Authorization: `Bearer ${s.api_key}` } : {}) },
			body: JSON.stringify({ model: s.model?.trim() || "jev-latest", state: fitState(state, cap), questions }),
			signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(TIMEOUT_MS)]) : AbortSignal.timeout(TIMEOUT_MS),
		});
	try {
		let r = await post(STATE_CAP);
		// Over the server's window still (a text dense in tokens): once more with half as much.
		if (r.status === 422) {
			const text = await r.text();
			if (!/context/i.test(text)) throw new Error(`HTTP 422: ${text.slice(0, 300)}`);
			r = await post(STATE_CAP / 2);
		}
		if (!r.ok) throw new Error(`HTTP ${r.status}: ${(await r.text()).slice(0, 300)}`);
		const answers = ((await r.json()) as any)?.answers ?? {};
		host().count(conversation, "call");
		log({ conversation, point, ms: Date.now() - started, answers: brief(answers) });
		return answers;
	} catch (e) {
		if (signal?.aborted) return undefined;
		quietUntil.set(s.base_url!, Date.now() + QUIET_MS);
		console.error(`[judge] ${point}: ${(e as Error).message}`);
		log({ conversation, point, ms: Date.now() - started, error: (e as Error).message });
		return undefined;
	}
}

/**
 * The gear's Check: one yes/no question to a server with these settings (the saved key when none
 * is given), what it answered and how fast. The first request may wait while the server loads.
 */
export async function checkJudge(s: JudgeSettings): Promise<{ ok: boolean; message: string }> {
	if (!s.base_url?.trim()) return { ok: false, message: "Enter the server's address" };
	const key = s.api_key || (getSetting("judge") as JudgeSettings | undefined)?.api_key;
	const started = Date.now();
	try {
		const r = await fetch(endpointOf(s.base_url), {
			method: "POST",
			headers: { "Content-Type": "application/json", ...(key ? { Authorization: `Bearer ${key}` } : {}) },
			body: JSON.stringify({
				model: s.model?.trim() || "jev-latest",
				state: "I was charged twice for my subscription this month.",
				questions: { billing: { type: "noul", instructions: "Is this message about billing?" } },
			}),
			signal: AbortSignal.timeout(TIMEOUT_MS),
		});
		const text = await r.text();
		if (!r.ok) return { ok: false, message: `HTTP ${r.status}: ${text.slice(0, 200)}` };
		const j: any = JSON.parse(text);
		const yes = j?.answers?.billing?.noul;
		if (typeof yes !== "number") return { ok: false, message: "It answered, but not as a System One server" };
		const ms = Date.now() - started;
		return { ok: true, message: `Works · ${j.model ?? s.model ?? "model"} · ${ms < 2000 ? `${ms} ms` : `${Math.round(ms / 1000)} s, loading first`}` };
	} catch (e) {
		const c: any = (e as any)?.cause;
		return { ok: false, message: String(c?.code ?? c?.message ?? (e as Error).message) };
	}
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
