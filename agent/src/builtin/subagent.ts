// `delegate`: hands self-contained tasks to subagents, Pi Durable's way (README "Abort and
// Subagents"): each task gets a child conversation owned by this tool call, started as a copy of
// this conversation's agent minus `delegate`, and the call returns their answers. Replay-safe: a
// rerun after a crash finds the same children and the same submissions. Several tasks run at once.

import { Type } from "@earendil-works/pi-ai";
import { AssistantEntry, defineTool } from "@earendil-works/pi-durable";
import { host, sessionOf } from "../host.ts";

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const delegate = defineTool({
	name: "delegate",
	description:
		"Give independent, self-contained tasks to subagents that work in parallel with the same tools and return their answers. " +
		"Each knows nothing of this conversation: put everything it needs (paths, facts, constraints) in its task. Up to 4 tasks per call.",
	parameters: Type.Object({
		tasks: Type.Array(Type.Object({ goal: Type.String(), context: Type.Optional(Type.String()) }), { minItems: 1, maxItems: 4 }),
	}),
	replay: "safe",
	execute: async (args, api, context) => {
		const parent = sessionOf(api.conversationId);
		const children: string[] = await api.commit(async (tx: any) => {
			const existing = (await tx.scanConversations({ ownerTaskId: api.taskId }, 16)).items.map((c: any) => c.id);
			const ids = [...existing];
			for (let i = existing.length; i < args.tasks.length; i++) {
				const created = await tx.createConversation({ ownership: { kind: "task", taskId: api.taskId } });
				ids.push(created.id);
			}
			return ids;
		}, context);
		for (const c of children) host().adopt(c, parent.id);
		await api.details({ subagents: children } as any, context);
		const results = await Promise.all(
			args.tasks.map(async (t, i) => {
				const id = `${api.callId}:${i}`;
				host().emit(parent.id, "subagent.start", { subagent_id: id, goal: t.goal, task_index: i });
				const child = await api.conversation(children[i], context);
				if (!child) return { task: i, status: "failed", answer: "the subagent's conversation is gone" };
				await child.configure({ tools: { remove: [delegate] } } as any, context);
				const content = t.context ? `${t.goal}\n\nContext:\n${t.context}` : t.goal;
				const settled: any = await (await child.submit({ type: "input", content, requestId: `delegate:${api.taskId}:${i}` }, context)).wait(context);
				const entry = settled.status === "done" ? await child.commit((tx: any) => tx.entry(AssistantEntry, settled.answer), context) : undefined;
				const answer = entry ? answerText(entry) : `unanswered: ${settled.reason ?? "?"}`;
				host().emit(parent.id, "subagent.complete", { subagent_id: id, summary: answer, status: settled.status === "done" ? "completed" : "failed" });
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
