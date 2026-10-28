// A thread for Bench: Pi's journal as an ordered list of items, plus its queue.
//
// The journal is append-only and every entry has an id that only grows, so an item's key is
// built from its entry id and its place inside the entry. Bench orders items by key and nothing
// else. Queued messages come after the whole journal, in the inbox's order.

import { InboxDoc } from "@earendil-works/pi-durable";
import { processOf } from "./builtin/process.ts";
import { Inserts } from "./inserts.ts";

/** A tool call in one line: what it acts on (a command, a path, a query…). */
export function preview(name: string, args: any): string {
	if (!args || typeof args !== "object") return "";
	// A code-mode script: its first line that says something.
	if (name === "codemode" && typeof args.code === "string") {
		const line = args.code.split("\n").map((l: string) => l.trim()).find((l: string) => l && !l.startsWith("//")) ?? "";
		return line.length > 160 ? line.slice(0, 157) + "..." : line;
	}
	const pick = args.command ?? args.path ?? args.query ?? args.url ?? args.urls?.[0] ?? args.name ?? args.goal ?? args.pattern ?? args.action;
	const text = typeof pick === "string" ? pick : JSON.stringify(args);
	return text.length > 160 ? text.slice(0, 157) + "..." : text;
}

// The items and the queue are typed in crates/protocol: Bench reads them with the same types.
import type { Inserts as InsertStates, PlanStep, Process, QueuedItem, ThreadItem } from "./protocol.gen.ts";
export type { QueuedItem, ThreadItem };

/** A display-only entry: why a run stopped. It has no `model`, so the model never sees it. */
export const FAIL_KIND = "bench.fail";

/** Pi's compaction summary: a new context, with the older entries left out of what the model reads. */
export const COMPACTION_KIND = "pi.compaction";

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

/**
 * What Pi's submission records say about the person's messages: Bench's id for each (the
 * `requestId` it sent with it), by the submission while it waits, and by its entry once Pi
 * placed it. Bench matches the row it showed on Enter by this id, never by the words.
 */
export class Requests {
	private bySub = new Map<string, string>();
	private byEntry = new Map<number, { sub: string; request?: string }>();

	/** Learn from one submission record (a commit's change, or a scan). */
	note(rec: any): void {
		if (rec?.type !== "input") return;
		const sub = String(rec.id);
		if (rec.requestId) this.bySub.set(sub, String(rec.requestId));
		if (rec.entry !== undefined) this.byEntry.set(Number(rec.entry), { sub, request: rec.requestId ? String(rec.requestId) : undefined });
	}

	ofSub(sub: string): string | undefined {
		return this.bySub.get(sub);
	}

	ofEntry(entry: number): { sub: string; request?: string } | undefined {
		return this.byEntry.get(entry);
	}
}

/** The steps a `todo` call wrote. */
function planOf(args: any): PlanStep[] {
	const todos = Array.isArray(args?.todos) ? args.todos : [];
	return todos.map((t: any) => ({ id: String(t?.id ?? ""), content: String(t?.content ?? ""), status: String(t?.status ?? "pending"), ...(typeof t?.parent === "string" ? { parent: t.parent } : {}) }));
}

/** The items of one journal entry, in order. Entries the person never sees give none. */
export function itemsOf(entry: any, requests?: Requests): ThreadItem[] {
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
	if (entry.kind === COMPACTION_KIND || entry.kind === "pi.system") return out;
	let n = 0;
	const key = () => `${seq}.${n++}`;
	for (const m of entry.model ?? []) {
		const at = typeof m.timestamp === "number" ? m.timestamp : undefined;
		if (m.role === "user") {
			const text = stripBlock(textOf(m.content));
			// The submission that brought it, and Bench's id for it, when Pi's records say.
			const from = requests?.ofEntry(seq);
			if (text) out.push({ key: key(), seq, kind: "user", text, at, ...(from ? { sub: from.sub, ...(from.request ? { request: from.request } : {}) } : {}) });
		} else if (m.role === "assistant") {
			for (const b of m.content ?? []) {
				if (b.type === "thinking" && b.thinking?.trim()) out.push({ key: key(), seq, kind: "thought", text: b.thinking, at });
				else if (b.type === "text" && b.text?.trim())
					// The words of a message that also calls a tool are the agent's narrative,
					// not an answer — Pi's own signal, read from the message's blocks.
					out.push({
						key: key(),
						seq,
						kind: "text",
						text: b.text,
						at,
						narration: m.content.some((x: any) => x?.type === "toolCall"),
					});
				else if (b.type === "toolCall") {
					out.push({ key: key(), seq, kind: "tool", call_id: String(b.id), name: String(b.name), args: b.arguments ?? {}, preview: preview(String(b.name), b.arguments), at });
					// The plan is the `todo` call itself (docs/thread-objects.md): its steps, read
					// from the journal, under the call's id.
					if (b.name === "todo") out.push({ key: key(), seq, kind: "plan", id: String(b.id), todos: planOf(b.arguments) });
					// So is a question: the `clarify` call, answered by its result.
					if (b.name === "clarify") {
						const a = b.arguments ?? {};
						const choices = (Array.isArray(a.choices) ? a.choices : []).map(String);
						out.push({ key: key(), seq, kind: "ask", id: String(b.id), ask_kind: "clarify", question: String(a.question ?? ""), choices, multi: Boolean(a.multi_select) && choices.length > 0 });
					}
				}
			}
			if (m.stopReason === "error") out.push({ key: key(), seq, kind: "text", text: "", at, error: String(m.errorMessage ?? "the model stopped with an error") });
		} else if (m.role === "toolResult") {
			// An edit's unified patch, as Pi's edit tool reports it in its details.
			const patch = (entry.data as any)?.details?.patch ?? m.details?.patch;
			out.push({ key: key(), seq, kind: "result", call_id: String(m.toolCallId), name: String(m.toolName), output: textOf(m.content), failed: !!m.isError, ...(typeof patch === "string" ? { diff: patch } : {}), at });
			// A question's answer is its call's result ("" declined, or no answer came).
			if (m.toolName === "clarify") {
				const answer = m.details?.answer ?? (entry.data as any)?.details?.answer;
				out.push({ key: key(), seq, kind: "asked", id: String(m.toolCallId), answer: typeof answer === "string" ? answer : "" });
			}
		}
	}
	return out;
}

