// A run as it happens: Pi's events of a conversation (`watchEvents`) turned into what Bench and
// the listeners in this process hear — the answer being written (`stream`), the run starting and
// ending, its numbers, why it failed. What stays is the journal's: Bench reads entries, and the
// search index its words, from Pi's commits (`thread.ts`), never from here.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import type { AgentEvent } from "@earendil-works/pi-durable";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import { resolveModel } from "../config.ts";
import type { Usage } from "../protocol.gen.ts";
import type { Live, Runtime } from "./runtime.ts";
import { FAIL_KIND } from "./journal.ts";
import { usagePayload } from "./usage.ts";
import { evaluatorOn, judgeSettings } from "../judge/client.ts";

const ctx = BACKGROUND_CONTEXT;

/** How a stopped run says so in the history: one quoted line, the same live and after a restart. */
const RUN_STOPPED = "> The run stopped: ";

export class Stream {
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	onEvent(live: Live, e: AgentEvent): void {
		const id = live.row.id;
		if (process.env.BENCH_AGENT_TRACE) fs.appendFileSync(process.env.BENCH_AGENT_TRACE, JSON.stringify(e) + "\n");
		switch (e.type) {
			case "submission": {
				// A message placed in the conversation while the run lives (a steer joins it, a
				// follow-up starts its next turn): from here the work answers it, so the answer so
				// far stays with the message before. Bench sees the entry through the journal.
				const rec: any = e.record;
				if (rec?.type !== "input" || rec.status !== "placed" || !live.busy) break;
				live.answer = "";
				live.blocks.clear();
				live.narrating = false;
				break;
			}
			case "run_start":
				if (!live.busy) {
					live.busy = true;
					live.generation++;
					this.beginRun(live);
				}
				break;
			case "message_start": {
				live.blocks.clear();
				live.narrating = false;
				// The first tokens can come with the start itself.
				const msg: any = e.message;
				if (msg?.role === "assistant") msg.content?.forEach((b: any, i: number) => this.onChange(live, { type: "block", contentIndex: i, block: b }));
				break;
			}
			case "message_update":
				for (const c of e.changes) this.onChange(live, c as any);
				if (e.usage) live.usage.input = (e.usage.input ?? 0) + (e.usage.cacheRead ?? 0) + (e.usage.cacheWrite ?? 0) || live.usage.input;
				break;
			case "message_end": {
				const msg: any = e.entry.model?.[0];
				if (msg?.role === "assistant") {
					// Whatever the deltas missed (a final block replaced at once) is flushed here.
					msg.content?.forEach((b: any, i: number) => this.onChange(live, { type: "block", contentIndex: i, block: b }));
					const text = (msg.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
					if (text.trim()) live.answer = text;
					live.usage.calls++;
					if (msg.usage) {
						live.usage.input = (msg.usage.input ?? 0) + (msg.usage.cacheRead ?? 0) + (msg.usage.cacheWrite ?? 0) || live.usage.input;
						live.usage.output += msg.usage.output ?? 0;
						live.usage.cached = msg.usage.cacheRead ?? 0;
					}
					if (msg.stopReason === "error") this.failTurn(live, msg.errorMessage ?? "the model stopped with an error");
					else this.rt.tell(id, { kind: "info", value: this.rt.models.info(live.row, msg) });
				}
				live.blocks.clear();
				live.narrating = false;
				break;
			}
			case "tool_execution_start":
				if (!e.parentToolCallId) this.rt.report(id, { kind: "tool", name: e.toolName });
				break;
			case "usage_changed":
				this.emitUsage(live);
				break;
			// The summary itself is the count's source (the journal), but it arrived now: the bar
			// moves without waiting for the thread to be read again. A failed compaction is not one.
			case "compaction_end":
				if (!e.error) {
					live.usage.compactions = (live.usage.compactions ?? 0) + 1;
					this.emitUsage(live);
				}
				break;
			case "run_end":
				this.finish(live);
				break;
			case "task_failed":
				console.error("[pi] task failed", e.kind, e.message);
				// The generation task is the run itself: its failure is the reason the turn went
				// quiet, and run_end follows without carrying it.
				if (e.kind === "pi.generation") this.failTurn(live, e.message);
				break;
		}
	}

	private onChange(live: Live, c: { type: string; contentIndex: number; delta?: string; block?: any }): void {
		const id = live.row.id;
		const blockText = (b: any) => (b?.type === "text" ? b.text : b?.type === "thinking" ? b.thinking : undefined);
		const push = (kind: string, text: string) => {
			if (!text) return;
			if (kind !== "thinking") this.rt.report(id, { kind: "delta", text });
			// The answer being written, before its entry exists. The entry's `append` replaces
			// it. Once this message has begun a tool call its words are narration: they go to
			// the work line, never to the answer's seat.
			this.rt.tell(id, { kind: "stream", value: [kind === "thinking" ? "thought" : live.narrating ? "narration" : "text", text] });
		};
		// Pi's own sign that these words will not be the answer: the model began a tool call
		// in this very message. Empty delta: the text streamed so far is narration too.
		const toolBegun = () => {
			if (live.narrating) return;
			live.narrating = true;
			this.rt.tell(id, { kind: "stream", value: ["narration", ""] });
		};
		const cur = live.blocks.get(c.contentIndex);
		if (c.type === "text_delta" || c.type === "thinking_delta") {
			const kind = c.type === "text_delta" ? "text" : "thinking";
			const b = cur ?? { type: kind, text: "" };
			b.text += c.delta ?? "";
			live.blocks.set(c.contentIndex, b);
			push(kind, c.delta ?? "");
		} else if (c.type === "toolcall_start" || c.type === "toolcall_delta") {
			toolBegun();
		} else if (c.type === "text_start" || c.type === "thinking_start" || c.type === "block") {
			if (c.block?.type === "toolCall" || c.block?.type === "tool_use") return toolBegun();
			const full = blockText(c.block);
			if (full === undefined) return;
			const kind = c.block.type;
			const had = cur?.text ?? "";
			if (full.startsWith(had)) push(kind, full.slice(had.length));
			live.blocks.set(c.contentIndex, { type: kind, text: full });
		}
	}

	/** The thread's numbers as Bench shows them. */
	usageOf(live: Live): Usage {
		return usagePayload(resolveModel().model, live.usage, resolveModel().contextLength, !!judgeSettings() && evaluatorOn(live.row.agent));
	}

	emitUsage(live: Live): void {
		const secs = Math.max(1, (Date.now() - live.usage.started) / 1000);
		// Speed of the turn in progress; between turns, the last turn's.
		if (live.busy) live.usage.tps = Math.round((live.usage.output / secs) * 10) / 10;
		this.rt.tell(live.row.id, { kind: "usage", value: this.usageOf(live) });
		// Between turns these are the thread's settled numbers: save them, so the bar is not
		// empty when the thread or the app is reopened.
		if (!live.busy) this.rt.store.update(live.row.id, { usage: { ...live.usage } });
	}

	/** A run begins: what the last one wrote, why it failed and its clock are its own, not this one's. */
	beginRun(live: Live): void {
		live.answer = "";
		live.blocks.clear();
		live.narrating = false;
		live.fail = undefined;
		live.usage.started = Date.now();
		live.usage.output = 0;
		this.rt.turns.begin(live);
		this.rt.report(live.row.id, { kind: "start" });
		this.rt.tell(live.row.id, { kind: "run", value: true });
		this.rt.listChanged();
	}

	/** The reason a run stopped, as one line. Said at once, kept in the history: without it a
	 * failed turn looks like an answer that had nothing to say, and after a restart there is
	 * nothing left to explain the emptiness. */
	private failTurn(live: Live, why: unknown): void {
		const text = String(why ?? "")
			.replace(/^error:?\s*/i, "")
			.replace(/\s+/g, " ")
			.trim()
			.slice(0, 400);
		if (!text || live.fail?.includes(text)) return;
		live.fail = live.fail ? `${live.fail}; ${text}` : text;
		// Kept in the journal, so a reopened thread still says why it stopped. Display-only: the
		// model never sees it.
		void live.conversation
			.submit({ type: "write", entry: { kind: FAIL_KIND, data: { text } }, requestId: randomUUID() } as any, ctx)
			.catch((err) => console.error("[fail entry]", err));
	}

	private finish(live: Live): void {
		if (!live.busy) return;
		live.busy = false;
		// Whatever the person held, the run is over: the hold lets go, nobody waits on it.
		this.rt.holds.end(live);
		// What the turn changed in its repository, into the journal under its answer.
		void this.rt.turns.settle(live).catch((err) => console.error("[turn changes]", err));
		this.rt.tell(live.row.id, { kind: "run", value: false });
		// The turn's blended rate, as one number worth remembering.
		const turnSecs = Math.max(1, (Date.now() - live.usage.started) / 1000);
		if (live.usage.output) live.usage.tps = Math.round((live.usage.output / turnSecs) * 10) / 10;
		const id = live.row.id;
		// The failure travels with the answer: half an answer plus the reason it stopped is the
		// truth of the turn, and it is what a reopened thread has to show.
		const text = live.answer + (live.fail ? `${live.answer ? "\n\n" : ""}${RUN_STOPPED}${live.fail}` : "");
		this.rt.store.touch(id);
		this.emitUsage(live);
		this.rt.report(id, { kind: "complete", text });
		this.rt.listChanged();
	}
}
