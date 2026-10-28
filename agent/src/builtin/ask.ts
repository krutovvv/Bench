// `clarify` (a question with choices, Bench's card / Telegram's buttons) and `todo` (the plan Bench
// shows over the composer).

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { host, threadOf } from "../host.ts";

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const clarify = defineTool({
	name: "clarify",
	description:
		"Ask the person a question when a decision is genuinely theirs. Give up to 4 choices, the one you recommend first; " +
		"they can always type their own answer. Don't ask what you can find out or decide yourself.",
	parameters: Type.Object({
		question: Type.String(),
		choices: Type.Optional(Type.Array(Type.String(), { maxItems: 4 })),
		multi_select: Type.Optional(Type.Boolean()),
	}),
	// Asking again after a restart is harmless: the question is still the person's to answer.
	replay: "safe",
	// The call is the question and its result the answer (docs/thread-objects.md): the card in
	// Bench reads both from the journal. The id the person answers to is the call's.
	execute: async (args, api, context) => {
		const s = threadOf(api.conversationId);
		const choices = (args.choices ?? []).map((c) => c.trim()).filter(Boolean).slice(0, 4);
		const answer = await host().ask(s.id, api.callId, args.question, choices, Boolean(args.multi_select) && choices.length > 0, context?.abortSignal);
		return {
			...text(answer ? { answer } : { answer: null, note: "No answer (declined or timed out). Decide yourself or stop." }),
			details: { answer: answer || null },
		};
	},
});

export const todo = defineTool({
	name: "todo",
	description:
		"Keep a plan for work of 3+ steps, shown to the person: pass the whole list each time. Statuses: pending, in_progress (one at a time), completed, cancelled. Keep it current with every call: a step goes in_progress before you start it and completed the moment it is done, cancelled once you drop it — a plan left as it was reads to the person as work still owed, and is worse than no plan at all.",
	parameters: Type.Object({
		todos: Type.Array(
			Type.Object({
				id: Type.String(),
				content: Type.String(),
				status: Type.Union([Type.Literal("pending"), Type.Literal("in_progress"), Type.Literal("completed"), Type.Literal("cancelled")]),
				parent: Type.Optional(Type.String()),
			}),
		),
	}),
	// The plan is this call itself: its arguments in the journal are what Bench shows, where the
	// first call of the work stands (docs/thread-objects.md). Nothing is kept beside it.
	replay: "safe",
	execute: async (args) => {
		const done = args.todos.filter((t) => t.status === "completed").length;
		return text({ success: true, done, total: args.todos.filter((t) => t.status !== "cancelled").length });
	},
});

/** Steps a plan with open items may go unchanged before the model is reminded of it. */
export const PLAN_STALE_STEPS = 5;

/** Each conversation's plan as its last `todo` left it: the step it shows as current, and the
 * steps taken since. Kept in memory: after a restart the next `todo` starts it again. */
const plans = new Map<string, { current: string; since: number }>();

/**
 * What the model is told after a tool call about its plan, if anything. Models (Qwen above all)
 * write a plan at the start and never touch it again, and the person reads a plan left as it was
 * as work still at its first step: every few steps with open items unchanged, a line says so.
 */
export function planNudge(conversation: string, call: { name: string; arguments?: any }): string | undefined {
	if (call.name === "todo") {
		const todos: { content?: string; status?: string }[] = Array.isArray(call.arguments?.todos) ? call.arguments.todos : [];
		const open = todos.filter((t) => t.status === "in_progress" || t.status === "pending");
		const current = open.find((t) => t.status === "in_progress") ?? open[0];
		if (current) plans.set(conversation, { current: String(current.content ?? ""), since: 0 });
		else plans.delete(conversation);
		return undefined;
	}
	const plan = plans.get(conversation);
	if (!plan || ++plan.since % PLAN_STALE_STEPS) return undefined;
	return `[Your plan has not changed for ${plan.since} steps; it still shows "${plan.current}" as the current step. If that step is done or you have moved on, call todo with the whole list now: the person reads the plan as where you are.]`;
}

export const sessionSearch = defineTool({
	name: "session_search",
	description: "Search the person's past sessions (their text) when they refer to earlier work: 'what did we do about X'.",
	parameters: Type.Object({ query: Type.String(), limit: Type.Optional(Type.Number({ minimum: 1, maximum: 10 })) }),
	execute: async (args, api) => {
		const s = threadOf(api.conversationId);
		const hits = host()
			.search(args.query, args.limit ?? 5, s.agent)
			.filter((h) => h.thread_id !== s.id);
		return text(hits.length ? hits : { results: [], note: "Nothing found in past sessions." });
	},
});
