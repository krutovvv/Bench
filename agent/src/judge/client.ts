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
import { BENCH_HOME, getSetting } from "../config.ts";
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

export interface ChoiceAnswer {
	choice: string;
	confidence: number;
	probabilities: Record<string, number>;
}

/** A local server loads its model on the first request: that one may take this long. */
const TIMEOUT_MS = 60_000;
/** After a failure the evaluator is left alone this long. */
const QUIET_MS = 60_000;
/** The most of a state sent, in characters (a local server's window is ~16K tokens). */
export const STATE_CAP = 24_000;

/** Each address's end of quiet after a failure. */
const quietUntil = new Map<string, number>();

/** The evaluator's settings when it is on and has an address. */
export function judgeSettings(): JudgeSettings | undefined {
	const s = getSetting("judge") as JudgeSettings | undefined;
	return s?.enabled && s.base_url?.trim() ? s : undefined;
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
	questions: Record<string, Choice>,
	signal?: AbortSignal,
): Promise<Record<string, ChoiceAnswer> | undefined> {
	const s = judgeSettings();
	if (!s || Date.now() < (quietUntil.get(s.base_url!) ?? 0)) return undefined;
	const started = Date.now();
	try {
		const r = await fetch(endpointOf(s.base_url!), {
			method: "POST",
			headers: { "Content-Type": "application/json", ...(s.api_key ? { Authorization: `Bearer ${s.api_key}` } : {}) },
			body: JSON.stringify({ model: s.model?.trim() || "jev-latest", state, questions }),
			signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(TIMEOUT_MS)]) : AbortSignal.timeout(TIMEOUT_MS),
		});
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

/** The options of an answer from the likeliest. */
export function ranked(a: ChoiceAnswer | undefined): [string, number][] {
	return Object.entries(a?.probabilities ?? {}).sort((x, y) => y[1] - x[1]);
}

/** Every answer in the log in a line: the choice and its probability (a ranking's top three). */
function brief(answers: Record<string, any>): Record<string, string> {
	return Object.fromEntries(
		Object.entries(answers).map(([k, a]) => [k, ranked(a).slice(0, 3).map(([o, p]) => `${o}:${p.toFixed(2)}`).join(" ")]),
	);
}

/** One line per request: what to measure the evaluator by later. */
function log(entry: Record<string, unknown>): void {
	try {
		fs.appendFileSync(path.join(BENCH_HOME, "judge.log"), JSON.stringify({ at: new Date().toISOString(), ...entry }) + "\n");
	} catch {}
}
