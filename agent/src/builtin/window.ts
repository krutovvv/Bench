// What is sent to the model fits its window. A local model has a hard limit (131K tokens here) and
// a tool's result has none: one browser capture of 52K characters, two of them, and the request
// was refused ("Prompt length 134672 exceeds the available context size"), the run stopped. As
// pictures are shrunk to the model's limits (`images` in `extensions/assistant.ts`), text is too:
//
// - **A result** (`afterTool`) larger than a share of the window keeps its head and tail, the
//   middle cut out with a line that says how much and how to ask for less.
// - **A request** (`beforeRequest`) that would not fit — the history, plus what the provider counted
//   last time beyond what was sent (the system prompt, the tools' descriptions) — has its largest
//   tool results and call arguments cut in this request only, oldest first, until it fits with room
//   for the answer. The journal keeps them whole; Pi's compaction still does the lasting work.
//
// Tokens are estimated (`CHARS_PER_TOKEN`, cautious for Russian and code) and the estimate is
// corrected by the provider's own count after every answer.

import { resolveModel } from "../config.ts";
import { excerpt } from "../judge/excerpt.ts";
import { neededness } from "../judge/keep.ts";

/** Characters per token, on the safe side: Russian text runs near 3, code near 3.5. */
export const CHARS_PER_TOKEN = 3;
/** The most of the window one tool result may take. */
const RESULT_SHARE = 0.12;
/** The window kept free for the model's answer (and its thinking). */
const ANSWER_SHARE = 0.12;
/** What a block cut in a request keeps, at its head and tail together. */
const SHRUNK = 2000;
/** Blocks smaller than this are never cut in a request. */
const WORTH = 4000;

export const tokensOf = (chars: number): number => Math.ceil(chars / CHARS_PER_TOKEN);

/** The model's window, in tokens. */
export const windowTokens = (): number => resolveModel().contextLength;

/** `text` cut to `keep` characters: two thirds from its head, a third from its tail, and why. */
export function cutMiddle(text: string, keep: number, why: string): string {
	if (text.length <= keep) return text;
	const head = Math.floor((keep * 2) / 3);
	const tail = keep - head;
	return `${text.slice(0, head)}\n\n[… ${text.length - keep} of ${text.length} characters cut out here: ${why}]\n\n${text.slice(text.length - tail)}`;
}

/** A tool's result that fits its share of the window (`afterTool`); undefined: it already does. */
export function fitResult(result: any, window = windowTokens()): any {
	const budget = Math.floor(window * RESULT_SHARE * CHARS_PER_TOKEN);
	const blocks: any[] = result?.output ?? [];
	const total = blocks.reduce((n, b) => n + (b?.type === "text" ? String(b.text ?? "").length : 0), 0);
	if (total <= budget) return undefined;
	const why =
		`the result is larger than this model takes in one step (~${tokensOf(total)} tokens; its window is ${window}). ` +
		`Ask for less: a narrower query or selector, a page or range, grep for what you need.`;
	// Each text block keeps its share of the budget.
	const output = blocks.map((b) => (b?.type === "text" ? { ...b, text: cutMiddle(String(b.text ?? ""), Math.max(500, Math.floor((budget * String(b.text ?? "").length) / total)), why) } : b));
	return { ...result, output };
}

/**
 * As `fitResult`, but with the evaluator on, the parts the request needs instead of head and tail
 * (`judge/excerpt.ts`); its answer missing, the cut as before.
 */
export async function fitResultFor(conversation: string, result: any, signal?: AbortSignal, window = windowTokens()): Promise<any> {
	const budget = Math.floor(window * RESULT_SHARE * CHARS_PER_TOKEN);
	const blocks: any[] = result?.output ?? [];
	const text = blocks.filter((b) => b?.type === "text").map((b) => String(b.text ?? "")).join("\n");
	if (text.length <= budget) return undefined;
	const picked = await excerpt(conversation, text, budget, signal).catch(() => undefined);
	if (picked) return { ...result, output: [{ type: "text", text: picked }, ...blocks.filter((b) => b?.type !== "text")] };
	return fitResult(result, window);
}

/** What the provider counted beyond the estimate of what was sent, per conversation. */
const overhead = new Map<string, number>();
/** The estimate of the last request's messages, per conversation. */
const sent = new Map<string, number>();

/** The characters of a message's blocks that a request can carry. */
function charsOf(m: any): number {
	if (typeof m?.content === "string") return m.content.length;
	return (m?.content ?? []).reduce((n: number, b: any) => {
		if (b?.type === "text") return n + String(b.text ?? "").length;
		if (b?.type === "thinking") return n + String(b.thinking ?? "").length;
		if (b?.type === "toolCall") return n + JSON.stringify(b.arguments ?? {}).length + 50;
		return n + 1000;
	}, 0);
}

