// The evaluator keeps the plan's marks: every few steps each open item of the plan the agent wrote
// (`todo`) is asked about — not started, in progress, finished — against the last steps. A sure
// answer moves an item forward, never back; the thread shows the plan so marked (an entry of its
// own, `bench.plan`, read as a later snapshot of the same plan) and the model is told in a line,
// so it need not spend a step on `todo` to tick items. Its own `todo` call always wins: it
// rewrites the plan, and the marks start again from it. With the evaluator off, the plan's
// reminder does this job (`builtin/ask.ts` `planNudge`).

import type { PlanStep } from "../protocol.gen.ts";
import { cut } from "../builtin/turn.ts";
import { PLAN_KIND } from "../runtime/journal.ts";
import { sure, type Point } from "./point.ts";

/** Steps between two readings of the plan. */
export const PLAN_EVERY = 5;
/** The most items asked about at once. */
const MAX_ITEMS = 12;
/** How sure the evaluator must be to mark an item finished, or begun. */
const SURE_DONE = 0.85;
const SURE_BEGUN = 0.7;

const ORDER = ["pending", "in_progress", "completed"];
const MEANS: Record<string, string> = {
	pending: "Nothing in the steps so far works on this item yet.",
	in_progress: "The steps work on this item, and it is not finished yet.",
	completed: "The steps show this item finished.",
};

const isOpen = (p: PlanStep) => p.status === "pending" || p.status === "in_progress";

export const plan: Point = {
	name: "plan",
	moment: "result",
	ask({ turn }) {
		if (!turn.planId || turn.steps - turn.planAt < PLAN_EVERY) return undefined;
		const open = turn.plan.map((p, i) => [p, i] as const).filter(([p]) => isOpen(p)).slice(0, MAX_ITEMS);
		if (!open.length) return undefined;
		turn.planAt = turn.steps;
		return {
			// What was found and done at more length than a line a step, and the agent's own words:
			// a line each was too little to see an item finished.
			state: { agent_said: turn.said.slice(-6), recent_results: turn.evidence },
			questions: Object.fromEntries(
				open.map(([p, i]) => [
					`item${i}`,
					{
						type: "choice" as const,
						instructions: `An agent keeps this item in its plan for the person's request: "${cut(p.content, 300)}". Judging by its steps, where is this item now?`,
						// Its status as it stands first: the model leans to the first option, and leaving it is the safe side.
						criteria: Object.fromEntries([p.status, ...ORDER.filter((s) => s !== p.status)].map((s) => [s, MEANS[s]])),
					},
				]),
			),
		};
	},
	apply({ turn }, answers) {
		const next = turn.plan.map((p) => ({ ...p }));
		const finished: string[] = [];
		const begun: string[] = [];
		next.forEach((p, i) => {
			const a = answers[`item${i}`];
			if (!a || !isOpen(p)) return;
			if (sure(a, SURE_DONE) === "completed") {
				p.status = "completed";
				finished.push(p.content);
			} else if (p.status === "pending" && sure(a, SURE_BEGUN) === "in_progress") {
				p.status = "in_progress";
				begun.push(p.content);
			}
		});
		if (!finished.length && !begun.length) return undefined;
		// Something finished and nothing is under way: the next item is, as the plan reads to the person.
		if (!next.some((p) => p.status === "in_progress")) {
			const first = next.find((p) => p.status === "pending");
			if (first) {
				first.status = "in_progress";
				begun.push(first.content);
			}
		}
		const said = [finished.length ? `done: ${finished.map((c) => `"${cut(c, 80)}"`).join(", ")}` : "", begun.length ? `now: ${begun.map((c) => `"${cut(c, 80)}"`).join(", ")}` : ""]
			.filter(Boolean)
			.join("; ");
		return {
			plan: next,
			write: { kind: PLAN_KIND, data: { id: turn.planId, todos: next } },
			note: `[Evaluator: your plan is marked — ${said}. Call todo only to change the plan itself.]`,
		};
	},
};
