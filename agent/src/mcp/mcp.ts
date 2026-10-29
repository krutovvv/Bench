// MCP servers, per agent: the agent's mcp.json (`{ "mcpServers": { name: { command, args, env, cwd }
// | { url, headers } } }`, Pi's format), through the official client @earendil-works/pi-mcp. Tools
// are named mcp__<server>__<tool>, as in Pi. `exposure` as in Pi's docs/mcp.md: "codemode", the
// default (called from codemode scripts only: neither offered to the model nor listed, found with
// searchTools()), "direct" (offered to the model, scripts call it too), "deferred" (found with
// tool_search), "hidden" (not reachable). `toolExposure` overrides it per tool (`*` patterns). A
// script gets a tool's CallToolResult whole; each server is a namespace of code mode. A project worked
// on a server runs its command servers there (`stdioTransport`).

import { Type } from "@earendil-works/pi-ai";
import { defineTool, type ToolRegistration } from "@earendil-works/pi-durable";
import type { CodemodeNamespace } from "@earendil-works/pi-durable/codemode";
import { McpClient, StdioTransport, StreamableHttpTransport, toLlmContent } from "@earendil-works/pi-mcp";
import { authorize, authProviderFor, needsAuth, oauthProvider } from "./oauth.ts";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { agentDir, agentMcp, agents, agentEnv, isProject } from "../config.ts";
import { projects } from "../builtin/projects.ts";
import { quote, sshOptions } from "../machine/ssh.ts";

/** The servers as code mode's namespaces (`mcp__<server>__`), with what each says of itself; the
 *  codemode extension reads this list as it is at each script (`extensions.ts`). One list for every
 *  agent (Pi's extension takes one), rebuilt from `namespaces` whenever an agent's servers connect. */
export const NAMESPACES: CodemodeNamespace[] = [];
/** Each server's namespace as it last said it, key "<agent>/<server>": kept while it reconnects,
 *  dropped once its agent's servers connect without it. */
const namespaces = new Map<string, CodemodeNamespace>();

/** NAMESPACES as `namespaces` has them now, one per prefix. */
function syncNamespaces(): void {
	const byPrefix = new Map<string, CodemodeNamespace>();
	for (const n of namespaces.values()) if (!byPrefix.has(n.prefix)) byPrefix.set(n.prefix, n);
	NAMESPACES.splice(0, NAMESPACES.length, ...byPrefix.values());
}

const sanitize = (s: string) => s.replace(/[^A-Za-z0-9_]/g, "_");

export function mcpName(server: string, tool: string): string {
	const full = `mcp__${sanitize(server)}__${sanitize(tool)}`;
	if (full.length <= 64) return full;
	const suffix = "_" + createHash("sha256").update(full).digest("hex").slice(0, 8);
	return full.slice(0, 64 - suffix.length) + suffix;
}

interface Server {
	agent: string;
	name: string;
	client?: McpClient;
	tools: ToolRegistration[];
	/** Each tool's exposure, by its name here. */
	exposures: Map<string, string>;
	error?: string;
	/** It asked for OAuth: "Authorize" signs in. */
	needsAuth?: boolean;
	/** When it was last tried (ms): one that failed is tried again a while later (`ensureMcp`). */
	tried?: number;
}

/** key: "<agent>/<server>" */
const servers = new Map<string, Server>();

/** Each server's tools as last listed, key "<agent>/<server>": declared while it reconnects. */
const known = new Map<string, ToolRegistration[]>();

/**
 * An agent's MCP tools. A server that is down or reconnecting keeps its tools declared as they were
 * last listed, each answering that it is reconnecting. For a direct tool that is the cache: direct
 * tools sit at the head of every request, and a list that shrinks and grows back rewrites it, the
 * model server's prefix cache with it (on a local model, a minute of prefill each time). A script's
 * or a deferred tool is not in the request; its stand-in says the server is reconnecting where the
 * call would otherwise find no such tool. A server taken out of mcp.json loses them.
 */
export const mcpTools = (agent: string): ToolRegistration[] =>
	[...servers.entries()]
		.filter(([, s]) => s.agent === agent)
		.flatMap(([key, s]) => {
			if (s.tools.length) {
				known.set(key, s.tools);
				return s.tools;
			}
			return (known.get(key) ?? []).map((t: any) => ({
				...t,
				execute: async () => {
					throw new Error(`MCP server "${s.name}" is not connected now (${s.error ?? "reconnecting"}); try again in a moment, or go on without it`);
				},
			}));
		});
// `connected` is the client being alive after a try; `error` is why the try failed. A server with
// no entry at all was never tried — nobody called it yet — which is NOT the same as working.
export const mcpStatus = (agent?: string) =>
	[...servers.values()].filter((s) => !agent || s.agent === agent).map((s) => ({ agent: s.agent, name: s.name, tools: s.tools.length, error: s.error, connected: Boolean(s.client) && !s.error, needsAuth: !!s.needsAuth }));

