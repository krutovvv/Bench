// The assistant's system prompt, as Pi sections (each is resent only when it changed, which keeps
// the provider's prompt cache warm): who it is (the agent's SYSTEM.md and AGENTS.md), where it works, the project's own
// instructions, the skills it can load, and the notes it keeps. Short on purpose: a local model
// at 60–150 tokens/s pays for every line, and for every extra round a "verify everything" rule causes.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { agentDir, homeAgent, isProject, timezone } from "./config.ts";
import { LIMITS, readEntries } from "./builtin/memory.ts";
import { listSkills } from "./builtin/skills.ts";
import { machineOf, repoOf, skillsFor } from "./repo.ts";
import { projects } from "./builtin/projects.ts";
import type { ThreadFacts } from "./host.ts";

const IDENTITY = "You are a capable assistant working on the person's own computer through tools.";

const read = (f: string) => (fs.existsSync(f) ? fs.readFileSync(f, "utf8").trim() : "");

/** The agent's SYSTEM.md (who it is), as in Pi; then its AGENTS.md (standing instructions). */
export function soul(s: ThreadFacts): string {
	const home = agentDir(homeAgent(s.agent));
	const instructions = read(path.join(home, "AGENTS.md"));
	const parts = [read(path.join(home, "SYSTEM.md")) || IDENTITY, instructions && `# Standing instructions\n\n${instructions}`];
	// A project's own soul: what it is, how to work in it.
	if (isProject(s.agent)) {
		const own = read(path.join(agentDir(s.agent), "SYSTEM.md"));
		const p = projects.get(s.agent.slice(8));
		const name = p?.name ?? "this project";
		// Worked on another machine: say where, so commands and paths are meant for it.
		const where = p?.machine
			? `\n\nIts work is on the server \`${p.machine}\`${p.folder ? `, in ${p.folder}` : ""}: read, write, edit and bash act there over SSH, not on the machine you run on. Paths are that server's.`
			: "";
		const kept = p?.confined && p.folder ? `\n\nWork only in its folder ${p.folder}: your tools refuse anything outside it, and commands cannot write outside it.` : "";
		parts.push(`# The project: ${name}\n\n${own || "(no description yet)"}${where}${kept}`);
	}
	return parts.filter(Boolean).join("\n\n");
}

const PLATFORM: Record<string, string> = {
	local:
		"You are in Bench, a desktop chat app. Markdown renders (tables, code, math with $…$). To hand over a file, write MEDIA:/absolute/path on its own line: images, audio and video show inline, other files as a card.",
	telegram:
		"You are in Telegram. Markdown converts (bold, italic, code, links, headers); avoid tables. To send a file, write MEDIA:/absolute/path on its own line. Keep answers compact.",
	cron: "You run as a scheduled job: nobody is there to answer questions. Do the task and put the result in your answer; it is delivered as is.",
};

export function environment(s: ThreadFacts): string {
	const day = new Intl.DateTimeFormat("en-US", { timeZone: timezone(), weekday: "long", year: "numeric", month: "long", day: "numeric" }).format(new Date());
	return [
		`Today is ${day} (${timezone()}). For the exact time, run date.`,
		`Machine: ${os.type() === "Darwin" ? "macOS" : os.type()}, home ${os.homedir()}. Working directory: ${s.cwd}.`,
		"Bench's own data (tasks, sessions, settings) is reached through your tools, not files.",
		// The tools for code, said once: a long thread spent most of its steps reading files through
		// bash in small windows and editing them with scripts.
		"Files: find with grep (text), glob (names) and ls (a folder's tree); read a file whole with read (up to 2000 lines at once, not in small pieces); change it with edit, all its changes in one call (edits[]); new files with write. bash is for running things — builds, tests, git — not for reading or editing files.",
		settings(s),
		PLATFORM[s.source] ?? PLATFORM.local,
	]
		.filter(Boolean)
		.join("\n");
}

/** Where the thread's project keeps its settings, which the person edits in the project's
 * Settings in Bench, and the agent when asked. */
function settings(s: ThreadFacts): string | undefined {
	if (!s.project) return undefined;
	const dir = path.join(path.dirname(agentDir("project:x")), s.project);
	return `This project's settings live in ${dir}: SYSTEM.md (its soul: what it is, how to work in it), mcp.json (its MCP servers, Pi's format: {"mcpServers": {"<name>": {"command", "args", "env"} or {"url", "headers"}}}), skills/<name>/SKILL.md, prompts/. The person edits them in the project's Settings in Bench; change them with read, write and edit only when asked. Keys go into mcp.json as \${NAME}, their values into the agent's env (ask for them).`;
}

/** AGENTS.md (or CLAUDE.md) from the repository root down to the working directory, read where
 * it is (a project's server too: `repo.ts`). */
