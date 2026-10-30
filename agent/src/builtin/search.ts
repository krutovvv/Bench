// Finding things in code without bash, as coding agents have them (OpenCode, Crush): `grep` for
// text in files, `glob` for files by name, `ls` for a folder's tree. Without them the model built
// every search out of bash (grep, find, sed -n …p) and paged files in small windows — most of a
// long thread's steps. They run through the conversation's execution environment, so on a
// project's server they search the server (`machine/ssh.ts`). ripgrep when it is there; grep and find
// otherwise. .git, node_modules, target and the like are left out.

import { defineTool } from "@earendil-works/pi-durable";
import { Type } from "@earendil-works/pi-ai";

const SKIP = [".git", "node_modules", "target", "dist", "build", ".next", "vendor", ".venv", "__pycache__"];

const text = (t: string) => ({ output: [{ type: "text" as const, text: t }] });

/** Run a POSIX script there with its arguments as $1, $2…: no quoting of the model's text. */
async function sh(api: any, context: any, script: string, args: string[]): Promise<string> {
	if (!api.env) throw new Error("No execution environment is configured");
	let out = "";
	const r = await api.env.exec(["sh", "-c", script, "sh", ...args], { onOutput: (t: string) => (out += t) }, context);
	if (!r.ok) throw new Error(r.error.message);
	return out;
}

/** At most `limit` lines, and how many more there were. */
function capped(out: string, limit: number, what: string): string {
	const lines = out.split("\n").filter(Boolean);
	if (lines.length === 0) return `No ${what}.`;
	const more = lines.length - limit;
	return lines.slice(0, limit).join("\n") + (more > 0 ? `\n… ${more} more: narrow the path or the pattern.` : "");
}

const skipGrep = SKIP.map((d) => `--exclude-dir=${d}`).join(" ");
const skipRg = SKIP.map((d) => `-g '!${d}'`).join(" ");
const skipFind = SKIP.map((d) => `-name ${d} -prune -o`).join(" ");

export const grepTool = defineTool({
	name: "grep",
	// Only reads: run again after a restart, it finds what is there then.
	replay: "safe",
	description:
		"Search file contents with a regular expression (ripgrep syntax), in a folder (default: the working directory) or one file. Returns file:line:text, at most `limit` matches. Use it instead of bash grep or sed to find where something is; then read the file around it.",
	parameters: Type.Object({
		pattern: Type.String({ description: "Regular expression, e.g. fn render_\\w+ or TODO" }),
		path: Type.Optional(Type.String({ description: "Folder or file; default the working directory" })),
		include: Type.Optional(Type.String({ description: "Only files matching this glob, e.g. *.rs or *.{ts,tsx}" })),
		ignore_case: Type.Optional(Type.Boolean()),
		context: Type.Optional(Type.Number({ description: "Lines of context around each match (0-5)" })),
		limit: Type.Optional(Type.Number({ description: "Most matches, default 100" })),
	}),
	async execute(args: any, api: any, context: any) {
		const limit = Math.min(Math.max(Number(args.limit) || 100, 1), 500);
		const ctx = Math.min(Math.max(Number(args.context) || 0, 0), 5);
		const flags = `${args.ignore_case ? "-i" : ""} ${ctx ? `-C ${ctx}` : ""}`;
		// $3 (the include glob) is passed as one word, never expanded here.
		const script =
			`{ if command -v rg >/dev/null 2>&1; then ` +
			`if [ -n "$3" ]; then rg -n --no-heading --color never ${flags} ${skipRg} -g "$3" -e "$1" -- "$2"; else rg -n --no-heading --color never ${flags} ${skipRg} -e "$1" -- "$2"; fi; ` +
			`else if [ -n "$3" ]; then grep -rnIE ${flags} ${skipGrep} --include="$3" -e "$1" -- "$2"; else grep -rnIE ${flags} ${skipGrep} -e "$1" -- "$2"; fi; fi; } 2>/dev/null | head -n ${limit + 1}`;
		return text(capped(await sh(api, context, script, [String(args.pattern), String(args.path || "."), String(args.include ?? "")]), limit, "matches"));
	},
});

export const globTool = defineTool({
	name: "glob",
	// Only reads: run again after a restart, it finds what is there then.
	replay: "safe",
	description:
		"Find files by name with a glob, e.g. **/*.rs, src/**/mod.rs or *.config.js, under a folder (default the working directory). Returns paths, at most `limit`. Use it instead of bash find or ls -R.",
	parameters: Type.Object({
		pattern: Type.String({ description: "A glob: **/*.ts, src/**/test_*.py, *.md" }),
		path: Type.Optional(Type.String({ description: "Folder to look under; default the working directory" })),
		limit: Type.Optional(Type.Number({ description: "Most paths, default 200" })),
	}),
	async execute(args: any, api: any, context: any) {
		const limit = Math.min(Math.max(Number(args.limit) || 200, 1), 1000);
		// find matches the last component; a pattern with a folder matches the whole path (** as *,
		// which crosses folders in -path).
		const script =
			`cd "$2" || exit 1; if command -v rg >/dev/null 2>&1; then rg --files --color never ${skipRg} -g "$1"; ` +
			`else case "$1" in */*) find . ${skipFind} -path "./$(printf %s "$1" | sed 's|\\*\\*/|*|g')" -print ;; *) find . ${skipFind} -name "$1" -print ;; esac | sed 's|^\\./||'; fi 2>/dev/null | head -n ${limit + 1}`;
		return text(capped(await sh(api, context, script, [String(args.pattern), String(args.path || ".")]), limit, "files"));
	},
});

export const lsTool = defineTool({
	name: "ls",
	// Only reads: run again after a restart, it finds what is there then.
	replay: "safe",
	description:
		"A folder's tree: folders end with /, depth 2 by default (at most 4), at most 300 entries; .git, node_modules, target and the like are left out. Use it to see how a project is laid out.",
	parameters: Type.Object({
		path: Type.Optional(Type.String({ description: "Folder; default the working directory" })),
		depth: Type.Optional(Type.Number({ description: "Levels, 1-4, default 2" })),
	}),
	async execute(args: any, api: any, context: any) {
		const depth = Math.min(Math.max(Number(args.depth) || 2, 1), 4);
		// Folders end with / (BSD find has no -printf: each path is looked at).
		const script = `cd "$1" || exit 1; find . -mindepth 1 -maxdepth ${depth} ${skipFind} -print 2>/dev/null | sed 's|^\\./||' | while IFS= read -r f; do if [ -d "$f" ]; then printf '%s/\\n' "$f"; else printf '%s\\n' "$f"; fi; done`;
		const out = await sh(api, context, script, [String(args.path || ".")]);
		return text(capped(out.split("\n").filter(Boolean).sort().join("\n"), 300, "entries"));
	},
});
