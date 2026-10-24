// A Bench stand-in for trying the server by hand: connects like Bench does in token mode, makes
// a session, sends one prompt and prints the events it gets back.
//
//   node scripts/probe.ts "What time is it?"            (uses ~/.bench-agent/server.json)
//   node scripts/probe.ts --session <id> "And now?"
//   PROBE_ANSWER="yes" node scripts/probe.ts "..."     answers a clarify/approval with this

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import WebSocket from "ws";

const home = process.env.BENCH_HOME ?? path.join(os.homedir(), ".bench");
const { port, token } = JSON.parse(fs.readFileSync(path.join(home, "server.json"), "utf8"));
const argv = process.argv.slice(2);
const si = argv.indexOf("--session");
let session = si >= 0 ? argv.splice(si, 2)[1] : "";
const prompt = argv.join(" ") || "Say hi in five words.";

const ws = new WebSocket(`ws://127.0.0.1:${port}/api/ws?token=${token}`);
let next = 1;
const waiting = new Map<number, (v: any) => void>();
const call = (method: string, params: any) =>
	new Promise<any>((resolve, reject) => {
		const id = next++;
		waiting.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)));
		ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
	});

ws.on("message", (data) => {
	const m = JSON.parse(String(data));
	if (m.id !== undefined) return waiting.get(m.id)?.(m);
	const { type, payload, session_id } = m.params;
	if (session_id !== session) return;
	if (type === "message.delta") process.stdout.write(payload.text);
	else if (type === "reasoning.delta") process.stdout.write(`\x1b[2m${payload.text}\x1b[0m`);
	else if (type === "tool.start") console.log(`\n[tool.start] ${payload.name} ${payload.context}`);
	else if (type === "tool.complete") console.log(`[tool.complete] ${payload.name} ${JSON.stringify(payload.result).slice(0, 300)}`);
	else if (type === "clarify.request" || type === "approval.request") {
		console.log(`\n[${type}] ${JSON.stringify(payload)}`);
		const answer = process.env.PROBE_ANSWER ?? (type === "approval.request" ? "once" : (payload.choices?.[0] ?? "yes"));
		void call(type === "clarify.request" ? "clarify.respond" : "approval.respond", { session_id, prompt_id: payload.prompt_id, answer, choice: answer });
	} else if (type === "message.complete") {
		console.log(`\n[message.complete] ${String(payload.text).length} chars`);
		setTimeout(() => process.exit(0), 300);
	} else console.log(`\n[${type}] ${JSON.stringify(payload).slice(0, 200)}`);
});

ws.on("open", async () => {
	console.log("profiles:", JSON.stringify((await call("profiles.list", {})).profiles.map((p: any) => p.name)));
	if (!session) session = (await call("session.create", { profile: process.env.PROBE_PROFILE ?? "default", cwd: process.cwd() })).session_id;
	console.log("session:", session);
	await call("prompt.submit", { session_id: session, text: prompt, profile: "default" });
});
ws.on("error", (e) => {
	console.error(e);
	process.exit(1);
});
setTimeout(() => (console.error("\n[probe] timeout"), process.exit(2)), Number(process.env.PROBE_TIMEOUT ?? 300) * 1000);
