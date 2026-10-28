// A real agent server for a test: started in its own home, spoken to as Bench speaks to it —
// typed JSON-RPC calls over the WebSocket (crates/protocol `Methods`), its events kept in order,
// REST with the token.

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import WebSocket from "ws";
import type { Event, Methods, ThreadEvent } from "../src/protocol.gen.ts";

export interface TestServer {
	port: number;
	/** What the server wrote to stderr so far. */
	log(): string;
	call<M extends keyof Methods>(method: M, params: Methods[M][0]): Promise<Methods[M][1]>;
	rest(method: string, p: string, body?: unknown): Promise<any>;
	/** Every event so far, in order. */
	events: Event[];
	/** The events of one thread so far. */
	of(thread: string): ThreadEvent[];
	/** Resolves once `pred` holds over the events, or fails after `ms`. */
	until(pred: () => boolean, ms?: number): Promise<void>;
	close(): void;
}

export async function startServer(home: string, opts: { websocket?: boolean } = {}): Promise<TestServer> {
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], {
		env: { ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1", PI_OFFLINE: "1" },
		cwd: path.join(import.meta.dirname, ".."),
		stdio: ["ignore", "ignore", "pipe"],
	});
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	const file = path.join(home, "server.json");
	for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
	assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
	const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));
	const rest = (method: string, p: string, body?: unknown) =>
		fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "X-Bench-Token": token, "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
	const events: Event[] = [];
	let ws: WebSocket | undefined;
	const wait = new Map<number, (m: any) => void>();
	if (opts.websocket !== false) {
		ws = new WebSocket(`ws://127.0.0.1:${port}/api/ws?token=${token}`);
		await new Promise((res, rej) => (ws!.on("open", res), ws!.on("error", rej)));
		ws.on("message", (d) => {
			const m = JSON.parse(String(d));
			if (m.method === "event") events.push(m.params);
			else wait.get(m.id)?.(m);
			wait.delete(m.id);
		});
	}
	let n = 0;
	return {
		port,
		log: () => err,
		call: (method, params) =>
			new Promise((res, rej) => {
				const id = ++n;
				wait.set(id, (m) => (m.error ? rej(new Error(m.error.message)) : res(m.result)));
				ws!.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
			}),
		rest,
		events,
		of: (thread) => events.flatMap((e) => (e.type === "thread" && e.thread === thread ? [e.event] : [])),
		until: async (pred, ms = 60000) => {
			const end = Date.now() + ms;
			while (!pred()) {
				if (Date.now() > end) throw new Error(`timed out:\n${err}`);
				await new Promise((r) => setTimeout(r, 20));
			}
		},
		close: () => {
			ws?.close();
			child.kill();
		},
	};
}

/** A message's params with the defaults Bench fills in. */
export const message = (thread: string, text: string): Methods["thread.send"][0] => ({ thread, text, context: "", request: null, queue: false, effort: null });
