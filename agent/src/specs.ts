// A project's specs: `SPEC.md` beside each module and the documents adopted as nodes (a `.md` under
// docs/ whose frontmatter carries `id` and `type`), linked into a graph by their frontmatter's
// `parent` and `references` (the repository's SPEC.md says how; ThinkRail's practice). Read on the
// project's machine, from the repository around its folder, and kept a few seconds: the agent asks
// on every read and edit which spec a file is under (`specFor`), Bench lists them for the
// Explorer's "Specs" (`GET /api/specs`).

import path from "node:path";
import { shOut } from "./machine/sh.ts";

export interface Spec {
	id: string;
	type: string;
	title: string;
	status?: string;
	parent?: string;
	references: string[];
	/** Absolute, on the project's machine. */
	path: string;
	/** Paths (from the root) it describes besides its own folder. */
	covers: string[];
}

/** Every SPEC.md and docs/*.md the repository has (tracked or not, ignored ones left out), each
 * with its frontmatter: a \x01 line naming the file, then the frontmatter's lines. */
const LIST = `root=$(git rev-parse --show-toplevel 2>/dev/null) || root=$(pwd)
cd "$root" || exit 0
printf '\\002%s\\n' "$root"
{ git ls-files -co --exclude-standard -- '*SPEC.md' 'docs/*.md' 2>/dev/null || find . -name SPEC.md -not -path '*/node_modules/*'; } | sort -u | while IFS= read -r f; do
  [ -f "$f" ] || continue
  [ "$(head -1 "$f")" = "---" ] || continue
  printf '\\001%s\\n' "\${f#./}"
  sed -n '2,/^---$/p' "$f"
done`;

/** The frontmatter's flat `key: value` lines; `[a, b]` and `- a` lines are lists. */
export function frontmatter(lines: string[]): Record<string, string | string[]> {
	const fm: Record<string, string | string[]> = {};
	let last: string | null = null;
	for (const line of lines) {
		if (line === "---") break;
		const item = line.match(/^\s+-\s+(.+)$/);
		if (item && last) {
			const prev = fm[last];
			fm[last] = [...(Array.isArray(prev) ? prev : []), item[1].trim()];
			continue;
		}
		const kv = line.match(/^([A-Za-z][\w-]*):\s*(.*)$/);
		if (!kv) continue;
		last = kv[1];
		const v = kv[2].trim();
		fm[last] = v.startsWith("[") && v.endsWith("]") ? v.slice(1, -1).split(",").map((s) => s.trim()).filter(Boolean) : v.replace(/^["']|["']$/g, "");
	}
	return fm;
}

/** The listing read as specs: the root, and the files whose frontmatter has an id and a type. */
export function parseSpecs(out: string): { root: string; specs: Spec[] } {
	const root = out.match(/^\x02(.*)$/m)?.[1] ?? "";
	const specs: Spec[] = [];
	for (const block of out.split("\x01").slice(1)) {
		const [file, ...rest] = block.split("\n");
		const fm = frontmatter(rest);
		const one = (k: string) => (typeof fm[k] === "string" ? (fm[k] as string) : undefined);
		const many = (k: string) => [fm[k] ?? []].flat().filter(Boolean) as string[];
		const id = one("id");
		const type = one("type");
		if (!id || !type) continue;
		specs.push({
			id,
			type,
			title: one("title") ?? id,
			status: one("status"),
			parent: one("parent"),
			references: [...many("references"), ...many("depends-on")],
			path: path.posix.join(root, file),
			covers: many("covers"),
		});
	}
	return { root, specs };
}

const cache = new Map<string, { at: number; value: Promise<{ root: string; specs: Spec[] }> }>();
const FRESH_MS = 5000;

/** The specs of the repository around `folder` on `machine` (none: this one). */
export function specsOf(machine: string | null, folder: string): Promise<{ root: string; specs: Spec[] }> {
	const key = `${machine ?? ""}:${folder}`;
	const hit = cache.get(key);
	if (hit && Date.now() - hit.at < FRESH_MS) return hit.value;
	const value = shOut(machine, folder, LIST, [])
		.then(parseSpecs)
		.catch(() => ({ root: "", specs: [] }));
	cache.set(key, { at: Date.now(), value });
	return value;
}

/** The spec a file is under: the nearest SPEC.md in its folder or one above it (within the root),
 * else one that `covers` it. None for a spec itself. */
export function specFor(root: string, specs: Spec[], file: string): Spec | undefined {
	if (!root || !file.startsWith(root.replace(/\/$/, "") + "/")) return undefined;
	if (specs.some((s) => s.path === file)) return undefined;
	const byDir = new Map(specs.filter((s) => path.posix.basename(s.path) === "SPEC.md").map((s) => [path.posix.dirname(s.path), s]));
	for (let d = path.posix.dirname(file); d.length >= root.length; d = path.posix.dirname(d)) {
		const s = byDir.get(d);
		// The root's own SPEC.md is the product's goal, not a module's: a file it alone is above
		// is pointed at a spec that covers it, or at nothing, never at the whole product.
		if (s && d !== root.replace(/\/$/, "")) return s;
		if (d === "/" || d === ".") break;
	}
	const rel = file.slice(root.replace(/\/$/, "").length + 1);
	return specs.find((s) => s.covers.some((c) => rel === c || rel.startsWith(c.replace(/\/$/, "") + "/")));
}
