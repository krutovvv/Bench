// bench-agent's server: Bench's protocol (crates/protocol) — JSON-RPC 2.0 over a WebSocket at
// /api/ws?token= (requests, and `event` notifications), REST under /api/ with the token in
// X-Bench-Token. A server's agent is always the build of the Bench that installed it, so the
// wire never speaks to an older one. Run as the machine's service, which Bench installs
// (src/client/service.rs), or by hand (`node src/server.ts`); one per home (`instance.ts`).
// Writes its port and token to ~/.bench/server.json.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { onMachine } from "./machine/sh.ts";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { WebSocketServer, type WebSocket } from "ws";
import pkg from "../package.json" with { type: "json" };
import { AGENT_HOME, migrateProfiles, migrateProjectModels } from "./config.ts";
import { migrateServerRows } from "./builtin/servers.ts";
import { expand, HttpError, readBody, type Dynamic, type Module, type Req, type Route } from "./http.ts";
import type { Event } from "./protocol.gen.ts";
import { connectMcp, mcpStatus } from "./mcp/mcp.ts";
import agents from "./modules/agents.ts";
import presetsModule from "./modules/presets.ts";
import audio from "./modules/audio.ts";
import backup from "./modules/backup.ts";
import commands from "./modules/commands.ts";
import cron from "./modules/cron.ts";
import { onJobsChanged } from "./builtin/cron.ts";
import files from "./modules/files.ts";
import lsp from "./modules/lsp.ts";
import projects from "./modules/projects.ts";
import settings from "./modules/settings.ts";
import specs from "./modules/specs.ts";
import threads from "./modules/threads.ts";
import { Runtime } from "./runtime/runtime.ts";
import { BUILD, capLog, holdHome } from "./instance.ts";
import { syncTelegram } from "./telegram.ts";

holdHome(AGENT_HOME);
capLog(AGENT_HOME);

const STATE = path.join(AGENT_HOME, "server.json");
const args = process.argv.slice(2);
const argPort = Number(args[args.indexOf("--port") + 1] || process.env.BENCH_AGENT_PORT || 0);

const prior = (() => {
	try {
		return JSON.parse(fs.readFileSync(STATE, "utf8"));
	} catch {
		return {};
	}
})();
const token: string = process.env.BENCH_AGENT_TOKEN || prior.token || randomBytes(32).toString("hex");

const sockets = new Set<WebSocket>();
/** An event to every connected Bench: a JSON-RPC notification whose params are the `Event`. */
function publish(event: Event): void {
	const frame = JSON.stringify({ jsonrpc: "2.0", method: "event", params: event });
	for (const ws of sockets) if (ws.readyState === ws.OPEN) ws.send(frame);
}

for (const name of migrateProfiles()) console.log(`[migrate] agent ${name} → agents/${name}`);
for (const row of migrateServerRows()) console.log(`[migrate] ${row} is a server's agent now, not a project row`);
for (const id of migrateProjectModels()) console.log(`[migrate] project ${id} follows the general model now (its settings.json in backups/)`);
const rt = new Runtime(publish);
/** How long Pi has to close when the agent stops; then it stops anyway. */
const SHUTDOWN_MS = 5000;
// A schedule changed: its project's environment did (the sidebar's mark follows).
onJobsChanged((agent) => publish({ type: "env_changed", project: agent.startsWith("project:") ? agent.slice("project:".length) : "general" }));
await rt.open();


// ---- the modules -----------------------------------------------------------

// Each module declares its RPC methods and REST routes (src/modules/*.ts); the server only
// collects them. A new area of the agent is a new module and a line here.
const modules: Module[] = [threads, commands, projects, backup, agents, presetsModule, cron, files, lsp, audio, settings, specs].map((m) => m({ rt, publish }));
const methods: Record<string, (params: any) => unknown> = Object.assign({}, ...modules.map((m) => m.rpc ?? {}));
const routes: Record<string, Route> = Object.assign({}, ...modules.map((m) => m.routes ?? {}));
const dynamic: Dynamic[] = modules.flatMap((m) => m.dynamic ?? []);