/** The items of many entries, in journal order. */
export function itemsOfAll(entries: readonly any[], requests?: Requests): ThreadItem[] {
	return [...entries].sort((a, b) => Number(a.id) - Number(b.id)).flatMap((e) => itemsOf(e, requests));
}

/** How often this thread was summarized: one for each summary in its journal. The journal keeps
 *  them even though they are not rows, so the count is there again after a restart. */
export function compactionsOf(entries: readonly any[]): number {
	return entries.reduce((n, e) => n + (e?.kind === COMPACTION_KIND ? 1 : 0), 0);
}

/** The queue as Bench shows it: the inbox's own order, writes (inserts, notes) left out. */
export function queueOf(inbox: { items?: readonly any[] } | null | undefined, requests?: Requests): QueuedItem[] {
	return (inbox?.items ?? [])
		.filter((i) => i.mode !== "write")
		.map((i) => {
			const request = requests?.ofSub(String(i.id));
			return { key: `q${i.id}`, sub: String(i.id), mode: i.mode, text: stripBlock(textOf(i.content)), ...(request ? { request } : {}) };
		});
}

/** What one Pi commit means for the threads Bench watches, by conversation id. */
export interface ThreadChange {
	conversation: string;
	append?: ThreadItem[];
	queue?: QueuedItem[];
	inserts?: InsertStates;
	/** Background processes whose task records this commit changed, in id order. */
	processes?: Process[];
}

/**
 * Read one commit publication: new entries become `append` (in id order), a new inbox revision
 * becomes `queue`, a new inserts document becomes `inserts`, a changed process task becomes
 * one of `processes`. Only conversations `watched`
 * answers yes for are read.
 */
export function changesOf(changes: readonly any[], watched: (conversation: string) => boolean, requests: (conversation: string) => Requests): ThreadChange[] {
	const by = new Map<string, ThreadChange>();
	// The submissions first: an entry or a queue of this same commit is told with their ids.
	for (const ch of changes) {
		if (ch.type !== "submission") continue;
		const c = String(ch.value?.conversationId);
		if (watched(c)) requests(c).note(ch.value);
	}
	const at = (c: string) => by.get(c) ?? by.set(c, { conversation: c }).get(c)!;
	const entries: any[] = [];
	for (const ch of changes) {
		if (ch.type === "entry") {
			const c = String(ch.value.conversationId);
			if (watched(c)) entries.push(ch.value);
		} else if (ch.type === "document" && ch.conversationId !== undefined && ch.value) {
			const c = String(ch.conversationId);
			if (!watched(c)) continue;
			if (ch.record?.kind === INBOX_KIND) at(c).queue = queueOf(ch.value, requests(c));
			else if (ch.record?.kind === INSERTS_KIND) at(c).inserts = (ch.value as any).items ?? {};
		} else if (ch.type === "task") {
			const c = String(ch.value?.conversationId);
			const p = watched(c) ? processOf(ch.value) : undefined;
			if (p) (at(c).processes ??= []).push(p);
		}
	}
	entries.sort((a, b) => Number(a.id) - Number(b.id));
	for (const e of entries) {
		const items = itemsOf(e, requests(String(e.conversationId)));
		if (!items.length) continue;
		const t = at(String(e.conversationId));
		(t.append ??= []).push(...items);
	}
	return [...by.values()];
}
