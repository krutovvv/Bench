// Code mode as Pi means it, on a real harness with a stub model: an MCP server's tools behind code
// mode (Pi's default exposure) are not offered to the model, a script calls them and gets the
// CallToolResult whole; the script asks the evaluator (`bench/evaluator`) about each record, the
// gear's main server down and its fallback answering; with the project's evaluator off, the
// script's question is refused with the reason. And the gear's list of servers: main, fallback.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-codemode-mcp-"));
process.env.BENCH_HOME = home;

const { readAgent, saveProvider, setSetting, writeAgent, writeAgentMcp } = await import("../src/config.ts");
const { Runtime } = await import("../src/runtime/runtime.ts");
const { closeMcp } = await import("../src/mcp/mcp.ts");
const { deleteServer, evaluatorView, makeFallback, makeMain, saveServer } = await import("../src/judge/models.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

// An MCP server of three tickets, its records as structuredContent.
const mcp = path.join(home, "crm.mjs");
fs.writeFileSync(
	mcp,
	`import readline from "node:readline";
const tickets = ["the site is down", "a typo on the about page", "checkout is down for everyone"];
readline.createInterface({ input: process.stdin }).on("line", (l) => {
	const m = JSON.parse(l);
	const reply = (result) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: m.id, result }) + "\\n");
	if (m.method === "initialize") reply({ protocolVersion: m.params.protocolVersion, capabilities: { tools: {} }, serverInfo: { name: "crm", version: "1" }, instructions: "The support desk's tickets." });
	else if (m.method === "tools/list") reply({ tools: [{ name: "tickets", description: "The open tickets", inputSchema: { type: "object" } }] });
	else if (m.method === "tools/call") reply({ content: [{ type: "text", text: tickets.join("\\n") }], structuredContent: { tickets } });
});
`,
);

// A System One server: a ticket is urgent when it says "down".
const judged: string[] = [];
const evaluator = http.createServer((req, res) => {
	let data = "";
	req.on("data", (c) => (data += c));
	req.on("end", () => {
		const body = JSON.parse(data);
		res.setHeader("Content-Type", "application/json");
		// The agent's own points ask too (the thread's opening message): those are answered unsure.
		const answers = Object.fromEntries(
			Object.entries(body.questions).map(([id, q]: [string, any]) => [
				id,
				id === "urgent" ? { type: "noul", noul: /down/.test(JSON.stringify(body.state)) ? 0.9 : 0.1 } : q.type === "noul" ? { type: "noul", noul: 0.5 } : { type: "choice", choice: Object.keys(q.criteria)[0], confidence: 0, probabilities: Object.fromEntries(Object.keys(q.criteria).map((k) => [k, 1 / Object.keys(q.criteria).length])) },
			]),
		);
		if ("urgent" in answers) judged.push(JSON.stringify(body.state));
		res.end(JSON.stringify({ answers }));
	});
});
await new Promise<void>((r) => evaluator.listen(0, "127.0.0.1", r));

const SCRIPT = `const r = await tools.mcp__crm__tickets({});
const list = r.structuredContent.tickets;
const m = await models.getModelOfType("classifier", "bench", "evaluator");
const out = await Promise.all(list.map((t) => models.classify(m, { state: { ticket: t }, questions: { urgent: { type: "bool", instructions: "Is it urgent?", criteria: { true: "urgent", false: "it can wait" } } } })));
text(list.filter((t, i) => out[i].answers.urgent?.probability > 0.5).join(" | "));`;

const chunk = (delta: any, finish: string | null = null) => `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;
const offered: string[][] = [];
const results: string[] = [];
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		res.writeHead(200, { "content-type": "text/event-stream" });
		if (body.includes("You name chat sessions")) return res.end(chunk({ role: "assistant", content: "Tickets" }) + chunk({}, "stop") + "data: [DONE]\n\n");
		const json = JSON.parse(body);
		const last = json.messages.at(-1);
		if (last.role === "tool") {
			results.push(typeof last.content === "string" ? last.content : JSON.stringify(last.content));
			return res.end(chunk({ role: "assistant", content: "Done." }) + chunk({}, "stop") + "data: [DONE]\n\n");
		}
		offered.push((json.tools ?? []).map((t: any) => t.function?.name));
		res.end(chunk({ role: "assistant", tool_calls: [{ index: 0, id: "call_cm", type: "function", function: { name: "codemode", arguments: JSON.stringify({ code: SCRIPT }) } }] }) + chunk({}, "tool_calls") + "data: [DONE]\n\n");
	});
});
await new Promise<void>((r) => model.listen(0, "127.0.0.1", r));

after(async () => {
	model.close();
	evaluator.close();
	await closeMcp();
	fs.rmSync(home, { recursive: true, force: true });
});

const until = async (what: () => boolean, ms = 20000) => {
	const end = Date.now() + ms;
	while (!what()) {
		if (Date.now() > end) throw new Error("timed out");
		await new Promise((r) => setTimeout(r, 20));
	}
};

test("the gear's servers: the first is main, one is the fallback, a deleted main gives way", () => {
	setSetting("judge", { enabled: true, base_url: "http://old", model: "jev-latest" });
	assert.deepEqual(evaluatorView(), { servers: [{ id: "evaluator", name: "Evaluator", base_url: "http://old", model: "jev-latest", api_key: undefined }], main: "evaluator", fallback: null }, "the earlier shape reads as one server");
	setSetting("judge", {});
	const a = saveServer({ name: "Local Jev", base_url: "http://a" });
	const b = saveServer({ name: "OpenRouter", base_url: "http://b", api_key: "k" });
	assert.deepEqual([a, b, evaluatorView().main], ["local-jev", "openrouter", "local-jev"]);
	makeFallback(b);
	assert.throws(() => makeFallback(a), /own fallback/);
	saveServer({ id: b, name: "OpenRouter", base_url: "http://b2" });
	assert.equal(evaluatorView().servers[1].api_key, "k", "an empty key keeps the saved one");
	makeMain(b);
	assert.deepEqual([evaluatorView().main, evaluatorView().fallback], [b, null], "the fallback made main is no longer the fallback");
	deleteServer(b);
	assert.deepEqual([evaluatorView().main, evaluatorView().servers.length], [a, 1]);
});

test("an MCP server behind code mode: a script reads its records whole and asks the evaluator of each", async () => {
	saveProvider({ id: "stub", name: "Stub", base_url: `http://127.0.0.1:${(model.address() as any).port}/v1`, api_key: "k", api_mode: "chat_completions", models: { m: {} } });
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m", evaluator: { enabled: true } } as any);
	writeAgentMcp("default", { crm: { command: process.execPath, args: [mcp] } });
	// The main server is down; the fallback answers.
	setSetting("judge", { servers: [{ id: "down", base_url: "http://127.0.0.1:9" }, { id: "up", base_url: `http://127.0.0.1:${(evaluator.address() as any).port}` }], main: "down", fallback: "up" });
	let done = 0;
	const rt = new Runtime(() => {});
	rt.runs.add((_id, e) => {
		if (e.kind === "complete") done++;
	});
	await rt.open();
	const { row } = await rt.create("default", home, "local");

	await rt.submit(row.id, "Which tickets are urgent?");
	await until(() => done === 1);
	assert.ok(offered[0].includes("codemode"));
	assert.ok(!offered[0].some((n) => n.startsWith("mcp__crm__")), "the server's tools are the scripts', not offered to the model");
	assert.match(results.at(-1)!, /the site is down \| checkout is down for everyone/);
	assert.equal(judged.length, 3, "one question a ticket, answered by the fallback");
	const live = [...(rt as any).byConversation.values()][0];
	assert.ok((live.usage.judge ?? 0) >= 3, "the script's questions count in the thread's Evaluator calls");

	// The project's evaluator off: the script's question is refused, with the reason.
	writeAgent("default", { ...readAgent("default"), evaluator: undefined });
	await rt.submit(row.id, "Again");
	await until(() => done === 2);
	assert.match(results.at(-1)!, /evaluator is off in this project/);
	assert.equal(judged.length, 3);
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