/** The /browser insert's server: its tools are the insert's agent tool, offered to the model as the
 *  terminal's ssh_run is (an insert has a tool that acts on it), unless its mcp.json says otherwise. */
const INSERT_SERVER = "browser";

/** A server's exposure when no rule names a tool: its own, else the insert's server direct, else codemode. */
const defaultExposure = (spec: any, server: string): string => spec.exposure ?? (server === INSERT_SERVER ? "direct" : "codemode");

function exposureOf(spec: any, server: string, tool: string): string {
	const rules: Record<string, string> = spec.toolExposure ?? {};
	if (rules[tool]) return rules[tool] === "codemode-deferred" ? "codemode" : rules[tool];
	for (const [pattern, e] of Object.entries(rules))
		if (pattern.includes("*") && new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`).test(tool)) return e;
	return defaultExposure(spec, server);
}

/**
 * An agent's servers whose tools are behind code mode, from its mcp.json as configured (not as
 * connected: the prompt names them before they connect): enabled, and code mode their default or
 * some tool's (`toolExposure`). The prompt says these are reached from a script (`prompt.ts`).
 */
export function scriptedServers(agent: string): string[] {
	return Object.entries(agentMcp(agent))
		.filter(([name, spec]: [string, any]) => {
			if (!spec || spec.enabled === false) return false;
			const all = [defaultExposure(spec, name), ...Object.values<string>(spec.toolExposure ?? {})];
			return all.some((e) => String(e).startsWith("codemode"));
		})
		.map(([name]) => name);
}

const home = (v: unknown) => (typeof v === "string" ? v.replace(/^~(?=\/|$)/, os.homedir()) : v);
/** `${NAME}` in values comes from the stored keys and the environment, as in Pi's mcp.json. */
const fill = (v: unknown, env: Record<string, string>): any =>
	typeof v === "string" ? v.replace(/\$\{(\w+)\}/g, (_m, k) => env[k] ?? "") : Array.isArray(v) ? v.map((x) => fill(x, env)) : v && typeof v === "object" ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, fill(x, env)])) : v;

const connected = new Set<string>();
/** mcp.json's modification time when an agent's servers connected: an edit reconnects them. */
const connectedAt = new Map<string, number>();
const mcpMtime = (agent: string): number => {
	try {
		return fs.statSync(path.join(agentDir(agent), "mcp.json")).mtimeMs;
	} catch {
		return 0;
	}
};

/** Connects the servers of one agent (or of every agent), replacing what was connected before. */
export async function connectMcp(agent?: string, timeoutMs = 30000): Promise<void> {
	for (const name of agent ? [agent] : agents()) {
		connected.add(name);
		connectedAt.set(name, mcpMtime(name));
		await connectAgent(name, timeoutMs);
	}
}

/** One connected server as the evaluator sees it: its tools' names and how they are reached. */
export interface ServerTools {
	name: string;
	/** Its tools' common start, `mcp__<server>__` as Pi names them. */
	prefix: string;
	/** Its tools' names here. */
	tools: string[];
	/** Those the model may be offered with tool_search (`deferred`). */
	loadable: string[];
	/** Some of them are the scripts' alone (`codemode`). */
	scripted: boolean;
}

/** An agent's connected servers and how the model reaches their tools. */
export function mcpServerTools(agent: string): ServerTools[] {
	return [...servers.values()]
		.filter((s) => s.agent === agent && s.client && !s.error)
		.map((s) => {
			const tools = s.tools.map((t: any) => String(t.name ?? t.definition?.name ?? ""));
			return { name: s.name, prefix: `mcp__${sanitize(s.name)}__`, tools, loadable: tools.filter((t) => s.exposures.get(t) === "deferred"), scripted: tools.some((t) => s.exposures.get(t) === "codemode") };
		});
}

/** How a line for the model names a server's tools: in a script, or as its own tools. */
export function reachOf(s: ServerTools): string {
	return s.scripted ? `"${s.name}" (tools.${s.prefix}* in a codemode script)` : `"${s.name}" (${s.prefix}*)`;
}

/** The agent has tools behind tool_search (a `deferred` server): only then is tool_search offered. */
export function hasDeferred(agent: string): boolean {
	return [...servers.values()].some((s) => s.agent === agent && [...s.exposures.values()].includes("deferred"));
}

/** A server that failed is tried again with a message this long after, no sooner. */
const RETRY_MS = 60_000;
/** How long such a retry may hold the message up. */
const RETRY_TIMEOUT_MS = 5_000;

/**
 * An agent's servers connect with its first session, not all at the server's start; again when its
 * mcp.json changed. A server that did not answer (down, restarting) is tried again with a later
 * message, a minute apart: before, one that failed once stayed out until mcp.json changed, and
 * its tools were missing from every thread (the browser on dev, down when the agent started).
 */
export async function ensureMcp(agent: string): Promise<boolean> {
	if (!connected.has(agent) || connectedAt.get(agent) !== mcpMtime(agent)) {
		await connectMcp(agent);
		return true;
	}
	const failed = [...servers.values()].filter((s) => s.agent === agent && !s.client && !s.needsAuth && Date.now() - (s.tried ?? 0) >= RETRY_MS);
	if (!failed.length) return false;
	const env = agentEnv(agent);
	const specs = agentMcp(agent);
	await Promise.all(failed.map((s) => connectServer(agent, s.name, specs[s.name], env, RETRY_TIMEOUT_MS)));
	return failed.some((s) => servers.get(`${agent}/${s.name}`)?.client);
}

async function connectAgent(agent: string, timeoutMs: number): Promise<void> {
	await Promise.all([...servers.entries()].filter(([, s]) => s.agent === agent).map(([k, s]) => (servers.delete(k), s.client?.close().catch(() => {}))));
	const env = agentEnv(agent);
	const specs = agentMcp(agent);
	// A server no longer in its mcp.json (or turned off) is no namespace of scripts any more.
	for (const key of namespaces.keys()) {
		const [owner, name] = [key.slice(0, key.indexOf("/")), key.slice(key.indexOf("/") + 1)];
		if (owner === agent && (!specs[name] || specs[name].enabled === false)) namespaces.delete(key);
	}
	syncNamespaces();
	await Promise.all(Object.entries(specs).map(([name, raw]: [string, any]) => connectServer(agent, name, raw, env, timeoutMs)));
}

/** One server of an agent, connected anew: its tools, or why not. */
async function connectServer(agent: string, name: string, raw: any, env: Record<string, string>, timeoutMs: number): Promise<void> {
	if (!raw || raw.enabled === false) return;
	await servers.get(`${agent}/${name}`)?.client?.close().catch(() => {});
	const spec = fill(raw, env);
	const server: Server = { agent, name, tools: [], exposures: new Map(), tried: Date.now() };
	servers.set(`${agent}/${name}`, server);
	try {
		const client = new McpClient({ name: "bench", version: "1.0.0" });
		const transport = spec.url
			? httpTransport(agent, name, spec)
			: stdioTransport(agent, spec, env);
		await withTimeout(client.connect(transport), timeoutMs, "connect timed out");
		const prefix = `mcp__${sanitize(name)}__`;
		const own = { name, prefix, description: client.instructions?.trim().slice(0, 600) || client.serverInfo?.title || undefined };
		namespaces.set(`${agent}/${name}`, own);
		syncNamespaces();
		// Connected only once it listed its tools: one that times out here is tried again (`ensureMcp`).
		const listed = await withTimeout(client.listTools(), timeoutMs, "tools/list timed out").catch(async (e) => {
			await client.close().catch(() => {});
			throw e;
		});
		server.client = client;
		server.tools = listed
			.map((t: any) => ({ t, exposure: exposureOf(spec, name, t.name).replace("codemode-deferred", "codemode") }))
			.filter(({ exposure }) => exposure !== "hidden")
			.map((x) => (server.exposures.set(mcpName(name, x.t.name), x.exposure), x))
			.map(({ t, exposure }) =>
				defineTool({
					name: mcpName(name, t.name),
					description: t.description || `${t.name} (MCP server ${name})`,
					parameters: Type.Unsafe({ ...t.inputSchema, type: "object", properties: t.inputSchema?.properties ?? {} }),
					deferred: exposure !== "direct",
					...(exposure === "codemode" ? { callers: ["tools"] } : {}),
					// A script gets the CallToolResult whole (its structuredContent too); the model, its text.
					structuredOutputSchema: Type.Unknown(),
					execute: async (args: any, _api: any, context: any) => {
						const result = await client.callTool(t.name, args ?? {}, { signal: context?.abortSignal, timeout: (spec.timeout ?? 60) * 1000 } as any);
						return { output: toLlmContent(result) as any, structuredOutput: result as any, ...(result.isError ? { isError: true } : {}) };
					},
				} as any),
			);
	} catch (e) {
		server.needsAuth = Boolean(spec.url) && needsAuth(e);
		server.error = server.needsAuth ? "needs signing in" : explain(String((e as Error)?.message ?? e));
		console.error(`[mcp] ${agent}/${name}: ${server.error}`);
	}
}

/** A command server's transport. A project worked on a server runs it there, in its folder, through
 * the same SSH as its language servers (`lsp/lsp.ts`): it acts on that server's files and programs.
 * Only its own `env` goes along; `"local": true` keeps one on this machine (one that signs in
 * through a browser here, or needs a program the server lacks). */
function stdioTransport(agent: string, spec: any, env: Record<string, string>) {
	const p = isProject(agent) ? projects.get(agent.slice(8)) : undefined;
	if (p?.machine && !spec.local) {
		const vars = Object.entries(spec.env ?? {}).map(([k, v]) => `export ${k}=${quote(String(v))}; `).join("");
		const dir = spec.cwd || p.folder;
		const run = `${vars}${dir ? `cd ${quote(dir)} && ` : ""}exec ${[spec.command, ...(spec.args ?? [])].map((a: unknown) => quote(String(a))).join(" ")}`;
		// A login shell: node, npx and the like are found as in a terminal there.
		return new StdioTransport({ command: process.env.BENCH_SSH ?? "ssh", args: [...sshOptions(p.machine), `bash -lc ${quote(run)}`], env: process.env } as any);
	}
	return new StdioTransport({ command: home(spec.command) as string, args: (spec.args ?? []).map(home), env: { ...env, ...(spec.env ?? {}) }, cwd: home(spec.cwd) as string | undefined } as any);
}

/** An HTTP server's transport: with its own headers as given; without an Authorization header,
 * through OAuth (kept credentials, refreshed; see mcp-oauth.ts). */
function httpTransport(agent: string, name: string, spec: any, provider = oauthProvider(agent, name, spec.url, spec.oauth)) {
	const headers = spec.headers ?? {};
	const own = Object.keys(headers).some((h) => h.toLowerCase() === "authorization");
	return new StreamableHttpTransport({ url: spec.url, headers, ...(own ? {} : { authProvider: authProviderFor(provider) }) } as any);
}

/** Sign in to an agent's HTTP MCP server: the page to open in the browser ("" when no sign-in was needed). */
export async function authorizeServer(agent: string, name: string, connected?: () => void): Promise<string> {
	let spec = fill(agentMcp(agent)[name], agentEnv(agent));
	if (!spec?.url) throw new Error(`no HTTP MCP server "${name}"`);
	// Its own Authorization header the server refuses (an expired key): signing in replaces it with
	// OAuth, so the header goes from mcp.json.
	const header = Object.keys(spec.headers ?? {}).find((h) => h.toLowerCase() === "authorization");
	if (header) {
		const client = new McpClient({ name: "bench", version: "1.0.0" });
		try {
			await withTimeout(client.connect(httpTransport(agent, name, spec)), 30000, "connect timed out");
			await client.close().catch(() => {});
			await connectMcp(agent);
			return "";
		} catch (e) {
			if (!needsAuth(e)) throw e;
		}
		dropHeader(agent, name, header);
		const { [header]: _, ...rest } = spec.headers;
		spec = { ...spec, headers: rest };
	}
	return authorize(
		agent,
		name,
		spec.url,
		spec.oauth,
		async (provider) => {
			const client = new McpClient({ name: "bench", version: "1.0.0" });
			await withTimeout(client.connect(httpTransport(agent, name, spec, provider)), 30000, "connect timed out");
			await client.close().catch(() => {});
		},
		async () => {
			await connectMcp(agent);
			connected?.();
		},
	);
}

/** Take a header out of a server's entry in the agent's own mcp.json (as written, not filled). */
function dropHeader(agent: string, name: string, header: string): void {
	const file = path.join(agentDir(agent), "mcp.json");
	const doc = JSON.parse(fs.readFileSync(file, "utf8"));
	const entry = doc.mcpServers?.[name];
	if (!entry?.headers) return;
	delete entry.headers[header];
	if (!Object.keys(entry.headers).length) delete entry.headers;
	fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n");
}

/** `p`, or `message` as an error after `ms`; the timer goes with whichever comes first. */
function withTimeout<T>(p: Promise<T>, ms: number, message: string): Promise<T> {
	let timer: NodeJS.Timeout | undefined;
	const late = new Promise<T>((_, reject) => (timer = setTimeout(() => reject(new Error(message)), ms)));
	return Promise.race([p, late]).finally(() => clearTimeout(timer));
}

export async function closeMcp(): Promise<void> {
	await Promise.all([...servers.values()].map((s) => s.client?.close().catch(() => {})));
	servers.clear();
	namespaces.clear();
	syncNamespaces();
}

/** A server's error as the person can act on it. */
export function explain(error: string): string {
	// Some services (Figma) let no client register itself for OAuth: one made by hand is needed.
	if (/dynamic client registration failed/i.test(error))
		return `${error} — this service does not let Bench register itself for sign-in: make an OAuth app in the service and put its id and secret in the server's config as "oauth": {"client_id": "…", "client_secret": "…"}`;
	return error;
}
