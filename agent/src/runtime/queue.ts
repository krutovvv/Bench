// The thread's waiting messages (Pi's inbox): take one back, fix its words, send it now — and the
// arithmetic of their order.
//
// Pi's inbox is an array, and a boundary places what it finds **by position**: `applyBoundary`
// takes the first item of each kind it is allowed at that boundary (with the default
// `one-at-a-time` modes, exactly one steer at a tool round, one follow-up when the run answers).
// Position is therefore "what the agent answers next" — the ids only matter in that new ones
// arrive at the end. "Send now" is the one place that has to disagree with arrival order, so it
// is the one place that moves an item.
//
// Pi has no "edit a waiting message" nor "send this one first": `edit` and `sendNow` change the
// inbox document in a commit of ours — a workaround, the same document Pi itself commits to.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { InboxDoc, type SubmissionId } from "@earendil-works/pi-durable";
import { randomUUID } from "node:crypto";
import type { Live, Runtime } from "./runtime.ts";
import { BLOCK_CLOSE, BLOCK_OPEN, textOf } from "./journal.ts";

const ctx = BACKGROUND_CONTEXT;

/**
 * Put the item with `id` first, in place, and answer where it ended up: 0 when it was moved (or
 * already was first), -1 when the queue does not hold it — it was taken back or already placed,
 * and there is nothing to move.
 */
export function moveToFront<T extends { id: unknown }>(items: T[], id: string): number {
	const at = items.findIndex((i) => String(i.id) === id);
	if (at < 0) return -1;
	if (at > 0) items.unshift(...items.splice(at, 1));
	return 0;
}
/**
 * A waiting message's content with its words replaced: the block state Bench sent with it stays
 * (the model still sees what the insert showed), and so does every attachment — only the person's
 * words change. Reading and writing use the same two shapes: a string, or a text block then the
 * attachments.
 */
export function withText(content: any, text: string): any {
	const said = textOf(content);
	const at = said.lastIndexOf(BLOCK_OPEN);
	const block = at >= 0 && said.trimEnd().endsWith(BLOCK_CLOSE) ? `\n\n${said.slice(at)}` : "";
	const body = `${text}${block}`;
	const files = Array.isArray(content) ? content.filter((b: any) => b?.type !== "text") : [];
	return files.length ? [{ type: "text", text: body }, ...files] : body;
}

export class Queue {
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	private async live(id: string): Promise<Live> {
		const live = await this.rt.thread(id);
		if (!live) throw new Error(`thread not found: ${id}`);
		return live;
	}

	/** Take a waiting message back out of the queue. */
	async withdraw(id: string, submission: string): Promise<string> {
		const live = await this.live(id);
		// Pi settles the submission and takes its item out of the inbox in one commit.
		return await this.rt.harness.abortSubmission(Number(submission) as SubmissionId, ctx, live.conversation.id);
	}

	/**
	 * Change the words of a message that still waits in the queue. It has not gone anywhere, so a
	 * wrong word is fixable; one already placed in the run is not — Pi settles that submission — and
	 * comes back as `not_queued`.
	 */
	async edit(id: string, submission: string, text: string): Promise<"edited" | "not_queued"> {
		const live = await this.live(id);
		let edited = false;
		await live.conversation.commit(async (tx) => {
			const inbox = await tx.doc(InboxDoc, live.conversation.id);
			const at = inbox.items.findIndex((i: any) => String(i.id) === submission);
			if (at < 0 || inbox.items[at].mode === "write") return;
			inbox.items[at] = { ...inbox.items[at], content: withText(inbox.items[at].content, text) };
			edited = true;
		}, ctx);
		return edited ? "edited" : "not_queued";
	}

	/** Send a waiting message now: out of the queue, into the running work (or a new run). It is
	 * submitted afresh under a new request id: Pi keys a submission by its `requestId`, and the
	 * old one, just aborted, would be answered again instead of a new one made. */
	async sendNow(id: string, submission: string): Promise<void> {
		const live = await this.live(id);
		const inbox = await this.rt.harness.snapshot(InboxDoc, live.conversation.id, ctx);
		// A write is no message: only an input waiting can be sent.
		const item = (inbox?.items ?? []).find((i) => String(i.id) === submission);
		if (!item || item.mode === "write") return;
		if ((await this.rt.harness.abortSubmission(Number(submission) as SubmissionId, ctx, live.conversation.id)) !== "aborted") return;
		// Its turn shows when Pi places it (after the current tools, or when the run answers).
		const sub = await live.conversation.submit({ type: "input", content: item.content, ...(live.busy ? { whenBusy: "steer" as const } : {}), requestId: randomUUID() }, ctx);
		// A new submission lands at the END of the inbox, and a boundary places what stands first
		// of its kind — so without this move "send now" sends the message last, which is its
		// opposite. It is the message the person just chose, so it goes to the head, ahead of
		// whatever waited before it (those keep their order behind it).
		await live.conversation.commit(async (tx) => {
			const box = await tx.doc(InboxDoc, live.conversation.id);
			moveToFront(box.items, String(sub.id));
		}, ctx);
	}
}
