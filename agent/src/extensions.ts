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
// "codemode" is Pi's code mode: one script calls many tools, and only what it prints reaches the
// model. Every thread has it, beside the tools it can call directly: not a mode for code, a way to
// do several steps in one.

import * as ai from "@earendil-works/pi-ai";
import * as pi from "@earendil-works/pi-durable";
import { CodemodeSandbox } from "@earendil-works/pi-codemode";
import { hasDeferred, NAMESPACES } from "./mcp/mcp.ts";
import { createCodemodeExtension } from "@earendil-works/pi-durable/codemode";
import { createToolSearchExtension } from "@earendil-works/pi-durable/tool-search";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as ask from "./builtin/ask.ts";
import * as cron from "./builtin/cron.ts";
import * as memory from "./builtin/memory.ts";
import * as skills from "./builtin/skills.ts";
import { agentDir, agents } from "./config.ts";
import { codingTools } from "./builtin/hashline.ts";
import { assistant, images } from "./extensions/assistant.ts";
import { host } from "./host.ts";
import { mcpTools } from "./mcp/mcp.ts";
import * as prompt from "./prompt.ts";


/** What a user extension's default export receives. */
export const blocks = () => {
	const byName = Object.fromEntries(codingTools({ images }).map((t: any) => [t.name, t]));
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
	// Code mode with `models.*`, as Pi has it: a script asks the evaluator (`bench/evaluator`,
	// `judge/models.ts`) for many small decisions, where its project uses it (`extensions/assistant.ts`).
	// Each MCP server is a namespace of its scripts (`mcp/mcp.ts` keeps the list as they connect).
	const codemode = createCodemodeExtension({ sandbox: new CodemodeSandbox({ memoryLimitBytes: 256 * 1024 * 1024 }), namespaces: NAMESPACES });
	const builtin: pi.Extension[] = [assistant as any, createToolSearchExtension() as any, codemode as any, ...agents().map(mcpExtension)];
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
		// Edits are picked up without a restart. The watch alone does not keep the process alive.
		let timer: NodeJS.Timeout | undefined;
		fs.watch(path.join(agentDir(agent), "extensions"), () => {
			clearTimeout(timer);
			timer = setTimeout(async () => {
				await loadAgent(agent);
				onChange();
			}, 300);
		}).unref();
	}
}

/** An agent's extensions, as objects for `configure()`: its own by name, else the built-in one; always its MCP and code mode, tool search where it has deferred tools. */
export function forAgent(names: string[] | undefined, agent: string): pi.Extension[] {
	// tool_search where a server's tools wait behind it (`deferred`); Pi's own description sends
	// the model to it for every MCP tool, which scripts reach instead under the default exposure.
	const wanted = [...new Set([...(names?.length ? names : ["assistant"]), ...(hasDeferred(agent) ? ["tool-search"] : []), "codemode"])];
	// Its own extensions by name, else the built-in ones; its own MCP servers. A project takes
	// nothing of Host's: Host's extensions and servers are Host's threads' alone.
	const list = wanted.map((n) => loaded.get(`${agent}:${n}`)?.extension ?? loaded.get(n)?.extension).filter(Boolean) as pi.Extension[];
	const mcp = loaded.get(`mcp:${agent}`)?.extension;
	return mcp ? [...list, mcp] : list;
}

/** After an agent's MCP servers reconnected. */
export function installMcp(registry: { install(e: pi.Extension): void }, agent: string): void {
	const e = mcpExtension(agent);
	loaded.set(e.name, { name: e.name, source: "builtin", extension: e });
	registry.install(e);
}
