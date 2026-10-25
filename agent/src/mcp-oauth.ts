// OAuth for HTTP MCP servers that ask for it ("MCP server requires authentication"), through
// pi-mcp's client (`@earendil-works/pi-mcp/oauth`: discovery, PKCE, dynamic client registration,
// refresh). Credentials are kept per agent and server in ~/.bench/mcp-oauth/, so a server signed in
// once stays signed in. Signing in: `authorize()` gives the page to open; the browser comes back to
// a callback on this machine, and the server is connected again.

import fs from "node:fs";
import path from "node:path";
import { adaptOAuthProvider, authorizeMcp, McpOAuthAuthorizationRequiredError, McpOAuthProvider, OAuthCallbackServer, type McpOAuthState } from "@earendil-works/pi-mcp/oauth";
import { AGENT_HOME } from "./config.ts";

/** Where the browser comes back after signing in: fixed, so a registered client stays valid. */
const CALLBACK_PORT = Number(process.env.BENCH_MCP_OAUTH_PORT ?? 47823);
const REDIRECT = `http://localhost:${CALLBACK_PORT}/callback`;

const file = (agent: string, name: string) => path.join(AGENT_HOME, "mcp-oauth", `${agent.replace(/[^\w.-]/g, "_")}__${name.replace(/[^\w.-]/g, "_")}.json`);

/** The page to open to sign in, per "agent/name", from the last connection that asked for it. */
const pending = new Map<string, URL>();

/**
 * `client`: a client registered with the service by hand ({"oauth": {"client_id", "client_secret"}}
 * in the server's config), for services that refuse dynamic registration (Figma).
 */
export function oauthProvider(agent: string, name: string, serverUrl: string, client?: { client_id?: string; client_secret?: string }): McpOAuthProvider {
	const f = file(agent, name);
	return new McpOAuthProvider({
		serverUrl,
		redirectUrl: REDIRECT,
		clientMetadata: { client_name: "Bench" },
		...(client?.client_id ? { clientId: client.client_id, ...(client.client_secret ? { clientSecret: client.client_secret } : {}) } : {}),
		store: {
			load: () => {
				try {
					return JSON.parse(fs.readFileSync(f, "utf8")) as McpOAuthState;
				} catch {
					return undefined;
				}
			},
			save: (state) => {
				fs.mkdirSync(path.dirname(f), { recursive: true });
				fs.writeFileSync(f, JSON.stringify(state), { mode: 0o600 });
			},
		},
		onRedirect: (url) => {
			pending.set(`${agent}/${name}`, url);
		},
	});
}

/** The transport option for a server that may need OAuth. */
export const authProviderFor = (provider: McpOAuthProvider) => adaptOAuthProvider(provider);

export const needsAuth = (e: unknown) => e instanceof McpOAuthAuthorizationRequiredError || /requires authentication|unauthorized|401/i.test(String((e as Error)?.message ?? e));

/** Whether a server has credentials kept. */
export const signedIn = (agent: string, name: string) => fs.existsSync(file(agent, name));

let callback: OAuthCallbackServer | undefined;

/**
 * Start signing in to a server: the page to open in the browser. `connect` makes a connection
 * attempt with the provider (it records the page); `reconnect` runs once the browser came back.
 */
export async function authorize(agent: string, name: string, serverUrl: string, client: { client_id?: string; client_secret?: string } | undefined, connect: (p: McpOAuthProvider) => Promise<unknown>, reconnect: () => Promise<void>): Promise<string> {
	const provider = oauthProvider(agent, name, serverUrl, client);
	pending.delete(`${agent}/${name}`);
	try {
		await connect(provider);
		// Connected without signing in (the kept credentials were enough).
		await reconnect();
		return "";
	} catch (e) {
		if (!needsAuth(e)) throw e;
	}
	const url = pending.get(`${agent}/${name}`);
	if (!url) throw new Error("the server asked for authentication but gave no sign-in page");
	if (!callback) callback = await OAuthCallbackServer.listen({ port: CALLBACK_PORT, redirectHost: "localhost", timeoutMs: 10 * 60_000 });
	const state = await provider.state();
	void callback
		.waitForCallback(state)
		.then(async ({ code }) => {
			await authorizeMcp(provider, { serverUrl, authorizationCode: code });
			await reconnect();
		})
		.catch((e) => console.error(`[mcp] ${agent}/${name} sign-in: ${(e as Error)?.message ?? e}`));
	return url.toString();
}
