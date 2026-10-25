// What the agent's tools may ask of the server: which session a conversation is, put a question
// or an approval to the person (Bench's card, Telegram's buttons), update the session's plan, run
// a prompt in a session of its own. The runtime installs itself here at start.

import type { Todo } from "./store.ts";

export interface SessionInfo {
	id: string;
	profile: string;
	cwd: string;
	source: string;
	/** Telegram sessions: their chat. */
	chat_id?: string;
	/** What the thread is about ("tasks:today", "ssh:<host>", "files:<host id>", "folder:<path>"). */
	subject?: string;
	/** The project the thread belongs to; "" = none. */
	project?: string;
	/** What the person inserted into the thread, in order (src/inserts.ts). */
	inserts?: { id: string; key: string; state: Record<string, unknown> }[];
}

export interface Host {
	session(conversationId: string | number): SessionInfo | undefined;
	/** A subagent's conversation acts for its parent's session. */
	adopt(conversationId: string | number, sessionId: string): void;
	/** The answer, or "" when declined or timed out. */
	ask(sessionId: string, question: string, choices: string[], multiSelect: boolean, signal?: AbortSignal): Promise<string>;
	/** "once" | "session" | "always" | "deny". */
	approve(sessionId: string, command: string, description: string): Promise<string>;
	setTodos(sessionId: string, todos: Todo[]): void;
	todos(sessionId: string): Todo[];
	emit(sessionId: string, type: string, payload: Record<string, unknown>): void;
	search(query: string, limit: number, profile: string): { session_id: string; title: string; snippet: string; when: string }[];
	/** Runs one prompt to its answer in a new session of this profile (cron jobs). */
	run(profile: string, prompt: string, opts: { source: string; title?: string; cwd?: string }): Promise<{ sessionId: string; text: string }>;
	/** Sends text to a delivery target ("telegram:<chat>"). */
	deliver(target: string, text: string): Promise<void>;
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
export function sessionOf(conversationId: string | number): SessionInfo {
	const s = host().session(conversationId);
	if (!s) throw new Error("this conversation has no session");
	return s;
}
