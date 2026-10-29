// The evaluator at a list a tool returned as JSON — a shop's search (Ozon, Avito), a web search
// (Tavily), any MCP server's results: its items are ordered by how useful each is for the request,
// the likeliest first. Every item stays; the model reads the best before the rest. As `search.ts`
// does for grep, for the lists that are not lines.

import { cut, resultText } from "../builtin/turn.ts";
import type { Point, Seen } from "./point.ts";

/** Fewer items than this are read as they are. */
const MIN_ITEMS = 8;
/** The most items asked about, and how much of each the evaluator reads. */
const MAX_ITEMS = 30;
const ITEM = 300;
/** An item put first when the choice gives it at least this, and many times what items usually get. */
const NEEDED = 0.05;
const STANDS_OUT = 3;

/** The result's list: the parsed JSON, and where its array is (the top, or a field of it). */
function listOf(seen: Seen): { json: any; field?: string; items: any[] } | undefined {
	if (!seen.call || seen.result?.isError || ["grep", "glob", "bash", "ssh_run", "read", "todo"].includes(seen.call.name)) return undefined;
	let json: any;
	try {
		json = JSON.parse(resultText(seen.result));
	} catch {
		return undefined;
	}
	if (Array.isArray(json)) return json.length >= MIN_ITEMS ? { json, items: json } : undefined;
	if (json && typeof json === "object")
		for (const [k, v] of Object.entries(json)) if (Array.isArray(v) && v.length >= MIN_ITEMS && v.every((x) => x && typeof x === "object")) return { json, field: k, items: v };
	return undefined;
}

export const listing: Point = {
	name: "listing",
	moment: "result",
	ask(seen) {
		const list = listOf(seen);
		if (!list) return undefined;
		const shown = list.items.slice(0, MAX_ITEMS);
		return {
			// The items in the state, short names as the options: the state is read whole.
			state: { list: Object.fromEntries(shown.map((x, i) => [`i${i}`, cut(JSON.stringify(x), ITEM)])) },
			questions: {
				item: {
					type: "choice",
					instructions: "Which item of the list a tool returned (in the state, under list) is the most useful for the person's request? none, if none is.",
					criteria: { ...Object.fromEntries(shown.map((_, i) => [`i${i}`, `Item i${i} of list`])), none: "None of the items is useful for the request." },
				},
			},
		};
	},
	apply(seen, answers) {
		const list = listOf(seen);
		if (!list) return undefined;
		const probs = answers.item?.probabilities ?? {};
		const got = list.items.map((_, i) => Number(probs[`i${i}`] ?? 0));
		const usual = Math.max([...got].sort((a, b) => a - b)[Math.floor(got.length / 2)] ?? 0, 0.005);
		const first = got.map((p, i) => [p, i] as const).filter(([p]) => p >= NEEDED && p >= usual * STANDS_OUT).sort((a, b) => b[0] - a[0]).map(([, i]) => i);
		if (!first.length) return undefined;
		const order = [...first, ...list.items.map((_, i) => i).filter((i) => !first.includes(i))];
		const items = order.map((i) => list.items[i]);
		const json = list.field ? { ...list.json, [list.field]: items } : items;
		const text = `[Evaluator: the list ordered by use for the request: the first ${first.length} likeliest, then the rest in their order]\n${JSON.stringify(json)}`;
		const r = seen.result;
		return { result: { ...r, output: [{ type: "text", text }, ...(r.output ?? []).filter((b: any) => b?.type !== "text")] } };
	},
};
