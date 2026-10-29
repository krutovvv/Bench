// The evaluator before a call: when the agent is about to make a call it made just before, it is
// asked whether this one repeats one that failed, or one whose result it already has, with nothing
// it depends on changed since — the loop of a run that drives the same probe again and again. Sure
// of it, the call is refused with the reason, once: the same call again goes through, as the agent
// may know better.
//
// Which earlier call is "the same" is the code's to say, on the whole arguments: a line's start
// cannot tell two scripts apart (every `cd /tmp && python3 - <<'EOF' import json…` reads alike in
// 200 characters). What the evaluator judges is what the code cannot: whether anything changed
// since, and whether a near copy is the same probe or a fix of it.

import { cut, type Call } from "../turn/turn.ts";
import type { Point } from "./point.ts";

/** How sure the evaluator must be to stop the same call: a wrong stop costs a step, so it must be very. */
const SURE = 0.85;
/** A near copy (a long argument mostly alike) is often a fix of the last one: surer still. */
const SURE_NEAR = 0.95;
/** How far back an earlier call is looked for. */
const BACK = 3;
/** How alike two long arguments must be (their words, Dice) to be a near copy. */
const NEAR = 0.8;
/** An argument this long is text (a command, a script) and compared by its words; shorter, exactly. */
const LONG = 80;
/** How much of each call the evaluator reads. */
const SHOWN = 3000;

const SAY: Record<string, string> = {
	same_failed: "[Evaluator: this call repeats one that just failed, and nothing it depends on has changed since. It is not run: change the call, take another way, or tell the person what blocks you.]",
	same_result: "[Evaluator: this call repeats one whose result you already have, and nothing it depends on has changed since. It is not run: use that result, or change something first.]",
};

const keyOf = (call: { name: string; arguments?: any }) => `${call.name} ${JSON.stringify(call.arguments ?? {})}`;
const norm = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v ?? null)).replace(/\s+/g, " ").trim();
const words = (s: string) => s.toLowerCase().match(/[\p{L}\p{N}_]+/gu) ?? [];

/** How alike two texts are by their words (Dice over word multisets): 1 the same, 0 nothing shared. */
export function alike(a: string, b: string): number {
	const wa = words(a), wb = words(b);
	if (!wa.length && !wb.length) return 1;
	const count = new Map<string, number>();
	for (const w of wa) count.set(w, (count.get(w) ?? 0) + 1);
	let shared = 0;
	for (const w of wb) {
		const n = count.get(w) ?? 0;
		if (n > 0) (shared++, count.set(w, n - 1));
	}
	return (2 * shared) / (wa.length + wb.length);
}

/** How a call stands to an earlier one: the same, a near copy, or another call. */
export function likeness(next: any, earlier: any): "same" | "near" | undefined {
	const a = next && typeof next === "object" ? next : { "": next };
	const b = earlier && typeof earlier === "object" ? earlier : { "": earlier };
	const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
	let near = false;
	for (const k of keys) {
		const x = norm(a[k]), y = norm(b[k]);
		if (x === y) continue;
		// A short value (a path, an offset, a pattern) that differs is another call.
		if (x.length < LONG || y.length < LONG || alike(x, y) < NEAR) return undefined;
		near = true;
	}
	return near ? "near" : "same";
}

/** The earlier call of the last few the next one is likest to: the same before a near copy. */
function earlierOf(next: { name: string; arguments?: any }, calls: Call[]): { call: Call; kind: "same" | "near"; since: Call[] } | undefined {
	let found: { call: Call; kind: "same" | "near"; since: Call[] } | undefined;
	const last = calls.slice(-BACK);
	last.forEach((c, i) => {
		// A call refused before it ran is no earlier result: nothing to repeat.
		if (c.name !== next.name || c.notRun) return;
		const kind = likeness(next.arguments ?? {}, c.args);
		if (kind && !(found?.kind === "same" && kind === "near")) found = { call: c, kind, since: last.slice(i + 1) };
	});
	return found;
}

const shown = (c: { name: string; args?: any; arguments?: any }) => `${c.name} ${cut(JSON.stringify(c.args ?? c.arguments ?? {}), SHOWN)}`;

/** The kind of likeness it was asked about: the same call, or a near copy. */
export const repeat: Point<"same" | "near"> = {
	name: "repeat",
	moment: "call",
	ask({ call, turn }) {
		if (!call || call.name === "todo" || turn.stopped === keyOf(call)) return undefined;
		const e = earlierOf(call, turn.calls);
		if (!e) return undefined;
		return {
			context: e.kind,
			state: {
				next_call: shown(call),
				earlier_call: `${shown(e.call)}${e.call.failed ? " → FAILED: " : " → "}${e.call.out}`,
				calls_since: e.since.map((c) => `${c.name} ${cut(JSON.stringify(c.args ?? {}), 300)}${c.notRun ? " → NOT RUN" : c.failed ? " → FAILED" : ""}`),
			},
			questions: {
				repeat: {
					type: "choice",
					instructions: "The agent is about to make the next call (next_call). The earlier call (earlier_call) is the likest of its last ones; calls_since are what it did between. Is next_call a repeat of earlier_call?",
					// "new" first: the model leans to the first option, and letting the call run is the safe side.
					criteria: {
						new: "A new step: it differs in what it does (other arguments, another part, a fixed error), or something it depends on changed since (a file written or edited, a page moved on, a process started).",
						same_failed: "The same as earlier_call, which failed, with nothing changed since: it will fail the same way.",
						same_result: "The same as earlier_call, whose result the agent already has, with nothing changed since: it will give the same.",
					},
				},
			},
		};
	},
	apply({ call, turn }, answers, kind) {
		// Both kinds are a repeat: their sum is how sure it is one (0.79 + 0.17 for the same probe
		// run again on the local server), the likelier of the two says which.
		const p = answers.repeat?.probabilities ?? {};
		const failed = Number(p.same_failed ?? 0);
		const known = Number(p.same_result ?? 0);
		const c = failed >= known ? "same_failed" : "same_result";
		if (!call || failed + known < (kind === "near" ? SURE_NEAR : SURE)) return undefined;
		turn.stopped = keyOf(call);
		return { block: SAY[c] };
	},
};
