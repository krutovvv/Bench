// Agent extensions, Pi Durable's unit of behaviour (tools, prompt sections, hooks, durable tasks).
//
// Built in: "assistant" (src/extensions/assistant.ts), the agent's MCP servers' tools and Pi's
// "tool-search". The person's own, per agent as in Pi: agents/<agent>/extensions/<name>.ts, loaded
// without a restart: a changed file is imported again and installed under its name, which Pi
// replaces in place ("Reload" in its README): new work uses the new code, running work finishes
// on the old. A file exports a default function that gets Bench's building blocks and returns
// the extension (so it needs no node_modules of its own):
//
//   export default ({ pi, ai, tools }) => pi.defineExtension({
//     name: "research",
//     tools: [tools.read, tools.bash, ...],
//     sections: [pi.section("role", () => "You research …")],
//   });
//
// An agent chooses its extensions in settings.json ("extensions": ["assistant", "research"]).

import * as ai from "@earendil-works/pi-ai";
import * as pi from "@earendil-works/pi-durable";
import { createToolSearchExtension } from "@earendil-works/pi-durable/tool-search";
import * as piTools from "@earendil-works/pi-durable/tools";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as ask from "./builtin/ask.ts";
import * as cron from "./builtin/cron.ts";
import * as memory from "./builtin/memory.ts";
import * as skills from "./builtin/skills.ts";
import * as subagent from "./builtin/subagent.ts";
import * as taskStore from "./builtin/tasks.ts";
import { agentDir, agents } from "./config.ts";
import { assistant } from "./extensions/assistant.ts";
import { host } from "./host.ts";
import { mcpTools } from "./mcp.ts";
import * as prompt from "./prompt.ts";


/** What a user extension's default export receives. */
export const blocks = () => {
	const coding = piTools.createCodingTools();
	const byName = Object.fromEntries((coding.tools ?? []).map((t: any) => [t.name, t]));
	return {
		pi,
		ai,
		host,
		prompt,
		tools: {
			read: byName.read,
			write: byName.write,
			edit: byName.edit,
			bash: byName.bash,
			clarify: ask.clarify,
			todo: ask.todo,
			session_search: ask.sessionSearch,
			memory: memory.memoryTool,
			skill_view: skills.skillView,
			skill_manage: skills.skillManage,
			cron: cron.cronTool,
			delegate: subagent.delegate,
			tasks: taskStore.tasksTool,
		},
	};
};

export interface Loaded {
	/** "<agent>:<name>" for an agent's own, the bare name for a built-in one. */
	name: string;
	agent?: string;
	source: "builtin" | "user";
	file?: string;
	extension?: pi.Extension;
	error?: string;
}

const loaded = new Map<string, Loaded>();

export function extensions(): Loaded[] {
	return [...loaded.values()];
}

export function extension(name: string): pi.Extension | undefined {
	return loaded.get(name)?.extension;
}

/** One agent's MCP tools as an extension, "mcp:<agent>" (rebuilt when its servers reconnect). */
export function mcpExtension(agent: string): pi.Extension {
	return pi.defineExtension({ name: `mcp:${agent}`, tools: mcpTools(agent) } as any);
}

/** An agent's extension file; installed as "<agent>:<its name>" so agents never clash. */
async function loadFile(agent: string, file: string): Promise<Loaded> {
	const base = path.basename(file, ".ts");
	try {
		const stamp = fs.statSync(file).mtimeMs;
		const mod = await import(`${pathToFileURL(file).href}?v=${stamp}`);
		const made = typeof mod.default === "function" ? await mod.default(blocks()) : mod.default;
		if (!made?.name) throw new Error("the default export must be an extension or return one");
		const name = `${agent}:${made.name}`;
		return { name, agent, source: "user", file, extension: { ...made, name } };
	} catch (e) {
		return { name: `${agent}:${base}`, agent, source: "user", file, error: String((e as Error)?.message ?? e) };
	}
}

/** Loads the built-in and every agent's extensions into the registry; `onChange` after a reload. */
export async function loadExtensions(registry: { install(e: pi.Extension): void }, onChange: () => void): Promise<void> {
	const builtin: pi.Extension[] = [assistant as any, createToolSearchExtension() as any, ...agents().map(mcpExtension)];
	for (const e of builtin) {
		loaded.set(e.name, { name: e.name, source: "builtin", extension: e });
		registry.install(e);
	}
	const loadAgent = async (agent: string) => {
		const dir = path.join(agentDir(agent), "extensions");
		fs.mkdirSync(dir, { recursive: true });
		for (const f of fs.readdirSync(dir).filter((f) => f.endsWith(".ts"))) {
			const l = await loadFile(agent, path.join(dir, f));
			loaded.set(l.name, l);
			if (l.extension) registry.install(l.extension);
			console.log(`[extensions] ${l.name}: ${l.error ?? "loaded"}`);
		}
	};
	for (const agent of agents()) {
		await loadAgent(agent);
		// Edits are picked up without a restart.
		let timer: NodeJS.Timeout | undefined;
		fs.watch(path.join(agentDir(agent), "extensions"), () => {
			clearTimeout(timer);
			timer = setTimeout(async () => {
				await loadAgent(agent);
				onChange();
			}, 300);
		});
	}
}

/** An agent's extensions, as objects for `configure()`: its own by name, else the built-in one; always its MCP and tool search. */
export function forAgent(names: string[] | undefined, agent: string): pi.Extension[] {
	const wanted = [...new Set([...(names?.length ? names : ["assistant"]), "tool-search"])];
	const own = agent.startsWith("project:") ? "default" : agent;
	const list = wanted.map((n) => loaded.get(`${own}:${n}`)?.extension ?? loaded.get(n)?.extension).filter(Boolean) as pi.Extension[];
	// A project's threads: the general MCP servers and the project's own.
	const mcp = [...new Set([own, agent])].map((a) => loaded.get(`mcp:${a}`)?.extension).filter(Boolean) as pi.Extension[];
	return [...list, ...mcp];
}

/** After an agent's MCP servers reconnected. */
export function installMcp(registry: { install(e: pi.Extension): void }, agent: string): void {
	const e = mcpExtension(agent);
	loaded.set(e.name, { name: e.name, source: "builtin", extension: e });
	registry.install(e);
}
