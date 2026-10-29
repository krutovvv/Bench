// Edits by line anchors ("hashline"): `read` shows every line as `LINE:HASH|text`, and `edit` names
// the lines it changes by those anchors instead of repeating their old text. A model then never
// has to reproduce code exactly to change it — the step weak and fast models fail most (an
// `oldText` with one space off) — and an anchor whose line changed since it was read is refused
// with the lines as they are now, never applied to the wrong place.
//
// `read` is Pi's, wrapped: its text output is the file's lines from `offset`, with its notes as
// diagnostics, so the anchors are added line by line. `edit` keeps Pi's own replacement for
// text the anchors cannot name (`oldText`/`newText`): such a call is Pi's edit unchanged.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { createCodingTools, createEditTool, createReadTool, type ReadToolOptions } from "@earendil-works/pi-durable/tools";
import * as Diff from "diff";

/** A line's hash: two base-36 characters of its FNV-1a, trailing `\r` aside. */
export function lineHash(line: string): string {
	let h = 0x811c9dc5;
	const s = line.endsWith("\r") ? line.slice(0, -1) : line;
	for (let k = 0; k < s.length; k++) {
		h ^= s.charCodeAt(k);
		h = Math.imul(h, 0x01000193);
	}
	return ((h >>> 0) % 1296).toString(36).padStart(2, "0");
}

/** `lines` as `read` shows them, the first being line `first` (1-based). */
export function anchored(lines: string[], first: number): string {
	return lines.map((l, k) => `${first + k}:${lineHash(l)}|${l.endsWith("\r") ? l.slice(0, -1) : l}`).join("\n");
}

/** An anchor's line, checked against `lines` (0-based index), or why it does not hold. */
function lineOf(anchor: string, lines: string[]): number | string {
	const m = /^\s*(\d+)\s*:\s*([0-9a-z]{2})\s*$/i.exec(anchor);
	if (!m) return `"${anchor}" is not an anchor: write it as read shows it, LINE:HASH (12:a3)`;
	const n = Number(m[1]);
	if (n < 1 || n > lines.length) return `line ${n} is past the end of the file (${lines.length} lines)`;
	if (lineHash(lines[n - 1]) !== m[2].toLowerCase()) return `line ${n} is not what you read (${anchor}): the file changed there`;
	return n - 1;
}

export interface AnchoredEdit {
	/** "replace" (default): lines start..end become `content`; "" deletes them. "insert_after",
	 * "insert_before": `content` goes after or before line `start`; "0" inserts at the top. */
	op?: "replace" | "insert_after" | "insert_before";
	start: string;
	end?: string;
	content?: string;
}

/** Content lines as the model wrote them: one trailing newline dropped, anchors it copied from
 * `read` (every line carries one) taken off. */
function contentLines(content: string | undefined): string[] {
	if (!content) return [];
	const lines = content.replace(/\n$/, "").split("\n");
	const copied = /^\d+:[0-9a-z]{2}\|/;
	return lines.every((l) => l === "" || copied.test(l)) && lines.some((l) => copied.test(l)) ? lines.map((l) => l.replace(copied, "")) : lines;
}

/**
 * `lines` with every edit applied, each against the lines as they were (not one after another),
 * and the new lines' ranges (0-based, in the result). Throws with every anchor that does not
 * hold and the lines around it as they are now.
 */
