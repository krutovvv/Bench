// What a run asks the person and waits for: a question (the `clarify` tool) or a command to approve.
// What waits is live state, not the journal's: the call is already in the journal, and its result
// will be the answer. After a restart Pi runs the call again (or its `beforeTool`), which asks
// again — nothing to restore.
//
// What the person approved stays: "session" in the thread's document `bench.approvals` (a fork
// keeps what its thread allowed), "always" in its agent settings (`allowed`).

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { defineDoc } from "@earendil-works/pi-durable";
import { allowCommand, readAgent, readConfig } from "../config.ts";
import type { Ask } from "../protocol.gen.ts";
import type { Live, Runtime } from "./runtime.ts";

const ctx = BACKGROUND_CONTEXT;

export const ApprovalsDoc = defineDoc<{ commands: string[] }>({
	kind: "bench.approvals",
	version: 1,
	scope: "conversation",
	history: "latest",
	fork: "current",
	initial: () => ({ commands: [] }),
	checkpointWhen: () => true,
});

export class Asks {
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** What the live run waits for the person to answer. */
	of(live: Live): Ask[] {
		return [...live.prompts.values()].map((p) => p.ask);
	}

	/** The person's answer to the waiting call `promptId`; "" declines. */
	respond(id: string, promptId: string, answer: string): void {
		this.rt.live.get(id)?.prompts.get(promptId)?.resolve(answer);
	}

	/** Every question of the run declined: it was stopped. */
	declineAll(live: Live): void {
		for (const [id, p] of live.prompts) {
			p.resolve("");
			live.prompts.delete(id);
		}
	}

	/** A question to the person; the answer, or "" when declined or timed out. */
	async question(threadId: string, callId: string, question: string, choices: string[], signal?: AbortSignal): Promise<string> {
		const live = await this.rt.thread(threadId);
		if (!live) return "";
		const answer = this.wait(live, { id: callId, kind: "clarify", choices, command: "", description: "" }, question, Number(readConfig("default")?.agent?.clarify_timeout ?? 600));
		signal?.addEventListener("abort", () => live.prompts.get(callId)?.resolve(""));
		return answer;
	}

	/** A command to approve: "once" | "session" | "always" | "deny". One approved for the thread or
	 * for always is not asked again. */
	async approve(threadId: string, callId: string, command: string, description: string): Promise<string> {
		const live = await this.rt.thread(threadId);
		if (!live) return "deny";
		if (readAgent(live.row.agent).allowed?.includes(command)) return "always";
		const doc = await this.rt.harness.snapshot(ApprovalsDoc, live.conversation.id, ctx);
		if (doc?.commands?.includes(command)) return "session";
		const choice = await this.wait(
			live,
			{ id: callId, kind: "approve", choices: ["once", "session", "always", "deny"], command, description },
			`Approve this command?\n\n${command}\n\n${description}`,
			Number(readConfig("default")?.approvals?.timeout ?? 120),
		);
		if (choice === "always") allowCommand(live.row.agent, command);
		if (choice === "session")
			await live.conversation.commit(async (tx) => {
				const d = await tx.doc(ApprovalsDoc, live.conversation.id);
				if (!d.commands.includes(command)) d.commands.push(command);
			}, ctx);
		return choice || "deny";
	}

	/** Wait for the answer to `ask`: Bench hears what waits (`asks`), the listeners the question. */
	private wait(live: Live, ask: Ask, question: string, timeoutS: number): Promise<string> {
		// The thread's own view hears what waits; the list hears that the thread waits (the
		// sidebar's dot, `ThreadSummary.waiting`).
		const told = () => {
			this.rt.tell(live.row.id, { kind: "asks", value: this.of(live) });
			this.rt.listChanged();
		};
		return new Promise<string>((resolve) => {
			const timer = setTimeout(() => done(""), timeoutS * 1000);
			const done = (answer: string) => {
				clearTimeout(timer);
				if (!live.prompts.delete(ask.id)) return;
				told();
				resolve(answer);
			};
			live.prompts.set(ask.id, { resolve: done, ask });
			told();
			this.rt.report(live.row.id, { kind: "ask", ask, question });
		});
	}
}
