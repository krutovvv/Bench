// What the agent's tools may ask of the server: which session a conversation is, put a question
// or an approval to the person (Bench's card, Telegram's buttons), run a prompt in a session of
// its own. The runtime installs itself here at start (`runtime/tools-host.ts`). Light on purpose: every
// tool imports it.

export interface ThreadFacts {
	id: string;
	agent: string;
	cwd: string;
	source: string;
	/** Telegram sessions: their chat. */
	chat_id?: string;
	/** The project the thread belongs to; "" = none. */
	project?: string;
	/** What the person inserted into the thread, in order (src/inserts.ts). */
	inserts?: { id: string; key: string; state: Record<string, unknown> }[];
}

import type { Process } from "./protocol.gen.ts";
import type { RunEvent } from "./runtime/runtime.ts";

export interface Host {
	thread(conversationId: string | number): ThreadFacts | undefined;
	/** Waits while the person holds the run at a step boundary; resolves at once when free.
	 * False when the run was stopped while it waited: the step must not begin. */
	hold(threadId: string, signal?: AbortSignal): Promise<boolean>;
	/** A subagent's conversation acts for its parent's session. */
	adopt(conversationId: string | number, threadId: string): void;
	/** How many threads other than `threadId` are running a turn now (each holds one of the model server's slots). */
	busyElsewhere(threadId: string): number;
	/** Ask the person; `callId` is the asking call's, the id their answer comes back to. The
	 * answer, or "" when declined or timed out. */
	ask(threadId: string, callId: string, question: string, choices: string[], multiSelect: boolean, signal?: AbortSignal): Promise<string>;
	/** "once" | "session" | "always" | "deny". */
	approve(threadId: string, callId: string, command: string, description: string): Promise<string>;
	/** What a run did, for the listeners in this process (`RunEvent`: a subagent started…). */
	report(threadId: string, e: RunEvent): void;
	/** The background processes of a conversation, oldest first (`builtin/process.ts`). */
	processes(conversationId: string | number): Promise<Process[]>;
	/** Stop a background process by its task id. */
	stopProcess(id: string): Promise<void>;
	search(query: string, limit: number, agent: string): { thread_id: string; title: string; snippet: string; when: string }[];
	/** Runs one prompt to its answer in a new session of this agent (cron jobs). */
	run(agent: string, prompt: string, opts: { source: string; title?: string; cwd?: string }): Promise<{ threadId: string; text: string }>;
	/** Sends text to a delivery target ("telegram:<chat>"). */
	deliver(target: string, text: string): Promise<void>;
	/** Stops the thread's run, as the person's Stop does (the step limit, `builtin/steps.ts`). */
	stop(threadId: string): void;
	/** The evaluator asked once (`call`) or changed the course (`nudge`): the thread's meters count it. */
	count(conversationId: string | number, what: "call" | "nudge"): void;
}

let current: Host | undefined;

export function setHost(h: Host): void {
	current = h;
}

export function host(): Host {
	if (!current) throw new Error("the agent host is not running");
	return current;
}

/** The session a tool call belongs to. */
export function threadOf(conversationId: string | number): ThreadFacts {
	const s = host().thread(conversationId);
	if (!s) throw new Error("this conversation has no session");
	return s;
}

