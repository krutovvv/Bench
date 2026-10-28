// MCP servers, per agent: the agent's mcp.json (`{ "mcpServers": { name: { command, args, env, cwd }
// | { url, headers } } }`, Pi's format), through the official client @earendil-works/pi-mcp. Tools
// are named mcp__<server>__<tool>, as in Pi. `exposure` as in Pi's docs/mcp.md: "direct" (declared
// to the model), "deferred" (found with tool_search), "hidden" (not reachable); "codemode", Pi's
// default, is treated as deferred here. `toolExposure` overrides it per tool (`*` patterns).
// Without `exposure`, a server with many tools is deferred and a small one direct.

import { Type } from "@earendil-works/pi-ai";
import { defineTool, type ToolRegistration } from "@earendil-works/pi-durable";
import { McpClient, StdioTransport, StreamableHttpTransport, toLlmContent } from "@earendil-works/pi-mcp";
import { authorize, authProviderFor, needsAuth, oauthProvider } from "./oauth.ts";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { agentDir, agentMcp, agents, agentEnv } from "../config.ts";

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
}

/** key: "<agent>/<server>" */
const servers = new Map<string, Server>();

export const mcpTools = (agent: string): ToolRegistration[] => [...servers.values()].filter((s) => s.agent === agent).flatMap((s) => s.tools);
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

/** An agent's servers connect with its first session, not all at the server's start. */
export async function ensureMcp(agent: string): Promise<boolean> {
	if (connected.has(agent) && connectedAt.get(agent) === mcpMtime(agent)) return false;
	await connectMcp(agent);
	return true;
}

async function connectAgent(agent: string, timeoutMs: number): Promise<void> {
	await Promise.all([...servers.entries()].filter(([, s]) => s.agent === agent).map(([k, s]) => (servers.delete(k), s.client?.close().catch(() => {}))));
	const env = agentEnv(agent);
	await Promise.all(
		Object.entries(agentMcp(agent)).map(async ([name, raw]: [string, any]) => {
			if (!raw || raw.enabled === false) return;
			const spec = fill(raw, env);
			const server: Server = { agent, name, tools: [] };
			servers.set(`${agent}/${name}`, server);
			try {
				const client = new McpClient({ name: "bench", version: "1.0.0" });
				const transport = spec.url
					? httpTransport(agent, name, spec)
					: new StdioTransport({ command: home(spec.command) as string, args: (spec.args ?? []).map(home), env: { ...env, ...(spec.env ?? {}) }, cwd: home(spec.cwd) as string | undefined } as any);
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
		}),
	);
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