export function applyAnchored(lines: string[], edits: AnchoredEdit[]): { lines: string[]; changed: [number, number][] } {
	const wrong: string[] = [];
	const spans: { from: number; to: number; put: string[] }[] = [];
	for (const e of edits) {
		const op = e.op ?? "replace";
		if (op === "insert_after" && /^\s*0\s*(:.*)?$/.test(e.start)) {
			spans.push({ from: 0, to: 0, put: contentLines(e.content) });
			continue;
		}
		const a = lineOf(e.start, lines);
		const b = op === "replace" && e.end ? lineOf(e.end, lines) : a;
		for (const x of [a, b]) if (typeof x === "string") wrong.push(x);
		if (typeof a === "string" || typeof b === "string") continue;
		if (b < a) {
			wrong.push(`end ${e.end} is before start ${e.start}`);
			continue;
		}
		// A replace without content is a slip (a deletion says content: ""), never a silent delete.
		if (e.content === undefined) {
			wrong.push(`the edit at ${e.start} has no content (to delete lines, give content: "")`);
			continue;
		}
		const put = contentLines(e.content);
		if (op === "replace") spans.push({ from: a, to: b + 1, put });
		else if (op === "insert_after") spans.push({ from: a + 1, to: a + 1, put });
		else spans.push({ from: a, to: a, put });
	}
	if (wrong.length) {
		const near = new Set<number>();
		for (const e of edits)
			for (const s of [e.start, e.end]) {
				const n = Number(/^\s*(\d+)/.exec(s ?? "")?.[1]);
				if (n) for (let k = Math.max(1, n - 2); k <= Math.min(lines.length, n + 2); k++) near.add(k);
			}
		const now = [...near].sort((x, y) => x - y).map((n) => anchored([lines[n - 1]], n));
		throw new Error(`Nothing was changed: ${wrong.join("; ")}.${now.length ? `\nThose lines now:\n${now.join("\n")}` : ""}\nRead the lines again, or use these anchors.`);
	}
	spans.sort((x, y) => x.from - y.from || x.to - y.to);
	// Inserts at one place keep their order; anything else sharing lines is ambiguous.
	for (let k = 1; k < spans.length; k++)
		if (spans[k].from < spans[k - 1].to) throw new Error("Nothing was changed: two edits touch the same lines; merge them into one.");
	const out = [...lines];
	const changed: [number, number][] = [];
	// From the bottom up, so each span's place is still the original one; then the new ranges.
	for (const s of [...spans].reverse()) out.splice(s.from, s.to - s.from, ...s.put);
	let shift = 0;
	for (const s of spans) {
		changed.push([s.from + shift, s.from + shift + s.put.length]);
		shift += s.put.length - (s.to - s.from);
	}
	return { lines: out, changed };
}

/** Pi's read, its text lines anchored. */
export function createAnchoredRead(options: ReadToolOptions): any {
	const read: any = createReadTool(options);
	return {
		...read,
		description: `${read.description} Every text line comes as LINE:HASH|text: the LINE:HASH is the anchor edit names the line by; it is not part of the file.`,
		async execute(args: any, api: any, context: any) {
			const r = await read.execute(args, api, context);
			// One long line cut to a part: its hash would not be the line's, so it has none.
			const cut = (r.diagnostics ?? []).some((d: any) => d.code === "truncated" && /exceeds/.test(d.message ?? ""));
			if (cut) return r;
			const first = Math.max(1, Math.floor(Number(args.offset) || 1));
			return { ...r, output: (r.output ?? []).map((o: any) => (o.type === "text" ? { ...o, text: anchored(o.text.split("\n"), first) } : o)) };
		},
	};
}

/** The edits of one file run one after another (Pi's edit has its own queue, not exported). */
const queues = new Map<string, Promise<unknown>>();
function queued<T>(file: string, fn: () => Promise<T>): Promise<T> {
	const run = (queues.get(file) ?? Promise.resolve()).catch(() => {}).then(fn);
	queues.set(file, run);
	void run.finally(() => queues.get(file) === run && queues.delete(file)).catch(() => {});
	return run;
}

const get = <T>(r: { ok: true; value: T } | { ok: false; error: any }): T => {
	if (!r.ok) throw new Error(String(r.error?.message ?? r.error?.code ?? r.error));
	return r.value;
};

const anchoredEdit = Type.Object({
	op: Type.Optional(Type.Union([Type.Literal("replace"), Type.Literal("insert_after"), Type.Literal("insert_before")], { description: "replace (default), insert_after, insert_before" })),
	start: Type.Optional(Type.String({ description: "The first line's anchor as read shows it: 12:a3. \"0\" with insert_after: the top of the file" })),
	end: Type.Optional(Type.String({ description: "replace: the last line's anchor (default: start)" })),
	content: Type.Optional(Type.String({ description: "The new lines, without anchors; \"\" with replace deletes the lines" })),
	oldText: Type.Optional(Type.String({ description: "Instead of anchors: exact text to replace, unique in the file" })),
	newText: Type.Optional(Type.String({ description: "With oldText: its replacement" })),
});

