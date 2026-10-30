// Work Pi has parked: a task cut off by crashes this many times without a new state (`restartLimit`,
// 5 by default) is not run again until asked. Nothing moves in its thread then, so the thread says so
// and the person can run it again (`harness.retryTask`). Only tasks of a thread: the schedule's own
// conversation has none to say it in, and stays in the log.

import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";
import type { Runtime } from "./runtime.ts";

export class Parked {
	/** Parked tasks' ids by conversation. */
	private readonly tasks = new Map<string, Set<number>>();

	private readonly rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** Pi's report of a task it just parked (`onReport`, once for each). */
	note(task: { id: number; conversationId: unknown }): void {
		const conversation = String(task.conversationId);
		const of = this.tasks.get(conversation) ?? new Set<number>();
		of.add(Number(task.id));
		this.tasks.set(conversation, of);
		this.tell(conversation, true);
	}

	/** Those parked in an earlier life of the agent: Pi keeps them so, and reports them once. */
	async load(): Promise<void> {
		const graph = await this.rt.harness.taskGraph(ctx);
		for (const [id, task] of Object.entries((graph.value as any).tasks ?? {}) as [string, any][])
			if (task.blocked === "restart_limit") this.note({ id: Number(id), conversationId: task.conversationId });
		graph.dispose?.();
	}

	of(conversation: unknown): boolean {
		return !!this.tasks.get(String(conversation))?.size;
	}

	/** Run the thread's parked work again. */
	async retry(conversation: unknown): Promise<void> {
		const key = String(conversation);
		const of = this.tasks.get(key);
		if (!of) return;
		this.tasks.delete(key);
		for (const id of of) await this.rt.harness.retryTask(id as any, ctx);
		this.tell(key, false);
	}

	private tell(conversation: string, value: boolean): void {
		const row = this.rt.store.byConversation(conversation);
		if (row) this.rt.tell(row.id, { kind: "parked", value });
	}
}