const server = http.createServer(async (req, res) => {
	const url = new URL(req.url ?? "/", "http://localhost");
	const send = (status: number, body: any) => {
		res.writeHead(status, { "Content-Type": "application/json" });
		res.end(JSON.stringify(body));
	};
	if (req.headers["x-bench-token"] !== token && url.searchParams.get("token") !== token) return send(401, { detail: "unauthorized" });
	try {
		const r = req as Req;
		r.query = url.searchParams;
		r.path = url.pathname;
		r.body = await readBody(req);
		if (req.method === "GET" && url.pathname === "/api/fs/download") {
			// A file of this machine, or of the project's machine over SSH (`files.ts`).
			const m = await onMachine(url.searchParams.get("machine"), url.searchParams.get("path") ?? "");
			const p = m?.path ?? expand(url.searchParams.get("path") ?? "");
			let buf: Buffer;
			if (m) {
				const r = await m.env.readBinaryFile(p, BACKGROUND_CONTEXT);
				if (!r.ok) return send(r.error.code === "not_found" ? 404 : 400, { detail: r.error.message });
				buf = Buffer.from(r.value).subarray(0, 50_000_000);
			} else buf = fs.readFileSync(p).subarray(0, 50_000_000);
			res.writeHead(200, { "Content-Type": "application/octet-stream" });
			return res.end(buf);
		}
		const fixed = routes[`${req.method} ${url.pathname}`];
		if (fixed) return send(200, await fixed(r));
		for (const [method, re, handler] of dynamic) {
			const m = url.pathname.match(re);
			if (m && method === req.method) return send(200, await handler(r, m));
		}
		send(404, { detail: `no route: ${req.method} ${url.pathname}` });
	} catch (e) {
		const status = e instanceof HttpError ? e.status : (e as any)?.code === "ENOENT" ? 404 : 500;
		send(status, { detail: String((e as Error)?.message ?? e) });
	}
});

const wss = new WebSocketServer({ noServer: true });
server.on("upgrade", (req, socket, head) => {
	const url = new URL(req.url ?? "/", "http://localhost");
	if (url.pathname !== "/api/ws" || url.searchParams.get("token") !== token) {
		socket.write("HTTP/1.1 403 Forbidden\r\n\r\n");
		return socket.destroy();
	}
	wss.handleUpgrade(req, socket, head, (ws) => {
		sockets.add(ws);
		ws.on("close", () => sockets.delete(ws));
		ws.on("message", async (data) => {
			let msg: any;
			try {
				msg = JSON.parse(String(data));
			} catch {
				return;
			}
			const handler = methods[msg.method];
			try {
				if (!handler) throw new Error(`method not found: ${msg.method}`);
				const result = await handler(msg.params ?? {});
				ws.send(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: result ?? {} }));
			} catch (e) {
				ws.send(JSON.stringify({ jsonrpc: "2.0", id: msg.id, error: { code: -32000, message: String((e as Error)?.message ?? e) } }));
			}
		});
	});
});

// ---- start ---------------------------------------------------------------

server.listen(argPort || prior.port || 0, "127.0.0.1", () => {
	const port = (server.address() as any).port;
	fs.mkdirSync(AGENT_HOME, { recursive: true });
	fs.writeFileSync(STATE, JSON.stringify({ port, token, pid: process.pid, version: pkg.version, build: BUILD }), { mode: 0o600 });
	console.log(`BENCH_AGENT_READY port=${port}`);
});
server.on("error", (e: any) => {
	// The saved port is taken: take any free one.
	if (e.code === "EADDRINUSE" && !argPort) server.listen(0, "127.0.0.1");
	else throw e;
});

void syncTelegram(rt).catch((e) => console.error("[telegram]", e));

if (!process.env.BENCH_AGENT_NO_MCP) void connectMcp("default").then(() => {
	rt.reloadMcp();
	console.log(`[mcp] ${mcpStatus().map((s) => `${s.agent}/${s.name}:${s.error ? "error" : s.tools}`).join(" ")}`);
});

for (const sig of ["SIGINT", "SIGTERM"])
	process.on(sig, async () => {
		// Pi counts a task cut off by a crash towards parking it, and a close not: without this every
		// restart counted against whatever was waiting, as a thread in the line (`runtime/streams.ts`).
		await Promise.race([rt.shutdown(), new Promise((done) => setTimeout(done, SHUTDOWN_MS))]);
		process.exit(0);
	});
