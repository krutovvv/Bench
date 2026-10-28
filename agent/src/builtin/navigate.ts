// `lsp`: the code as an editor knows it, from the project's language servers (its lsp.json,
// `lsp.ts`) — where a symbol is defined, where it is used, a file's outline, symbols by name, what
// a symbol is. One step where grep, sed and read would take several, and exact: a server knows a
// call from a comment and a field from a local of the same name.
//
// The model names a place the way it reads code — a file, a line, the word on it — never a column.

import fs from "node:fs";
import path from "node:path";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { host } from "../host.ts";
import { definitionAt, hoverAt, indexing, outlineOf, readLsp, referencesAt, symbolsNamed, type Place, type Spot, type Symbol } from "../lsp.ts";
import { machineEnv, quote } from "../ssh-env.ts";
import { projects } from "./projects.ts";

/** A file a tool named, where it is and what it holds now (on the project's machine, if any). */
export async function placeOf(info: { cwd: string; project?: string }, p: string): Promise<{ file: string; machine?: string; text: string } | undefined> {
	if (!p) return undefined;
	const machine = info.project ? (projects.get(info.project)?.machine ?? undefined) : undefined;
	if (machine) {
		const env = await machineEnv(machine, info.cwd);
		const file = path.posix.resolve(env.cwd, p);
		const r = await env.readTextFile(file, BACKGROUND_CONTEXT);
		return r.ok ? { file, machine, text: r.value } : undefined;
	}
	const file = path.resolve(info.cwd, p);
	try {
		return { file, text: fs.readFileSync(file, "utf8") };
	} catch {
		return undefined;
	}
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * The spot of `symbol` in `text`: on line `line` (1-based) when given, else its first whole-word
 * use in the file's code (a comment's word is not the symbol); without a symbol, the line's first
 * non-blank character. Undefined: not there.
 */