/** How many tokens the conversation's last request held, as estimated and corrected by the provider's count. */
export function usedTokens(conversation: string): number {
	return (sent.get(conversation) ?? 0) + (overhead.get(conversation) ?? 0);
}

/** The provider's count after an answer (`afterResponse`): what the estimate missed. */
export function seeUsage(conversation: string, usage: any): void {
	const counted = Number(usage?.input ?? 0) + Number(usage?.cacheRead ?? 0) + Number(usage?.cacheWrite ?? 0);
	const estimate = sent.get(conversation);
	if (!counted || estimate === undefined) return;
	overhead.set(conversation, Math.max(0, counted - estimate));
}

/**
 * The request's messages cut to fit the window with room for the answer (`beforeRequest`):
 * the largest tool results and call arguments first, oldest first among equals, the last message
 * never. Undefined: they fit as they are.
 */
export function fitRequest(conversation: string, messages: readonly any[], window = windowTokens(), weighed?: [number, number, number, number][]): any[] | undefined {
	const room = Math.floor(window * (1 - ANSWER_SHARE)) - (overhead.get(conversation) ?? 0);
	let used = tokensOf(messages.reduce((n, m) => n + charsOf(m), 0));
	sent.set(conversation, used);
	if (used <= room) return undefined;
	// Every block worth cutting, the least needed first when the evaluator weighed them, else the
	// largest first, then the oldest.
	const cuts: [number, number, number, number][] = weighed ?? candidates(messages).map((c) => [...c, 0] as [number, number, number, number]);
	cuts.sort((a, b) => a[3] - b[3] || b[2] - a[2] || a[0] - b[0]);
	const out = messages.map((m) => (Array.isArray(m?.content) ? { ...m, content: [...m.content] } : m));
	const why = "kept whole in the thread, cut here so the conversation fits the model's window";
	for (const [i, j, n] of cuts) {
		if (used <= room) break;
		const b = out[i].content[j];
		if (b.type === "text") out[i].content[j] = { ...b, text: cutMiddle(String(b.text), SHRUNK, why) };
		else out[i].content[j] = { ...b, arguments: shrinkArgs(b.arguments) };
		used -= tokensOf(n - SHRUNK);
	}
	sent.set(conversation, used);
	return out;
}

/**
 * As `fitRequest`, but with the evaluator on, the blocks the next step needs least are cut first
 * (`judge/keep.ts`); the largest first only among equals.
 */
export async function fitRequestFor(conversation: string, messages: readonly any[], signal?: AbortSignal, window = windowTokens()): Promise<any[] | undefined> {
	const room = Math.floor(window * (1 - ANSWER_SHARE)) - (overhead.get(conversation) ?? 0);
	const total = tokensOf(messages.reduce((n, m) => n + charsOf(m), 0));
	if (total <= room) return fitRequest(conversation, messages, window);
	const blocks = candidates(messages);
	const texts = blocks.map(([i, j]) => {
		const b = messages[i].content[j];
		return b.type === "text" ? String(b.text) : JSON.stringify(b.arguments ?? {});
	});
	const need = await neededness(conversation, texts, signal).catch(() => undefined);
	return fitRequest(conversation, messages, window, need ? blocks.map((b, k) => [...b, need[k]] as [number, number, number, number]) : undefined);
}

/** Every block worth cutting: [message, block, characters], the last message never. */
function candidates(messages: readonly any[]): [number, number, number][] {
	const cuts: [number, number, number][] = [];
	messages.forEach((m, i) => {
		if (i === messages.length - 1 || !Array.isArray(m?.content)) return;
		m.content.forEach((b: any, j: number) => {
			const n = b?.type === "text" && m.role === "toolResult" ? String(b.text ?? "").length : b?.type === "toolCall" ? JSON.stringify(b.arguments ?? {}).length : 0;
			if (n > WORTH) cuts.push([i, j, n]);
		});
	});
	return cuts;
}

/** A call's arguments with its long strings cut (a file written whole, a long script). */
function shrinkArgs(args: any): any {
	if (typeof args === "string") return args.length > SHRUNK ? cutMiddle(args, SHRUNK, "the call's text, cut in this request") : args;
	if (Array.isArray(args)) return args.map(shrinkArgs);
	if (args && typeof args === "object") return Object.fromEntries(Object.entries(args).map(([k, v]) => [k, shrinkArgs(v)]));
	return args;
}