export function createAnchoredEdit(): any {
	const pi: any = createEditTool();
	return defineTool({
		name: "edit",
		description:
			"Edit a file by the anchors read showed (LINE:HASH): replace lines start..end with content, or insert content after or before a line. " +
			"Anchors are those of the file before this call; several edits in one call must not touch the same lines. " +
			"The result shows the changed lines with their new anchors, so a next edit needs no new read. " +
			"For text with no anchors at hand, an edit may give oldText and newText instead (exact, unique text).",
		parameters: Type.Object({ path: Type.String({ description: "The file (relative or absolute)" }), edits: Type.Array(anchoredEdit) }),
		prepareArguments: (input: any) => {
			if (!input || typeof input !== "object") return input;
			const args = { ...input };
			if (typeof args.edits === "string")
				try {
					args.edits = JSON.parse(args.edits);
				} catch {}
			if (args.edits && !Array.isArray(args.edits)) args.edits = [args.edits];
			if (!args.edits && (args.start || args.oldText)) {
				const { path: p, edits: _, ...one } = args;
				return { path: p, edits: [one] };
			}
			return args;
		},
		async execute(args: any, api: any, context: any) {
			const edits: any[] = args.edits ?? [];
			if (!edits.length) throw new Error("edits must name at least one change");
			// newText beside anchors is the content, as models write it.
			for (const e of edits) if (e.start && e.content === undefined && typeof e.newText === "string" && e.oldText === undefined) e.content = e.newText;
			const byText = edits.filter((e) => typeof e.oldText === "string");
			if (byText.length === edits.length) return pi.execute({ path: args.path, edits: byText.map((e) => ({ oldText: e.oldText, newText: e.newText ?? "" })) }, api, context);
			if (byText.length) throw new Error("Give each edit anchors or oldText, the same way in one call.");
			if (edits.some((e) => !e.start)) throw new Error("each edit needs start: the anchor of its line as read shows it (12:a3)");
			const env = api.env;
			if (!env) throw new Error("this thread has no files to edit");
			const file = get(await env.absolutePath(String(args.path).replace(/^@/, ""), context)) as string;
			return queued(file, async () => {
				const raw = get(await env.readTextFile(file, context)) as string;
				const bom = raw.startsWith("﻿") ? "﻿" : "";
				const body = bom ? raw.slice(1) : raw;
				const crlf = body.includes("\r\n");
				const before = crlf ? body.replace(/\r\n/g, "\n") : body;
				const { lines, changed } = applyAnchored(before.split("\n"), edits);
				const after = lines.join("\n");
				get(await env.writeFile(file, bom + (crlf ? after.replace(/\n/g, "\r\n") : after), context));
				// The changed lines and two around each, with their new anchors.
				const shown = new Set<number>();
				for (const [a, b] of changed) for (let k = Math.max(0, a - 2); k < Math.min(lines.length, Math.max(b, a + 1) + 2); k++) shown.add(k);
				const order = [...shown].sort((x, y) => x - y);
				const view: string[] = [];
				for (let k = 0; k < order.length && view.length < 80; k++) {
					if (k && order[k] !== order[k - 1] + 1) view.push("…");
					view.push(anchored([lines[order[k]]], order[k] + 1));
				}
				const patch = Diff.createTwoFilesPatch(args.path, args.path, before, after, undefined, undefined, { context: 4, headerOptions: Diff.FILE_HEADERS_ONLY });
				return {
					output: [{ type: "text", text: `Edited ${args.path} (${edits.length} ${edits.length === 1 ? "change" : "changes"}). Now:\n${view.join("\n")}` }],
					details: { diff: "", patch, ...(changed.length ? { firstChangedLine: changed[0][0] + 1 } : {}) },
				};
			});
		},
	} as any);
}

/** Pi's coding tools (read, write, edit, bash) with read and edit by line anchors: what a thread
 *  is given (`extensions/assistant.ts`) and what a person's own extension builds from (`extensions.ts`). */
export function codingTools(options: ReadToolOptions): any[] {
	return (createCodingTools(options as any).tools ?? []).map((t: any) => (t.name === "read" ? createAnchoredRead(options) : t.name === "edit" ? createAnchoredEdit() : t));
}
