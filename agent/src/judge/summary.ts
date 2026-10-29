// The evaluator at compaction (`builtin/compaction.ts`): what a summary of a few thousand
// characters loses that the rest of the work needs, put after it as it is.
//
// In a thread compacted 25 times, 38% of the files the agent touched in the 20 steps after a
// compaction were ones it had touched in the 80 before: it read again what it had just known. And
// the person's corrections ("not in this project", "leave the memory alone") are what a summary
// blurs first. So three things follow the summary, chosen by the evaluator, in a share of the
// window: the person's words still in force, word for word; the files in work, as they are now on
// disk; the results the work needs exactly (`keep.ts`).

import { cut } from "../builtin/turn.ts";
import { windowTokens } from "../builtin/window.ts";
import { EVAL_OPEN, stripBlock } from "../runtime/journal.ts";
import { envFor } from "../runtime/tools-host.ts";
import { host } from "../host.ts";
import { ask, judgeActive } from "./client.ts";
import { baseState } from "./point.ts";
import { turnOf } from "../builtin/turn.ts";
import { keptThrough } from "./keep.ts";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";

/** The window's share kept after a summary (a token taken as 3 characters): the summary itself is ~1 000 tokens. */
const SHARE = 0.12;
/** The person's latest messages weighed, how much of each is read, how sure "in force" must be. */
const WORDS = 20;
const WORD = 600;
const SURE_WORDS = 0.6;
/** The files weighed (the latest touched), how sure "in work" must be, the most of one kept. */
const FILES = 12;
const SURE_FILE = 0.7;
const FILE_MAX = 12_000;

const textOf = (c: any): string => (typeof c === "string" ? c : (c ?? []).filter((b: any) => b?.type === "text").map((b: any) => String(b.text ?? "")).join("\n"));

/** The person's own messages of the part being summed up: not the evaluator's, not an earlier summary. */
export function personsWords(messages: readonly any[]): string[] {
	return messages
		.filter((m) => m?.role === "user")
		.map((m) => stripBlock(textOf(m.content)).trim())
		.filter((t) => t && !t.startsWith(EVAL_OPEN) && !t.startsWith("The conversation history before this point was compacted"));
}

/** The files the calls read, wrote or edited, the latest touched first. */
export function touchedFiles(messages: readonly any[]): string[] {
	const order: string[] = [];
	for (const m of messages)
		if (m?.role === "assistant")
			for (const c of m.content ?? []) {
				const p = c?.type === "toolCall" && ["read", "write", "edit"].includes(c.name) ? c.arguments?.path : undefined;
				if (typeof p === "string" && p) {
					const at = order.indexOf(p);
					if (at >= 0) order.splice(at, 1);
					order.push(p);
				}
			}
	return order.reverse();
}

/** What follows the summary; "" with no evaluator or nothing chosen. */
export async function afterSummary(conversation: string, messages: readonly any[], signal?: AbortSignal): Promise<string> {
	if (!judgeActive(conversation)) return "";
	let room = Math.floor(windowTokens() * SHARE * 3);
	const parts: string[] = [];

	const words = personsWords(messages).slice(-WORDS);
	const files = touchedFiles(messages).slice(0, FILES);
	if (words.length || files.length) {
		const answers = await ask(
			conversation,
			"summary",
			{
				...baseState(turnOf(conversation), 8),
				...(words.length ? { persons_words: Object.fromEntries(words.map((w, i) => [`w${i}`, cut(w, WORD)])) } : {}),
				...(files.length ? { files: Object.fromEntries(files.map((f, i) => [`f${i}`, f])) } : {}),
			},
			{
				...Object.fromEntries(
					words.map((_, i) => [
						`w${i}`,
						{
							type: "choice",
							instructions: `The conversation is being summed up to go on in a shorter window. Is the person's message w${i} (under persons_words) still in force for the rest of the work?`,
							// "past" first: the model leans to the first option; a word kept that is past costs little room.
							criteria: {
								past: "No: a question answered, a task done, or replaced by a later message.",
								in_force: "Yes: a constraint, a correction, a preference or a decision the rest of the work must keep (where to work, what not to touch, how it must be).",
							},
						},
					]),
				),
				...Object.fromEntries(
					files.map((_, i) => [
						`f${i}`,
						{
							type: "choice",
							instructions: `The conversation is being summed up to go on in a shorter window. Will the rest of the work read or change the file f${i} (under files) again?`,
							criteria: { done_with: "No: looked at or finished with.", in_work: "Yes: the work is on it, or its next steps need its text." },
						},
					]),
				),
			},
			signal,
		);
		if (answers) {
			const inForce = words.filter((_, i) => (answers[`w${i}`]?.probabilities?.in_force ?? 0) >= SURE_WORDS);
			if (inForce.length) {
				const block = `\n\n## The person's words still in force\n${inForce.map((w) => `> ${cut(w, 2000).replace(/\n/g, "\n> ")}`).join("\n\n")}`;
				room -= block.length;
				parts.push(block);
			}
			const inWork = files.filter((_, i) => (answers[`f${i}`]?.probabilities?.in_work ?? 0) >= SURE_FILE);
			const shown = await filesNow(conversation, inWork, Math.floor(room * 0.6));
			if (shown) {
				room -= shown.length;
				parts.push(shown);
			}
		}
	}
	parts.push(await keptThrough(conversation, resultsOf(messages), signal, room));
	return parts.join("");
}

/** The files as they are now on the thread's machine, the latest touched first, within `room`. */
async function filesNow(conversation: string, files: string[], room: number): Promise<string> {
	if (!files.length || room <= 0) return "";
	const env = await envFor(conversation, host().thread(conversation)?.cwd).catch(() => undefined);
	if (!env) return "";
	const out: string[] = [];
	for (const f of files) {
		const r = await env.readTextFile(f, BACKGROUND_CONTEXT).catch(() => undefined);
		if (!r?.ok) continue;
		const block = `\n\n### ${f}\n\`\`\`\n${cut(r.value, FILE_MAX)}\n\`\`\``;
		if (block.length > room) continue;
		room -= block.length;
		out.push(block);
	}
	return out.length ? `\n\n## Files in work, as they are now${out.join("")}` : "";
}

/** Each tool result with the call that made it, a block each, in order. */
export function resultsOf(messages: readonly any[]): string[] {
	const calls = new Map<string, string>();
	const out: string[] = [];
	for (const m of messages) {
		if (m?.role === "assistant")
			for (const c of m.content ?? []) if (c?.type === "toolCall") calls.set(String(c.id), `${c.name} ${JSON.stringify(c.arguments ?? {}).slice(0, 200)}`);
		if (m?.role === "toolResult" && !m.isError) {
			const t = textOf(m.content);
			if (t) out.push(`[${calls.get(String(m.toolCallId)) ?? m.toolName ?? "result"}] →\n${t}`);
		}
	}
	return out;
}
