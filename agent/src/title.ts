// A thread's title: made up by the model from its first message, unless the person named it.

import { noThinking } from "./thinking.ts";
import type { Live, Runtime } from "./runtime.ts";

const TITLE_PROMPT =
	"You name chat sessions. Given the user's opening message, write a title that lets them find this conversation again in a list.\n\n" +
	"Rules:\n- 3 to 7 words, sentence case (capitalize only the first word and proper nouns).\n- Name what the user wants DONE, not that they asked a question.\n" +
	"- Keep technical terms, filenames, numbers, and error codes exact.\n- Drop filler words: the, this, my, a, an.\n" +
	"- No trailing punctuation, no quotes, no tool names, no 'Title:' prefix.\n- Never answer the message. Name it.\n- Always produce something, even for a bare greeting.\n" +
	"- Write the title in the same language as the user's message.\n" +
	'Good: {"title": "Fix login button on mobile"}\nGood: {"title": "Postgres connection pool exhaustion"}\nGood: {"title": "Friendly greeting"}\n' +
	'Too vague: {"title": "Code changes"}\nToo long: {"title": "Investigate and fix the issue where the login button does not respond on mobile devices"}\n\n' +
	'Reply with JSON only: {"title": "..."}';

export class Titles {
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** The person's name for a thread: it stays (no title is made up over it). */
	rename(id: string, title: string): void {
		const t = title.replace(/\s+/g, " ").trim().slice(0, 120);
		if (!t || !this.rt.store.get(id)) throw new Error(`thread not found: ${id}`);
		this.rt.store.update(id, { title: t });
		const live = this.rt.live.get(id);
		if (live) live.row = this.rt.store.get(id)!;
		this.rt.listChanged();
	}

	/** Name the thread from its first message (a short call to its model, thinking off). */
	async name(live: Live, message: string): Promise<void> {
		const fallback = message.replace(/\s+/g, " ").trim().slice(0, 60);
		try {
			const model = this.rt.models.model(this.rt.models.ref(live.row.agent));
			const context = { systemPrompt: TITLE_PROMPT, messages: [{ role: "user", content: message.slice(0, 2000), timestamp: Date.now() }] };
			// A title needs no reasoning: off, also on a server that thinks unless told not to.
			const ask = (onPayload?: (p: any, m: any) => any) => (this.rt.models.pi as any).complete(model, context, { reasoningEffort: undefined, maxTokens: 400, onPayload });
			// A server that refuses the switch answers with an error: asked again without it.
			let res: any = await ask(noThinking).catch(() => undefined);
			if (!res || res.stopReason === "error") res = await ask();
			const out = (res?.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
			const m = out.match(/"title"\s*:\s*"([^"]+)"/);
			const title = (m?.[1] ?? out.replace(/<think>[\s\S]*?<\/think>/g, "")).trim().replace(/^["']|["'.]$/g, "").slice(0, 80);
			this.nameIfUnnamed(live.row.id, title || fallback);
		} catch {
			this.nameIfUnnamed(live.row.id, fallback);
		}
		this.rt.listChanged();
	}

	/** A made-up title, unless the person named the thread while it was being made. */
	private nameIfUnnamed(id: string, title: string): void {
		if (!this.rt.store.get(id)?.title) this.rt.store.update(id, { title });
	}
}
