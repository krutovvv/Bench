// `delegate`: hands self-contained tasks to subagents, Pi Durable's way (README "Abort and
// Subagents"): each task gets a child conversation owned by this tool call, started as a copy of
// this conversation's agent minus `delegate`, and the call returns their answers. Replay-safe: a
// rerun after a crash finds the same children and the same submissions. Several tasks run at once,
// as many as the model server has slots free: a local model answers few requests at a time, and
// requests run side by side share its speed, so subagents beyond its slots only slow everyone.

import { Type } from "@earendil-works/pi-ai";
import { AssistantEntry, configure, defineTool, type ConversationId } from "@earendil-works/pi-durable";
import { providers, resolveModel } from "../config.ts";
import { host, threadOf } from "../host.ts";

/**
 * How many subagents may run at once: the main model's provider's slots (`parallel`) less the
 * threads running a turn elsewhere; at least one, since the waiting parent frees its own slot.
 * No slots set: all at once.
 */
export function subagentSlots(tasks: number, parallel: number | undefined, busyElsewhere: number): number {
	if (!parallel) return tasks;
	return Math.max(1, Math.min(tasks, parallel - busyElsewhere));
}

/** Runs `jobs` with at most `limit` at once, results in order. */
async function pool<T>(limit: number, jobs: (() => Promise<T>)[]): Promise<T[]> {
	const out: T[] = new Array(jobs.length);
	let next = 0;
	const worker = async () => {
		while (next < jobs.length) {
			const i = next++;
			out[i] = await jobs[i]();
		}
	};
	await Promise.all(Array.from({ length: Math.min(limit, jobs.length) }, worker));
	return out;
}

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const delegate = defineTool({
	name: "delegate",
	description:
		"Give independent, self-contained tasks to subagents that work with the same tools and return their answers. " +
		"Each knows nothing of this conversation: put everything it needs (paths, facts, constraints) in its task. Up to 4 tasks per call. " +
		"They share the model server with every thread: when it answers few requests at once, they run one or two at a time and slow the others, " +
		"so delegate only large, truly independent work; small steps are faster done yourself.",
	parameters: Type.Object({
		tasks: Type.Array(Type.Object({ goal: Type.String(), context: Type.Optional(Type.String()) }), { minItems: 1, maxItems: 4 }),
	}),
	replay: "safe",
	execute: async (args, api, context) => {
		const parent = threadOf(api.conversationId);
		// The children are made and set up in one commit, as Pi's README shows: each starts as a
		// copy of this conversation's agent, less `delegate` (no subagents of its own).
		const children: ConversationId[] = await api.commit(async (tx) => {
			const existing = (await tx.scanConversations({ ownerTaskId: api.taskId }, 16)).items.map((c) => c.id);
			const ids = [...existing];
			for (let i = existing.length; i < args.tasks.length; i++) {
				const created = await tx.createConversation({ ownership: { kind: "task", taskId: api.taskId } });
				await configure(tx, created.id, { tools: { remove: [delegate] } });
				ids.push(created.id);
			}
			return ids;
		}, context);
		for (const c of children) host().adopt(c, parent.id);
		await api.details({ subagents: children } as any, context);
		const parallel = providers().find((p) => p.id === resolveModel().provider)?.parallel;
		const limit = subagentSlots(args.tasks.length, parallel, host().busyElsewhere(parent.id));
		const results = await pool(
			limit,
			args.tasks.map((t, i) => async () => {
				host().report(parent.id, { kind: "subagent", goal: t.goal });
				const child = await api.conversation(children[i], context);
				if (!child) return { task: i, status: "failed", answer: "the subagent's conversation is gone" };
				const content = t.context ? `${t.goal}\n\nContext:\n${t.context}` : t.goal;
				const settled: any = await (await child.submit({ type: "input", content, requestId: `delegate:${api.taskId}:${i}` }, context)).wait(context);
				const entry = settled.status === "done" ? await api.commit((tx) => tx.entry(AssistantEntry, settled.answer), context) : undefined;
				const answer = entry ? answerText(entry) : `unanswered: ${settled.reason ?? "?"}`;
				return { task: i, status: settled.status === "done" ? "completed" : "failed", answer };
			}),
		);
		return text({ results });
	},
});

function answerText(answer: any): string {
	const msg = answer?.model?.[0] ?? answer;
	const content = msg?.content ?? [];
	return Array.isArray(content) ? content.filter((b: any) => b.type === "text").map((b: any) => b.text).join("").trim() : String(content);
}
