// A long thread's compaction without reasoning: summing up what was said needs no thinking, and on
// a server that thinks by default it only costs time. Pi Durable summarizes with the thread's own
// model and reasoning level and has no setting for the summarizer's, so this uses its hook for
// exactly that (`beforeCompact`: an extension supplies the summary): the same model, the same
// format as Pi's own summarizer, reasoning off (`noThinking`). `SYSTEM`, `FORMAT` and `transcript`
// mirror pi-durable 1.4.0's `SUMMARIZATION_SYSTEM_PROMPT`, `SUMMARIZATION_PROMPT` and
// `serializeConversation` (`harness/compaction.js`; Pi does not export them), with one line of ours
// (the summary in the conversation's language): read them again when Pi is updated. The summary is memoized, so a
// restart places the one already made; any failure leaves the compaction to Pi, as before. With the
// evaluator on, what the rest of the work needs as it is follows the summary (`judge/summary.ts`).

import { CompactionTask, DEFAULT_COMPACTION_POLICY, AgentDoc, hook } from "@earendil-works/pi-durable";
import { noThinking } from "../runtime/thinking.ts";
import { afterSummary } from "../judge/summary.ts";

const SYSTEM = `You are a context summarization assistant. Your task is to read a conversation between a user and an AI assistant, then produce a structured summary following the exact format specified.

Do NOT continue the conversation. Do NOT respond to any questions in the conversation. ONLY output the structured summary.`;

const FORMAT = `The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work. If the conversation starts with an earlier summary, preserve its information and fold the newer messages into it.

Use this EXACT format:

## Goal
[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned by user]
- [Or "(none)" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Current work]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [Ordered list of what should happen next]

## Critical Context
- [Any data, examples, or references needed to continue]
- [Or "(none)" if not applicable]

Keep each section concise. Preserve exact file paths, function names, and error messages. Write in the language of the conversation.`;

const text = (content: any): string =>
	typeof content === "string" ? content : (content ?? []).flatMap((b: any) => (b.type === "text" && b.text !== undefined ? [b.text] : [])).join("\n");
const clip = (s: string, n: number) => (s.length <= n ? s : `${s.slice(0, n)}\n\n[... ${s.length - n} more characters truncated]`);

/** The messages as a transcript (no thinking: it can be larger than the context it sums up). */
export function transcript(messages: readonly any[]): string {
	const parts: string[] = [];
	for (const m of messages) {
		if (m.role === "user") {
			const t = text(m.content);
			if (t) parts.push(`[User]: ${t}`);
		} else if (m.role === "assistant") {
			const t = (m.content ?? []).flatMap((c: any) => (c.type === "text" ? [c.text] : []));
			const calls = (m.content ?? []).flatMap((c: any) =>
				c.type === "toolCall" ? [`${c.name}(${Object.entries(c.arguments ?? {}).map(([k, v]) => `${k}=${JSON.stringify(v)}`).join(", ")})`] : [],
			);
			if (t.length) parts.push(`[Assistant]: ${t.join("\n")}`);
			if (calls.length) parts.push(`[Assistant tool calls]: ${calls.join("; ")}`);
		} else if (m.role === "toolResult") {
			const t = text(m.content);
			if (t) parts.push(`[Tool result]: ${clip(t, 2000)}`);
		}
	}
	return parts.join("\n\n");
}

export const compactWithoutReasoning = hook(CompactionTask, {
	beforeCompact: async (compaction: any, api: any, context: any) => {
		const done = await api.memo("bench.summary", context);
		if (typeof done === "string" && done) return { summary: done };
		try {
			const ref = ((await api.snapshot(AgentDoc, api.conversationId, context)) as any)?.model;
			const model = ref && api.models.getModel(ref.provider, ref.modelId);
			if (!model) return undefined;
			const focus = compaction.instructions ? `\n\nAdditional focus: ${compaction.instructions}` : "";
			const res: any = await api.models.complete(
				model,
				{ systemPrompt: SYSTEM, messages: [{ role: "user", content: `<conversation>\n${transcript(compaction.messages)}\n</conversation>\n\n${FORMAT}${focus}`, timestamp: Date.now() }] },
				{ reasoningEffort: undefined, maxTokens: Math.min(Math.floor(0.8 * DEFAULT_COMPACTION_POLICY.reserveTokens), model.maxTokens || Infinity), cacheRetention: "none", onPayload: noThinking },
			);
			const summary = (res?.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("").trim();
			if (res?.stopReason !== "stop" || !summary) return undefined;
			// What a summary would lose that the work still needs — the person's words in force, the
			// files in work, exact results — follows it as it is, chosen by the evaluator
			// (`judge/summary.ts`); nothing without it.
			const kept = await afterSummary(String(api.conversationId), compaction.messages, context?.abortSignal).catch(() => "");
			return { summary: await api.memo("bench.summary", summary + kept, context) };
		} catch {
			return undefined;
		}
	},
} as any);
