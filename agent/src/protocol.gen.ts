// Generated from crates/protocol (cargo test -p protocol). Do not edit: change the Rust types.

export type ThreadItem = { "kind": "user", key: string, seq: number, text: string, at?: number, } | { "kind": "thought", key: string, seq: number, text: string, at?: number, } | { "kind": "text", key: string, seq: number, text: string, at?: number, error?: string, } | { "kind": "tool", key: string, seq: number, call_id: string, name: string, args: unknown, 
/**
 * What it acts on, in one line (a command, a path).
 */
preview: string, at?: number, } | { "kind": "result", key: string, seq: number, call_id: string, name: string, output: string, failed: boolean, 
/**
 * An edit's unified patch.
 */
diff?: string, at?: number, } | { "kind": "insert", key: string, seq: number, id: string, insert_key: string, } | { "kind": "fail", key: string, seq: number, text: string, };

export type QueuedItem = { key: string, sub: string, mode: string, text: string, };

export type ThreadOpen = { items: Array<ThreadItem>, queue: Array<QueuedItem>, inserts: Record<string, unknown>, running: boolean, head: number, };

export type StreamKind = "text" | "thought";
