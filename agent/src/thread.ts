// A thread for Bench: Pi's journal as an ordered list of items, plus its queue.
//
// The journal is append-only and every entry has an id that only grows, so an item's key is
// built from its entry id and its place inside the entry. Bench orders items by key and nothing
// else. Queued messages come after the whole journal, in the inbox's order.

import { InboxDoc } from "@earendil-works/pi-durable";
import { Inserts } from "./inserts.ts";

/** A tool call in one line: what it acts on (a command, a path, a query…). */
export function preview(name: string, args: any): string {
	if (!args || typeof args !== "object") return "";
	const pick = args.command ?? args.path ?? args.query ?? args.url ?? args.urls?.[0] ?? args.name ?? args.goal ?? args.pattern ?? args.action;
	const text = typeof pick === "string" ? pick : JSON.stringify(args);
	return text.length > 160 ? text.slice(0, 157) + "..." : text;
}

// The items and the queue are typed in crates/protocol: Bench reads them with the same types.
import type { QueuedItem, ThreadItem } from "./protocol.gen.ts";
export type { QueuedItem, ThreadItem };

/** A display-only entry: why a run stopped. It has no `model`, so the model never sees it. */
export const FAIL_KIND = "bench.fail";

export const INBOX_KIND = (InboxDoc as any).kind ?? "pi.inbox";
export const INSERTS_KIND = (Inserts as any).kind ?? "bench.inserts";

export const BLOCK_OPEN = "<block-state>";
export const BLOCK_CLOSE = "</block-state>";

/** A user message without the block state Bench sent with it (that is for the model). */
export function stripBlock(text: string): string {
	const at = text.lastIndexOf(BLOCK_OPEN);
	return at >= 0 && text.trimEnd().endsWith(BLOCK_CLOSE) ? text.slice(0, at).trimEnd() : text;
}

/** The text of a message's content: a string, or the text blocks of a list. */
export function textOf(content: any): string {
	if (typeof content === "string") return content;
	if (!Array.isArray(content)) return "";
	return content
		.filter((b: any) => b?.type === "text")
		.map((b: any) => b.text)
		.join("");
}

/** The items of one journal entry, in order. Entries the person never sees give none. */
export function itemsOf(entry: any): ThreadItem[] {
	const seq = Number(entry.id);
	const out: ThreadItem[] = [];
	if (entry.kind === "bench.insert") {
		out.push({ key: `${seq}`, seq, kind: "insert", id: String(entry.data?.id), insert_key: String(entry.data?.key) });
		return out;
	}
	if (entry.kind === FAIL_KIND) {
		out.push({ key: `${seq}`, seq, kind: "fail", text: String(entry.data?.text ?? "") });
		return out;
	}
	if (entry.kind === "pi.compaction" || entry.kind === "pi.system") return out;
	let n = 0;
	const key = () => `${seq}.${n++}`;
	for (const m of entry.model ?? []) {
		const at = typeof m.timestamp === "number" ? m.timestamp : undefined;
		if (m.role === "user") {
			const text = stripBlock(textOf(m.content));
			if (text) out.push({ key: key(), seq, kind: "user", text, at });
		} else if (m.role === "assistant") {
			for (const b of m.content ?? []) {
				if (b.type === "thinking" && b.thinking?.trim()) out.push({ key: key(), seq, kind: "thought", text: b.thinking, at });
				else if (b.type === "text" && b.text?.trim()) out.push({ key: key(), seq, kind: "text", text: b.text, at });
				else if (b.type === "toolCall") out.push({ key: key(), seq, kind: "tool", call_id: String(b.id), name: String(b.name), args: b.arguments ?? {}, preview: preview(String(b.name), b.arguments), at });
			}
			if (m.stopReason === "error") out.push({ key: key(), seq, kind: "text", text: "", at, error: String(m.errorMessage ?? "the model stopped with an error") });
		} else if (m.role === "toolResult") {
			// An edit's unified patch, as Pi's edit tool reports it in its details.
			const patch = (entry.data as any)?.details?.patch ?? m.details?.patch;
			out.push({ key: key(), seq, kind: "result", call_id: String(m.toolCallId), name: String(m.toolName), output: textOf(m.content), failed: !!m.isError, ...(typeof patch === "string" ? { diff: patch } : {}), at });
		}
	}
	return out;
}

/** The items of many entries, in journal order. */
export function itemsOfAll(entries: readonly any[]): ThreadItem[] {
	return [...entries].sort((a, b) => Number(a.id) - Number(b.id)).flatMap(itemsOf);
}

/** The queue as Bench shows it: the inbox's own order, writes (inserts, notes) left out. */
export function queueOf(inbox: { items?: readonly any[] } | null | undefined): QueuedItem[] {
	return (inbox?.items ?? [])
		.filter((i) => i.mode !== "write")
		.map((i) => ({ key: `q${i.id}`, sub: String(i.id), mode: i.mode, text: stripBlock(textOf(i.content)) }));
}

/** What one Pi commit means for the threads Bench watches, by conversation id. */
export interface ThreadChange {
	conversation: string;
	append?: ThreadItem[];
	queue?: QueuedItem[];
	inserts?: Record<string, unknown>;
}

/**
 * Read one commit publication: new entries become `append` (in id order), a new inbox revision
 * becomes `queue`, a new inserts document becomes `inserts`. Only conversations `watched`
 * answers yes for are read.
 */
export function changesOf(changes: readonly any[], watched: (conversation: string) => boolean): ThreadChange[] {
	const by = new Map<string, ThreadChange>();
	const at = (c: string) => by.get(c) ?? by.set(c, { conversation: c }).get(c)!;
	const entries: any[] = [];
	for (const ch of changes) {
		if (ch.type === "entry") {
			const c = String(ch.value.conversationId);
			if (watched(c)) entries.push(ch.value);
		} else if (ch.type === "document" && ch.conversationId !== undefined && ch.value) {
			const c = String(ch.conversationId);
			if (!watched(c)) continue;
			if (ch.record?.kind === INBOX_KIND) at(c).queue = queueOf(ch.value);
			else if (ch.record?.kind === INSERTS_KIND) at(c).inserts = (ch.value as any).items ?? {};
		}
	}
	entries.sort((a, b) => Number(a.id) - Number(b.id));
	for (const e of entries) {
		const items = itemsOf(e);
		if (!items.length) continue;
		const t = at(String(e.conversationId));
		(t.append ??= []).push(...items);
	}
	return [...by.values()];
}
