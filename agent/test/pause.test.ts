// The person's pause (Runtime.pause/resume, the gate in assistant.ts's beforeTool) on a real Pi
// harness in a temporary BENCH_HOME. The stub model asks for one tool call and then answers: a
// held run must not start the call until it is let go, and a stop while held must not start it
// at all.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-pause-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";

const { saveProvider, writeAgent } = await import("../src/config.ts");
const { Runtime } = await import("../src/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

const chunk = (delta: any, finish: string | null = null) =>
	`data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;

// The run's requests: the first of a turn gets a tool call; the one after the tool's result gets an answer.
let requests = 0;
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		// The thread's title is asked on the side: it is not a step of the run.
		if (body.includes("You name chat sessions")) {
			res.writeHead(200, { "content-type": "text/event-stream" });
			res.end(chunk({ role: "assistant", content: "Look around" }) + chunk({}, "stop") + "data: [DONE]\n\n");
			return;
		}
		requests++;
		const afterTool = body.includes('"role":"tool"');
		res.writeHead(200, { "content-type": "text/event-stream" });
		if (afterTool) res.write(chunk({ role: "assistant", content: "Done." }) + chunk({}, "stop"));
		else
			res.write(
				chunk({ role: "assistant", tool_calls: [{ index: 0, id: `call_${requests}`, type: "function", function: { name: "ls", arguments: '{"path":"."}' } }] }) +
					chunk({}, "tool_calls"),
			);
		res.end("data: [DONE]\n\n");
	});
});
await new Promise<void>((r) => model.listen(0, "127.0.0.1", r));
const port = (model.address() as any).port;

after(() => {
	model.close();
	fs.rmSync(home, { recursive: true, force: true });
});

const until = async (what: () => boolean, ms = 5000) => {
	const end = Date.now() + ms;
	while (!what()) {
		if (Date.now() > end) throw new Error("timed out");
		await new Promise((r) => setTimeout(r, 20));
	}
};

test("pause: a held run starts no tool until resumed; its holds are timed; a stop while held starts nothing", async () => {
	saveProvider({ id: "stub", name: "Stub", base_url: `http://127.0.0.1:${port}/v1`, api_key: "k", api_mode: "chat_completions", models: { m: {} } });
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m" } as any);
	const holds: any[] = [];
	const tools: string[] = [];
	const runs: boolean[] = [];
	const rt = new Runtime((_sid, type, payload: any) => {
		if (type === "thread.pause") holds.push(structuredClone(payload.holds));
		if (type === "thread.run") runs.push(payload.running);
		if (type === "tool.started" || type === "tool.start") tools.push(payload.name);
	});
	await rt.open();
	const { row } = await rt.create("default", undefined, "local");
	const id = row.id;

	await rt.submit(id, "look around");
	rt.pause(id);
	assert.equal(rt.isPaused(id), true);
	assert.equal(holds.at(-1).length, 1);
	assert.equal(holds.at(-1)[0].to, undefined, "the hold is open");
	// The model asked for its call, but the call waits at the gate: no second request comes.
	await until(() => requests >= 1);
	await new Promise((r) => setTimeout(r, 300));
	assert.equal(requests, 1, "held: the tool did not run, so the model was not asked again");
	assert.equal(rt.isBusy(id), true, "the run lives on, standing still");
	assert.equal((await rt.thread(id)).holds.length, 1, "a reopened thread sees the hold");

	rt.resume(id);
	assert.equal(rt.isPaused(id), false);
	assert.ok(holds.at(-1)[0].to >= holds.at(-1)[0].from, "the hold is closed with its end");
	await until(() => runs.at(-1) === false);
	assert.equal(requests, 2, "let go: the tool ran and the model answered");

	// A stop while held: the waiting call is refused, the run ends, nothing is asked again.
	await rt.submit(id, "again");
	rt.pause(id);
	await until(() => requests >= 3);
	await new Promise((r) => setTimeout(r, 200));
	await rt.interrupt(id);
	await until(() => !rt.isBusy(id));
	await new Promise((r) => setTimeout(r, 200));
	assert.equal(requests, 3, "stopped while held: the call never ran");
	assert.equal(rt.isPaused(id), false);

	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
