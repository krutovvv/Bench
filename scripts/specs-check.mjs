#!/usr/bin/env node
// Checks Bench's spec graph (SPEC.md at the root): every `SPEC.md` in the tree is a node. Fails on a missing field, an
// unknown type or status, a duplicate id, a link to an id that does not exist, a parent cycle, a
// `covers` path that does not exist, or a graph with other than one root.
//
//   node scripts/specs-check.mjs        prints the tree, exits 1 on any problem

import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const SKIP = new Set([".git", "node_modules", "target", "vendor", "dist", ".claude"]);
const TYPES = new Set(["goal-and-requirements", "architecture-design", "module-design", "submodule-design", "task-spec"]);
const STATUSES = new Set(["draft", "active", "stale", "done", "deprecated"]);
const LINKS = ["depends-on", "references", "implements"];

function walk(dir, out) {
	for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
		if (SKIP.has(e.name)) continue;
		const p = path.join(dir, e.name);
		if (e.isDirectory()) walk(p, out);
		else if (e.name === "SPEC.md") out.push(p);
	}
	return out;
}

/** The frontmatter's flat `key: value` lines; `[a, b]` and `- a` lines are lists. */
function frontmatter(text) {
	const m = text.match(/^---\n([\s\S]*?)\n---\n/);
	if (!m) return null;
	const fm = {};
	let last = null;
	for (const line of m[1].split("\n")) {
		const item = line.match(/^\s+-\s+(.+)$/);
		if (item && last) {
			fm[last] = [...(Array.isArray(fm[last]) ? fm[last] : []), item[1].trim()];
			continue;
		}
		const kv = line.match(/^([A-Za-z][\w-]*):\s*(.*)$/);
		if (!kv) continue;
		last = kv[1];
		const v = kv[2].trim();
		fm[last] = v.startsWith("[") && v.endsWith("]") ? v.slice(1, -1).split(",").map((s) => s.trim()).filter(Boolean) : v;
	}
	return fm;
}

const files = walk(ROOT, []);

const problems = [];
const nodes = new Map();
for (const file of files) {
	const rel = path.relative(ROOT, file);
	const fm = frontmatter(fs.readFileSync(file, "utf8"));
	if (!fm?.id || !fm?.type) {
		if (path.basename(file) === "SPEC.md") problems.push(`${rel}: no frontmatter with id and type`);
		continue;
	}
	if (nodes.has(fm.id)) problems.push(`${rel}: id "${fm.id}" is also ${nodes.get(fm.id).rel}`);
	if (!fm.title) problems.push(`${rel}: no title`);
	if (!TYPES.has(fm.type)) problems.push(`${rel}: unknown type "${fm.type}"`);
	if (fm.status && !STATUSES.has(fm.status)) problems.push(`${rel}: unknown status "${fm.status}"`);
	for (const c of [fm.covers ?? []].flat()) if (!fs.existsSync(path.join(ROOT, c))) problems.push(`${rel}: covers "${c}", which does not exist`);
	nodes.set(fm.id, { ...fm, rel });
}

for (const [id, n] of nodes) {
	if (n.parent && !nodes.has(n.parent)) problems.push(`${n.rel}: parent "${n.parent}" does not exist`);
	for (const k of LINKS) for (const to of [n[k] ?? []].flat()) if (!nodes.has(to)) problems.push(`${n.rel}: ${k} "${to}" does not exist`);
	const seen = new Set([id]);
	for (let p = n.parent; p && nodes.has(p); p = nodes.get(p).parent) {
		if (seen.has(p)) {
			problems.push(`${n.rel}: parent cycle through "${p}"`);
			break;
		}
		seen.add(p);
	}
}

const roots = [...nodes.values()].filter((n) => !n.parent);
if (roots.length !== 1) problems.push(`one root expected, found ${roots.length}: ${roots.map((r) => r.id).join(", ") || "none"}`);
else if (roots[0].type !== "goal-and-requirements") problems.push(`the root ${roots[0].id} is not a goal-and-requirements`);

const children = (id) => [...nodes.values()].filter((n) => n.parent === id).sort((a, b) => a.rel.localeCompare(b.rel));
const print = (n, depth) => {
	console.log(`${"  ".repeat(depth)}${n.id}  (${n.type}, ${n.status ?? "no status"})  ${n.rel}`);
	for (const c of children(n.id)) print(c, depth + 1);
};
for (const r of roots) print(r, 0);

if (problems.length) {
	console.error(`\n${problems.length} problem(s):\n${problems.map((p) => `  ${p}`).join("\n")}`);
	process.exit(1);
}
console.log(`\n${nodes.size} specs, no problems.`);
