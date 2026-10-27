// Generated from crates/protocol (cargo test -p protocol). Do not edit: change the Rust types.

export type ThreadItem = { "kind": "user", key: string, seq: number, text: string, at?: number, 
/**
 * The Pi submission that brought it, when the agent's records say.
 */
sub?: string, 
/**
 * Bench's id for the message (the `request_id` it was sent with): the row shown on
 * Enter is this one (docs/thread-objects.md).
 */
request?: string, } | { "kind": "thought", key: string, seq: number, text: string, at?: number, } | { "kind": "text", key: string, seq: number, text: string, 
/**
 * The words of an assistant message that also called a tool: the agent's narrative
 * of what it is about to do, not an answer. Pi's own signal — the message's blocks.
 * `None` is an agent too old to mark them; such texts are judged by their place.
 */
narration?: boolean, at?: number, error?: string, } | { "kind": "tool", key: string, seq: number, call_id: string, name: string, args: unknown, 
/**
 * What it acts on, in one line (a command, a path).
 */
preview: string, at?: number, } | { "kind": "result", key: string, seq: number, call_id: string, name: string, output: string, failed: boolean, 
/**
 * An edit's unified patch.
 */
diff?: string, at?: number, } | { "kind": "insert", key: string, seq: number, id: string, insert_key: string, } | { "kind": "fail", key: string, seq: number, text: string, } | { "kind": "plan", key: string, seq: number, id: string, todos: Array<PlanStep>, } | { "kind": "ask", key: string, seq: number, id: string, 
/**
 * "clarify" or "approve".
 */
ask_kind: string, question: string, 
/**
 * Extra text under the question: the command an approval wants to run.
 */
detail?: string, choices: Array<string>, multi: boolean, } | { "kind": "asked", key: string, seq: number, id: string, answer?: string, };

export type PlanStep = { id: string, content: string, 
/**
 * "pending", "in_progress", "completed", "cancelled".
 */
status: string, parent?: string, };

export type QueuedItem = { key: string, sub: string, mode: string, text: string, 
/**
 * Bench's id for the message, when Bench sent it.
 */
request?: string, };

export type ThreadOpen = { items: Array<ThreadItem>, queue: Array<QueuedItem>, inserts: Record<string, unknown>, running: boolean, 
/**
 * The live run's holds (the person's pauses), oldest first; the last one open while it
 * stands held. Empty when no run lives.
 */
holds: Array<Hold>, head: number, };

export type Hold = { from: number, to?: number, };

export type StreamKind = "text" | "thought" | "narration";