export async function projectContext(s: ThreadFacts): Promise<string | undefined> {
	const machine = machineOf(s.project);
	if (machine) {
		const { instructions } = await repoOf(s.cwd, machine).catch(() => ({ instructions: [] }));
		const parts = instructions.map((i) => `## ${path.posix.relative(s.cwd, i.file) || path.posix.basename(i.file)}\n\n${i.text}`);
		return parts.length ? `# Project instructions\n\n${parts.join("\n\n")}` : undefined;
	}
	if (path.resolve(s.cwd) === os.homedir()) return undefined;
	let root = s.cwd;
	try {
		root = execFileSync("git", ["-C", s.cwd, "rev-parse", "--show-toplevel"], { encoding: "utf8", timeout: 2000, stdio: ["ignore", "pipe", "ignore"] }).trim() || s.cwd;
	} catch {}
	const dirs: string[] = [];
	for (let d = path.resolve(s.cwd); ; d = path.dirname(d)) {
		dirs.unshift(d);
		if (d === root || d === path.dirname(d)) break;
	}
	const parts: string[] = [];
	for (const d of dirs)
		for (const name of ["AGENTS.md", "CLAUDE.md"]) {
			const f = path.join(d, name);
			if (fs.existsSync(f)) {
				parts.push(`## ${path.relative(s.cwd, f) || name}\n\n${fs.readFileSync(f, "utf8").trim().slice(0, 20000)}`);
				break;
			}
		}
	return parts.length ? `# Project instructions\n\n${parts.join("\n\n")}` : undefined;
}

export async function skills(s: ThreadFacts): Promise<string | undefined> {
	const list = await skillsFor(s.agent, s.cwd, machineOf(s.project)).catch(() => listSkills(s.agent, s.cwd, []));
	if (!list.length) return undefined;
	const lines = list.map((k) => `- ${k.name}: ${k.description.replace(/\s+/g, " ").slice(0, 120)}`);
	return `# Skills\nThe person's own procedures. When one fits the task, load it with skill_view first and follow it.\n${lines.join("\n")}`;
}

export function notes(s: ThreadFacts): string | undefined {
	const user = readEntries(s.agent, "user");
	const memory = readEntries(s.agent, "memory");
	if (!user.length && !memory.length) return undefined;
	const { memory: ml, user: ul } = LIMITS();
	const block = (title: string, entries: string[], limit: number) => (entries.length ? `## ${title} (${entries.join("\n§\n").length}/${limit} chars)\n${entries.join("\n§\n")}` : "");
	return ["# Notes kept across sessions (change them with the memory tool, only when asked)", block("About the person", user, ul), block("Your notes", memory, ml)].filter(Boolean).join("\n\n");
}

/**
 * What the person inserted into the thread with "/" (src/inserts.ts), each as live as a window
 * next to the conversation. Built from the thread's `bench.inserts` document, so it changes only
 * when an insert comes, goes or switches (another list, another tool): the prompt cache keeps.
 */
export function inserted(items: { key: string; state: Record<string, unknown> }[]): string | undefined {
	if (!items.length) return undefined;
	const lines = items.map(({ key, state }) => {
		const [kind] = key.split(":");
		if (kind === "ssh") return "- This machine's terminal. Run commands on it with ssh_run (host \"local\").";
		if (kind === "tasks")
			return `- The person's tasks, the "${state.list ?? "today"}" list. One list for every project; a project is a tag (its name as a slug), and in a project's thread the tasks tool works on that tag unless told "all". Read and change them with the tasks tool — the only place they live; the list updates by itself, so don't repeat it back unless asked. Time spent: list status "completed" with done_since / done_until and a tag; the result sums it.`;
		if (kind === "tools") return `- The everyday utilities, now on "${state.tool ?? ""}" (its input and output come with each message). Do what is asked with its content yourself (read, bash) and give the result in your answer.`;
		if (kind === "browser")
			return "- This thread's own tab in the server's shared browser, live: the person watches it, can click and type in it, then says \"continue\". Drive it with the mcp__browser__* tools only (find them with tool_search \"browser\" if not loaded). Work only in this thread's tab — the others are other threads'; <block-state> says its address: at the start of a turn, browser_tabs list and select it; if it is gone, browser_tabs new and go on there (it becomes this thread's tab). The browser keeps the sign-ins. The person already sees the page: read it with snapshots, never take screenshots to show it. End each turn with what is on the page now and what the next step would be.";
		return `- ${key}`;
	});
	return [
		"# Inserted in this thread",
		"The person put these into the conversation; they are live, and \"this\", \"here\", \"it\" usually mean one of them. Each message may end with <block-state>: what they show at that moment. Use it; never quote it back.",
		...lines,
	].join("\n");
}