export function spotOf(text: string, line: number | undefined, symbol: string | undefined): Spot | undefined {
	const lines = text.split("\n");
	const word = symbol ? new RegExp(`(?<![A-Za-z0-9_$])${escape(symbol)}(?![A-Za-z0-9_$])`) : undefined;
	if (line !== undefined) {
		const l = lines[line - 1];
		if (l === undefined) return undefined;
		if (!word) return { line: line - 1, character: Math.max(0, l.search(/\S/)) };
		const at = l.search(word);
		return at < 0 ? undefined : { line: line - 1, character: at };
	}
	if (!word) return undefined;
	const comment = /^\s*(\/\/|\/\*|\*|#)/;
	for (let k = 0; k < lines.length; k++) {
		const at = comment.test(lines[k]) ? -1 : lines[k].search(word);
		if (at >= 0) return { line: k, character: at };
	}
	return undefined;
}

const MAX_REFERENCES = 60;
const MAX_SYMBOLS = 30;
/** How long one call waits for a server still loading the project before it says so. */
const WAIT_MS = 20_000;
/** How many files of its kind a server is shown for a search by name: without a project file
 * (tsconfig) tsserver searches only what the open files reach. */
const OPEN_FOR_SEARCH = 60;
/** Files a project's code starts from: shown first. */
const ENTRIES = /(^|\/)(index|main|server|app|lib|mod)\.[a-z]+$/;

/**
 * Ask again while a server is still loading the project and has nothing yet (rust-analyzer
 * answers nothing, or too late, until it has indexed a large workspace — minutes the first
 * time). Empty after the wait: `still` says which servers are still at it.
 */
async function settled<T>(profile: string, ask: () => Promise<T[]>): Promise<{ got: T[]; still: string }> {
	const end = Date.now() + WAIT_MS;
	for (;;) {
		const got = await ask();
		const busy = indexing(profile);
		if (got.length || !busy.length) return { got, still: "" };
		if (Date.now() > end)
			return { got, still: `${busy.map((b) => `${b.name} has been loading the project for ${b.secs} s`).join(", ")}: ask again in a minute, or use grep and read meanwhile.` };
		await new Promise((r) => setTimeout(r, 1500));
	}
}

export const lspTool = defineTool({
	name: "lsp",
	description:
		"The code as an editor knows it, from the project's language servers. Use it instead of grep or sed to find code: " +
		"`definition` and `references` of a symbol (give `path`, the `symbol`, and its `line` if it occurs more than once), " +
		"`hover` (its type and docs), `outline` (a file's functions, types and fields with their lines), " +
		"`symbols` (where things named `query` are defined, across the project). Lines are 1-based.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("definition"), Type.Literal("references"), Type.Literal("hover"), Type.Literal("outline"), Type.Literal("symbols")]),
		path: Type.Optional(Type.String({ description: "A file (all but symbols)" })),
		symbol: Type.Optional(Type.String({ description: "The name as written in the file (definition, references, hover)" })),
		line: Type.Optional(Type.Number({ description: "The 1-based line the symbol is on, when it occurs more than once" })),
		query: Type.Optional(Type.String({ description: "A name or part of one (symbols)" })),
	}),
	replay: "safe",
	async execute(args, api, context) {
		const info = host().session(api.conversationId);
		if (!info) throw new Error("no session");
		const profile = info.profile;
		if (!Object.keys(readLsp(profile)).length) throw new Error("this project has no language servers (its lsp.json, in Settings → Language servers); use grep and read");
		const machine = info.project ? (projects.get(info.project)?.machine ?? undefined) : undefined;
		const root = info.cwd;
		const rel = (p: string) => (p.startsWith(root + "/") ? p.slice(root.length + 1) : p);
		const text = (t: string) => ({ output: [{ type: "text" as const, text: t }] });

		if (args.action === "symbols") {
			const query = String(args.query ?? args.symbol ?? "").trim();
			if (!query) throw new Error("query is required");
			// Files of each server's kind for servers that know a project only from open files
			// (tsserver without a tsconfig): its entry files first, then the rest, up to a limit.
			const open = async (exts: string[]) => {
				const globs = exts.map((e) => quote(`*.${e}`)).join(" ");
				let out = "";
				const r = await api.env?.exec(`git ls-files -- ${globs} 2>/dev/null | head -2000`, { cwd: root, onOutput: (t: string) => (out += t) }, context);
				const all = r?.ok ? out.split("\n").filter(Boolean) : [];
				const picked = [...all.filter((f) => ENTRIES.test(f)), ...all].slice(0, OPEN_FOR_SEARCH);
				const files = await Promise.all([...new Set(picked)].map((f) => placeOf(info, f)));
				return files.filter((f) => f !== undefined);
			};
			const { got: found, still } = await settled(profile, () => symbolsNamed(profile, root, machine, query, open));
			if (!found.length) return text(still || `No symbol named like "${query}".`);
			// The exact name first, then those that start with it, then the rest the server matched.
			const q = query.toLowerCase();
			const rank = (s: Symbol) => (s.name.toLowerCase() === q ? 0 : s.name.toLowerCase().startsWith(q) ? 1 : 2);
			const sorted = [...found].sort((a, b) => rank(a) - rank(b));
			const lines = sorted.slice(0, MAX_SYMBOLS).map((s) => `${rel(s.path)}:${s.line}  ${s.kind} ${s.name}${s.container ? `  (in ${s.container})` : ""}`);
			return text(lines.join("\n") + (sorted.length > MAX_SYMBOLS ? `\n… ${sorted.length - MAX_SYMBOLS} more; a fuller name narrows it` : ""));
		}

		const at = await placeOf(info, String(args.path ?? ""));
		if (!at) throw new Error(args.path ? `cannot read ${args.path}` : "path is required");

		if (args.action === "outline") {
			const { got: list, still } = await settled(profile, () => outlineOf(profile, root, machine, at.file, at.text));
			if (!list.length) return text(still || `No outline for ${rel(at.file)}: no language server of this project knows it, or it has no symbols.`);
			return text(list.map((s) => `${"  ".repeat(s.depth ?? 0)}${s.line}  ${s.kind} ${s.name}`).join("\n"));
		}

		const spot = spotOf(at.text, args.line, args.symbol);
		if (!spot) throw new Error(args.symbol ? `"${args.symbol}" is not ${args.line ? `on line ${args.line} of` : "in"} ${rel(at.file)}` : "symbol or line is required");

		if (args.action === "hover") {
			const md = await hoverAt(profile, root, machine, at.file, at.text, spot, 8000);
			return text(md ? md.slice(0, 3000) : "The language server says nothing about it.");
		}

		const ask = args.action === "definition" ? () => definitionAt(profile, root, machine, at.file, at.text, spot, 8000) : () => referencesAt(profile, root, machine, at.file, at.text, spot);
		const { got: places, still } = await settled(profile, ask);
		if (!places.length) return text(still || `No ${args.action === "definition" ? "definition" : "references"} found for "${args.symbol ?? "it"}".`);
		return text(await withLines(info, places.slice(0, MAX_REFERENCES), rel) + (places.length > MAX_REFERENCES ? `\n… ${places.length - MAX_REFERENCES} more` : ""));
	},
});

/** `path:line: the line's code`, grouped as they came, each file read once. */
async function withLines(info: { cwd: string; project?: string }, places: Place[], rel: (p: string) => string): Promise<string> {
	const texts = new Map<string, string[] | undefined>();
	const out: string[] = [];
	for (const p of places) {
		if (!texts.has(p.path)) texts.set(p.path, (await placeOf(info, p.path))?.text.split("\n"));
		const code = texts.get(p.path)?.[p.range.start.line]?.trim() ?? "";
		out.push(`${rel(p.path)}:${p.range.start.line + 1}: ${code.length > 200 ? code.slice(0, 197) + "..." : code}`);
	}
	return out.join("\n");
}
