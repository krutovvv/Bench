// What a run asks the person and waits for: a question (the `clarify` tool) or a command to approve.
// What waits is live state, not the journal's: the call is already in the journal, and its result
// will be the answer. After a restart Pi runs the call again (or its `beforeTool`), which asks
// again — nothing to restore.

import { readConfig } from "./config.ts";
import type { Ask } from "./protocol.gen.ts";
import type { Live, Runtime } from "./runtime.ts";

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
	async question(sessionId: string, callId: string, question: string, choices: string[], signal?: AbortSignal): Promise<string> {
		const live = await this.rt.session(sessionId);
		if (!live) return "";
		const answer = this.wait(live, { id: callId, kind: "clarify", choices, command: "", description: "" }, question, Number(readConfig("default")?.agent?.clarify_timeout ?? 600));
		signal?.addEventListener("abort", () => live.prompts.get(callId)?.resolve(""));
		return answer;
	}

	/** A command to approve: "once" | "session" | "always" | "deny". One approved for the session
	 * is not asked again while the thread is open. */
	async approve(sessionId: string, callId: string, command: string, description: string): Promise<string> {
		const live = await this.rt.session(sessionId);
		if (!live) return "deny";
		if (live.approvedForSession.has(command)) return "session";
		const choice = await this.wait(
			live,
			{ id: callId, kind: "approve", choices: ["once", "session", "always", "deny"], command, description },
			`Approve this command?\n\n${command}\n\n${description}`,
			Number(readConfig("default")?.approvals?.timeout ?? 120),
		);
		if (choice === "session" || choice === "always") live.approvedForSession.add(command);
		return choice || "deny";
	}

	/** Wait for the answer to `ask`: Bench hears what waits (`asks`), the listeners the question. */
	private wait(live: Live, ask: Ask, question: string, timeoutS: number): Promise<string> {
		const told = () => this.rt.tell(live.row.id, { kind: "asks", value: this.of(live) });
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
