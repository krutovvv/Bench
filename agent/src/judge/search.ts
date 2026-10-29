// The evaluator at a search: a long list of matches (grep, glob, a grep or find run through bash)
// is ordered by how likely each line is the place this turn's request needs, before the model
// reads it. The likeliest few come first with their probability, the rest follow in their own
// order: nothing is dropped, so a wrong guess costs a line of reading, not a missed match.
// Not sure enough, the result stays as the tool gave it.

import { cut, resultText } from "../turn/turn.ts";
import { evaluatorChars } from "./client.ts";
import type { Point, Seen } from "./point.ts";

/** Fewer lines than this are read as they are. */
const MIN_LINES = 12;
/** The likeliest lines put first. */
const SHOW = 8;
/** A line's least probability to be put first; a top line below `SURE` leaves the order alone. */
const FLOOR = 0.03;
const SURE = 0.15;
/** The most of one line shown to the evaluator. */
const LINE = 200;

const MATCH = /^[^\s:][^:\n]*:\d+:/;

/** The list a call returned, when it is one worth ordering: the match lines and the rest. */
function listOf(call: { name: string; arguments?: any }, text: string): { items: string[]; rest: string[] } | undefined {
	const lines = text.split("\n");
	let isItem: (l: string) => boolean;
	if (call.name === "grep") {
		// With context lines around each match the list is blocks, not lines: left as it is.
		if (Number(call.arguments?.context) > 0) return undefined;
		isItem = (l) => MATCH.test(l);
	} else if (call.name === "glob") {
		isItem = (l) => !!l.trim() && !l.startsWith("…") && !l.startsWith("No ");
	} else if (call.name === "bash" || call.name === "ssh_run") {
		const cmd = String(call.arguments?.command ?? "");
		if (/\b(rg|grep)\b/.test(cmd)) isItem = (l) => MATCH.test(l);
		else if (/\bfind\b/.test(cmd) && !/\|/.test(cmd)) isItem = (l) => /^\.?\.?\/?[\w.@-][^\s]*$/.test(l.trim());
		else return undefined;
	} else return undefined;
	const items = lines.filter(isItem);
	return items.length >= MIN_LINES ? { items, rest: lines.filter((l) => !isItem(l) && l.trim()) } : undefined;
}

const listOfSeen = (seen: Seen) => (seen.call && !seen.result?.isError ? listOf(seen.call, resultText(seen.result)) : undefined);

export const search: Point = {
	name: "search",
	moment: "result",
	ask(seen) {
		const list = listOfSeen(seen);
		if (!list) return undefined;
		// As many lines as fit their share of the evaluator's window (a quarter), in their order.
		const options: Record<string, string> = {};
		let room = evaluatorChars(0.25);
		for (const [i, line] of list.items.entries()) {
			const shown = cut(line, LINE);
			if ((room -= shown.length + 12) < 0 || i >= 250) break;
			options[`${i + 1}`] = shown;
		}
		return {
			state: { search: `${seen.call!.name} ${cut(JSON.stringify(seen.call!.arguments ?? {}), 300)}` },
			questions: {
				place: {
					type: "choice",
					instructions:
						"An agent working on the person's request ran this search. Which line of its results is the place the agent most likely needs to read or change next for that request?",
					criteria: options,
				},
			},
		};
	},
	apply(seen, answers) {
		const list = listOfSeen(seen);
		if (!list) return undefined;
		const probs = Object.entries(answers.place?.probabilities ?? {})
			.map(([k, p]) => [Number(k) - 1, p] as [number, number])
			.filter(([i]) => i >= 0 && i < list.items.length)
			.sort((a, b) => b[1] - a[1]);
		if (!probs.length || probs[0][1] < SURE) return undefined;
		const first = probs.filter(([, p]) => p >= FLOOR).slice(0, SHOW);
		const chosen = new Set(first.map(([i]) => i));
		const others = list.items.filter((_, i) => !chosen.has(i));
		const text = [
			`[Evaluator: the likeliest places for the request first]`,
			...first.map(([i, p]) => `${list.items[i]}    (${Math.round(p * 100)}%)`),
			``,
			`The other ${others.length}, in their order:`,
			...others,
			...list.rest,
		].join("\n");
		const r = seen.result;
		return { result: { ...r, output: [{ type: "text", text }, ...(r.output ?? []).filter((b: any) => b?.type !== "text")] } };
	},
};
