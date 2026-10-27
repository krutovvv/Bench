// `clarify` (a question with choices, Bench's card / Telegram's buttons) and `todo` (the plan Bench
// shows over the composer).

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { host, sessionOf } from "../host.ts";
import type { Todo } from "../store.ts";

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
	execute: async (args, api, context) => {
		const s = sessionOf(api.conversationId);
		const choices = (args.choices ?? []).map((c) => c.trim()).filter(Boolean).slice(0, 4);
		const answer = await host().ask(s.id, args.question, choices, Boolean(args.multi_select) && choices.length > 0, context?.abortSignal);
		return text(answer ? { answer } : { answer: null, note: "No answer (declined or timed out). Decide yourself or stop." });
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
	execute: async (args, api) => {
		const s = sessionOf(api.conversationId);
		host().setTodos(s.id, args.todos as Todo[]);
		const done = args.todos.filter((t) => t.status === "completed").length;
		return text({ success: true, done, total: args.todos.filter((t) => t.status !== "cancelled").length });
	},
});

export const sessionSearch = defineTool({
	name: "session_search",
	description: "Search the person's past sessions (their text) when they refer to earlier work: 'what did we do about X'.",
	parameters: Type.Object({ query: Type.String(), limit: Type.Optional(Type.Number({ minimum: 1, maximum: 10 })) }),
	execute: async (args, api) => {
		const s = sessionOf(api.conversationId);
		const hits = host()
			.search(args.query, args.limit ?? 5, s.profile)
			.filter((h) => h.session_id !== s.id);
		return text(hits.length ? hits : { results: [], note: "Nothing found in past sessions." });
	},
});
