// Code mode (extensions.ts) on a real Pi harness in a temporary BENCH_HOME with a stub model:
// every thread is offered `codemode` beside its other tools, and one script calls several tools
// and returns only what it chose to print.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-codemode-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";

const { saveProvider, writeAgent } = await import("../src/config.ts");
const { Runtime } = await import("../src/runtime/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

const chunk = (delta: any, finish: string | null = null) =>
	`data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;

// The script a code-mode step runs: two tools, one printed line.
const SCRIPT = `const [a, b] = await Promise.all([tools.bash({ command: "echo alpha" }), tools.bash({ command: "echo beta" })]);
text(a.output.trim() + "+" + b.output.trim());`;

/** Each turn's first request: the tools it offered. */
const offered: string[][] = [];
const results: string[] = [];
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		res.writeHead(200, { "content-type": "text/event-stream" });
		if (body.includes("You name chat sessions")) {
			res.end(chunk({ role: "assistant", content: "Code" }) + chunk({}, "stop") + "data: [DONE]\n\n");
			return;
		}
		const json = JSON.parse(body);
		const last = json.messages.at(-1);
		if (last.role === "tool") {
			results.push(typeof last.content === "string" ? last.content : JSON.stringify(last.content));
			res.end(chunk({ role: "assistant", content: "Done." }) + chunk({}, "stop") + "data: [DONE]\n\n");
			return;
		}
		const names = (json.tools ?? []).map((t: any) => t.function?.name);
		offered.push(names);
		res.end(
			chunk({ role: "assistant", tool_calls: [{ index: 0, id: `call_cm`, type: "function", function: { name: "codemode", arguments: JSON.stringify({ code: SCRIPT }) } }] }) +
				chunk({}, "tool_calls") +
				"data: [DONE]\n\n",
		);
	});
});
await new Promise<void>((r) => model.listen(0, "127.0.0.1", r));
const port = (model.address() as any).port;

after(() => {
	model.close();
	fs.rmSync(home, { recursive: true, force: true });
});

const until = async (what: () => boolean, ms = 15000) => {
	const end = Date.now() + ms;
	while (!what()) {
		if (Date.now() > end) throw new Error("timed out");
		await new Promise((r) => setTimeout(r, 20));
	}
};

test("every thread has code mode: one script runs several tools", async () => {
	saveProvider({ id: "stub", name: "Stub", base_url: `http://127.0.0.1:${port}/v1`, api_key: "k", api_mode: "chat_completions", models: { m: {} } });
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m" } as any);
	let done = 0;
	const rt = new Runtime(() => {});
	rt.runs.add((_id, e) => {
		if (e.kind === "complete") done++;
	});
	await rt.open();
	const { row } = await rt.create("default", home, "local");

	await rt.submit(row.id, "look");
	await until(() => done === 1);
	assert.ok(offered[0].includes("codemode"));
	assert.ok(offered[0].includes("bash"), "the other tools stay offered beside it");
	assert.match(results.at(-1)!, /alpha\+beta/, "only what the script printed comes back");
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
