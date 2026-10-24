// Skills: folders with a SKILL.md (YAML front matter: name, description), in the profile's
// skills/ (optionally grouped in category folders). Only their names and descriptions are in the
// prompt; `skill_view` loads one when it applies, `skill_manage` writes them when asked.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import fs from "node:fs";
import path from "node:path";
import YAML from "yaml";
import { agentDir } from "../config.ts";
import { sessionOf } from "../host.ts";

export interface Skill {
	name: string;
	description: string;
	category: string;
	dir: string;
}

export function frontMatter(text: string): Record<string, any> {
	const m = text.match(/^---\n([\s\S]*?)\n---/);
	try {
		return (m && YAML.parse(m[1])) || {};
	} catch {
		return {};
	}
}

export function listSkills(profile: string): Skill[] {
	const root = path.join(agentDir(profile), "skills");
	const out: Skill[] = [];
	const walk = (d: string, category: string, depth: number) => {
		if (depth > 3 || !fs.existsSync(d)) return;
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			if (!e.isDirectory() || e.name.startsWith(".")) continue;
			const dir = path.join(d, e.name);
			const file = path.join(dir, "SKILL.md");
			if (fs.existsSync(file)) {
				const fm = frontMatter(fs.readFileSync(file, "utf8"));
				out.push({ name: String(fm.name ?? e.name), description: String(fm.description ?? "").trim(), category, dir });
			} else walk(dir, category ? `${category}/${e.name}` : e.name, depth + 1);
		}
	};
	walk(root, "", 0);
	return out.sort((a, b) => a.name.localeCompare(b.name));
}

export function findSkill(profile: string, name: string): Skill | undefined {
	return listSkills(profile).find((s) => s.name === name || path.basename(s.dir) === name);
}

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const skillView = defineTool({
	name: "skill_view",
	description: "Load a skill's instructions (SKILL.md), or one of its files, by the name in the skills list. Load the skill before doing the task it covers.",
	parameters: Type.Object({
		name: Type.String(),
		file: Type.Optional(Type.String({ description: "A file inside the skill folder, e.g. references/api.md" })),
	}),
	execute: async (args, api) => {
		const { profile } = sessionOf(api.conversationId);
		const skill = findSkill(profile, args.name);
		if (!skill) throw new Error(`no skill "${args.name}"; see the skills list`);
		const target = path.resolve(skill.dir, args.file ?? "SKILL.md");
		if (!target.startsWith(skill.dir + path.sep) && target !== path.join(skill.dir, "SKILL.md")) throw new Error("file is outside the skill folder");
		const body = fs.readFileSync(target, "utf8");
		const files = args.file ? [] : fs.readdirSync(skill.dir, { recursive: true }).map(String).filter((f) => f !== "SKILL.md" && fs.statSync(path.join(skill.dir, f)).isFile());
		return text(files.length ? `${body}\n\n[Files in this skill: ${files.join(", ")}]` : body);
	},
});

export const skillManage = defineTool({
	name: "skill_manage",
	description:
		"Create, rewrite, patch or delete a skill, or write a file inside one. Use when the person asks to save a procedure as a skill or to fix one. " +
		"A skill is a folder with SKILL.md starting with front matter (---\\nname: …\\ndescription: one line on when to use it\\n---).",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("create"), Type.Literal("edit"), Type.Literal("patch"), Type.Literal("delete"), Type.Literal("write_file")]),
		name: Type.String(),
		content: Type.Optional(Type.String({ description: "Whole SKILL.md (create, edit) or the file's content (write_file)" })),
		category: Type.Optional(Type.String({ description: "Folder to create the skill in (create)" })),
		old_string: Type.Optional(Type.String({ description: "Unique text to replace (patch)" })),
		new_string: Type.Optional(Type.String({ description: "Replacement (patch)" })),
		file: Type.Optional(Type.String({ description: "Path inside the skill (write_file)" })),
	}),
	execute: async (args, api) => {
		const { profile } = sessionOf(api.conversationId);
		if (!/^[a-z0-9][a-z0-9._-]*$/i.test(args.name)) throw new Error("name: letters, digits, '.', '_', '-'");
		const existing = findSkill(profile, args.name);
		if (args.action === "create") {
			if (existing) throw new Error(`skill "${args.name}" exists; use edit or patch`);
			if (!args.content?.startsWith("---")) throw new Error("content must start with front matter (---)");
			const dir = path.join(agentDir(profile), "skills", args.category ?? "", args.name);
			fs.mkdirSync(dir, { recursive: true });
			fs.writeFileSync(path.join(dir, "SKILL.md"), args.content);
			return text({ success: true, path: path.join(dir, "SKILL.md") });
		}
		if (!existing) throw new Error(`no skill "${args.name}"`);
		const file = path.join(existing.dir, "SKILL.md");
		if (args.action === "delete") {
			fs.rmSync(existing.dir, { recursive: true, force: true });
			return text({ success: true, deleted: existing.dir });
		}
		if (args.action === "edit") {
			if (!args.content?.startsWith("---")) throw new Error("content must start with front matter (---)");
			fs.writeFileSync(file, args.content);
			return text({ success: true, path: file });
		}
		if (args.action === "patch") {
			const body = fs.readFileSync(file, "utf8");
			const n = args.old_string ? body.split(args.old_string).length - 1 : 0;
			if (n !== 1) throw new Error(n ? `old_string occurs ${n} times; make it unique` : "old_string not found");
			fs.writeFileSync(file, body.replace(args.old_string!, args.new_string ?? ""));
			return text({ success: true, path: file });
		}
		const target = path.resolve(existing.dir, args.file ?? "");
		if (!args.file || !target.startsWith(existing.dir + path.sep)) throw new Error("file must be a path inside the skill folder");
		fs.mkdirSync(path.dirname(target), { recursive: true });
		fs.writeFileSync(target, args.content ?? "");
		return text({ success: true, path: target });
	},
});
