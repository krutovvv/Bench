// MCP servers, per agent: the agent's mcp.json (`{ "mcpServers": { name: { command, args, env, cwd }
// | { url, headers } } }`, Pi's format), through the official client @earendil-works/pi-mcp. Tools
// are named mcp__<server>__<tool>, as in Pi. `exposure` as in Pi's docs/mcp.md: "direct" (declared
// to the model), "deferred" (found with tool_search), "hidden" (not reachable); "codemode", Pi's
// default, is treated as deferred here. `toolExposure` overrides it per tool (`*` patterns).
// Without `exposure`, a server with many tools is deferred and a small one direct. A project worked
// on a server runs its command servers there (`stdioTransport`).

import { Type } from "@earendil-works/pi-ai";
import { defineTool, type ToolRegistration } from "@earendil-works/pi-durable";
import { McpClient, StdioTransport, StreamableHttpTransport, toLlmContent } from "@earendil-works/pi-mcp";
import { authorize, authProviderFor, needsAuth, oauthProvider } from "./oauth.ts";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { agentDir, agentMcp, agents, agentEnv, isProject } from "../config.ts";
import { projects } from "../builtin/projects.ts";
import { quote, sshOptions } from "../machine/ssh.ts";

/** Without `exposure`: servers with more tools than this are deferred behind tool_search. */
const DEFER_OVER = 12;

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
 * last listed, each answering that it is reconnecting: the tools sit at the head of every request,
 * and a list that shrinks and grows back rewrites it, the model server's prefix cache with it (on a
 * local model, a minute of prefill each time). A server taken out of mcp.json loses them.
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

function exposureOf(spec: any, tool: string, count: number): string {
	const rules: Record<string, string> = spec.toolExposure ?? {};
	if (rules[tool]) return rules[tool];
	for (const [pattern, e] of Object.entries(rules))
		if (pattern.includes("*") && new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`).test(tool)) return e;
	return spec.exposure ?? (count > DEFER_OVER ? "deferred" : "direct");
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

/** An agent's connected servers and their tools' names (the evaluator's view of what a turn can use). */
export function mcpServerTools(agent: string): { name: string; tools: string[] }[] {
	return [...servers.values()].filter((s) => s.agent === agent && s.client && !s.error).map((s) => ({ name: s.name, tools: s.tools.map((t: any) => String(t.name ?? t.definition?.name ?? "")) }));
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
	await Promise.all(Object.entries(agentMcp(agent)).map(([name, raw]: [string, any]) => connectServer(agent, name, raw, env, timeoutMs)));
}

/** One server of an agent, connected anew: its tools, or why not. */
async function connectServer(agent: string, name: string, raw: any, env: Record<string, string>, timeoutMs: number): Promise<void> {
	if (!raw || raw.enabled === false) return;
	await servers.get(`${agent}/${name}`)?.client?.close().catch(() => {});
	const spec = fill(raw, env);
	const server: Server = { agent, name, tools: [], tried: Date.now() };
	servers.set(`${agent}/${name}`, server);
	try {
		const client = new McpClient({ name: "bench", version: "1.0.0" });
		const transport = spec.url
			? httpTransport(agent, name, spec)
			: stdioTransport(agent, spec, env);
		await withTimeout(client.connect(transport), timeoutMs, "connect timed out");
		server.client = client;
		const listed = await withTimeout(client.listTools(), timeoutMs, "tools/list timed out");
		server.tools = listed
			.map((t: any) => ({ t, exposure: exposureOf(spec, t.name, listed.length) }))
			.filter(({ exposure }) => exposure !== "hidden")
			.map(({ t, exposure }) =>
				defineTool({
					name: mcpName(name, t.name),
					description: t.description || `${t.name} (MCP server ${name})`,
					parameters: Type.Unsafe({ ...t.inputSchema, type: "object", properties: t.inputSchema?.properties ?? {} }),
					deferred: exposure !== "direct",
					execute: async (args: any, _api: any, context: any) => {
						const result = await client.callTool(t.name, args ?? {}, { signal: context?.abortSignal, timeout: (spec.timeout ?? 60) * 1000 } as any);
						return { output: toLlmContent(result) as any, ...(result.isError ? { isError: true } : {}) };
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

function withTimeout<T>(p: Promise<T>, ms: number, message: string): Promise<T> {
	return Promise.race([p, new Promise<T>((_, reject) => setTimeout(() => reject(new Error(message)), ms))]);
}

export async function closeMcp(): Promise<void> {
	await Promise.all([...servers.values()].map((s) => s.client?.close().catch(() => {})));
	servers.clear();
}

/** A server's error as the person can act on it. */
export function explain(error: string): string {
	// Some services (Figma) let no client register itself for OAuth: one made by hand is needed.
	if (/dynamic client registration failed/i.test(error))
		return `${error} — this service does not let Bench register itself for sign-in: make an OAuth app in the service and put its id and secret in the server's config as "oauth": {"client_id": "…", "client_secret": "…"}`;
	return error;
}
