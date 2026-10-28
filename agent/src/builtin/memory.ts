// `memory`: two small notebooks of the agent, memories/MEMORY.md (the agent's notes) and
// memories/USER.md (about the person), entries separated by "\n§\n". Both are
// in the system prompt. Written only when the person asks; there is no automatic saving.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import fs from "node:fs";
import path from "node:path";
import { getSetting, agentDir, homeAgent } from "../config.ts";
import { threadOf } from "../host.ts";

const SEP = "\n§\n";

export const LIMITS = () => {
	const m = getSetting<{ memory_char_limit?: number; user_char_limit?: number }>("memory", {}) ?? {};
	return { memory: Number(m.memory_char_limit ?? 2200), user: Number(m.user_char_limit ?? 1375) };
};

export function memoryFile(agent: string, target: "memory" | "user"): string {
	// One memory for every project's threads: the general agent's.
	return path.join(agentDir(homeAgent(agent)), "memories", target === "user" ? "USER.md" : "MEMORY.md");
}

export function readEntries(agent: string, target: "memory" | "user"): string[] {
	const f = memoryFile(agent, target);
	return fs.existsSync(f) ? fs.readFileSync(f, "utf8").split(SEP).map((e) => e.trim()).filter(Boolean) : [];
}

function write(agent: string, target: "memory" | "user", entries: string[]): void {
	const f = memoryFile(agent, target);
	fs.mkdirSync(path.dirname(f), { recursive: true });
	fs.writeFileSync(f + ".tmp", entries.join(SEP));
	fs.renameSync(f + ".tmp", f);
}

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const memoryTool = defineTool({
	name: "memory",
	description:
		"Save, replace or remove a short note that will be in your context in every future session. " +
		"target 'user' is about the person (who they are, preferences); 'memory' is everything else worth keeping across sessions " +
		"(environment facts, conventions). Use only when the person asks you to remember something. Write facts, not instructions. " +
		"Notes have a character budget: replace or merge old ones instead of piling up.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("add"), Type.Literal("replace"), Type.Literal("remove")]),
		target: Type.Union([Type.Literal("memory"), Type.Literal("user")]),
		content: Type.Optional(Type.String({ description: "The note (add, replace)" })),
		old_text: Type.Optional(Type.String({ description: "A unique part of the note to replace or remove" })),
	}),
	execute: async (args, api) => {
		const { agent } = threadOf(api.conversationId);
		const entries = readEntries(agent, args.target);
		const limit = LIMITS()[args.target];
		const find = () => {
			const hits = entries.map((e, i) => [e, i] as const).filter(([e]) => args.old_text && e.includes(args.old_text));
			if (hits.length !== 1) throw new Error(hits.length ? `old_text matches ${hits.length} notes; make it unique` : "old_text matches no note");
			return hits[0][1];
		};
		if (args.action === "add") {
			if (!args.content?.trim()) throw new Error("content is required");
			entries.push(args.content.trim());
		} else if (args.action === "replace") {
			if (!args.content?.trim()) throw new Error("content is required");
			entries[find()] = args.content.trim();
		} else entries.splice(find(), 1);
		const used = entries.join(SEP).length;
		if (used > limit) throw new Error(`over the ${limit}-character budget (${used}); replace or remove an old note first`);
		write(agent, args.target, entries);
		return text({ success: true, target: args.target, entries: entries.length, usage: `${used}/${limit}` });
	},
});
