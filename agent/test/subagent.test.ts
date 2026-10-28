// `delegate` (builtin/subagent.ts) on a real Pi harness with a stub model: the parent's call makes
// child conversations without `delegate`, each answers its task, and the call returns the answers.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-subagent-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";

const { saveProvider, writeAgent } = await import("../src/config.ts");
const { Runtime } = await import("../src/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

const chunk = (delta: any, finish: string | null = null) =>
	`data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;

// The parent delegates once and then answers; a child answers its task at once. The tools each
// request offers are kept, to see the child has no `delegate`.
const offered: { child: boolean; tools: string[] }[] = [];
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		res.writeHead(200, { "content-type": "text/event-stream" });
		if (body.includes("You name chat sessions")) {
			res.end(chunk({ role: "assistant", content: "Delegating" }) + chunk({}, "stop") + "data: [DONE]\n\n");
			return;
		}
		const json = JSON.parse(body);
		const tools: string[] = (json.tools ?? []).map((t: any) => t.function?.name);
		// A child knows only its task; the parent's requests carry the person's question.
		const child = !body.includes("how many files?");
		offered.push({ child, tools });
		if (child) res.write(chunk({ role: "assistant", content: "There are 3 files." }) + chunk({}, "stop"));
		else if (body.includes('"role":"tool"')) res.write(chunk({ role: "assistant", content: "The subagent says 3." }) + chunk({}, "stop"));
		else
			res.write(
				chunk({ role: "assistant", tool_calls: [{ index: 0, id: "call_d", type: "function", function: { name: "delegate", arguments: JSON.stringify({ tasks: [{ goal: "count the files" }] }) } }] }) +
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

test("delegate: a child without delegate answers its task, and the call returns the answer", async () => {
	saveProvider({ id: "stub", name: "Stub", base_url: `http://127.0.0.1:${port}/v1`, api_key: "k", api_mode: "chat_completions", models: { m: {} } });
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m" });
	const rt = new Runtime(() => {});
	await rt.open();
	const { row } = await rt.create("default", home, "local");
	await rt.submit(row.id, "how many files?");
	// The parent's answer, read from its journal (a child's events reach the parent's session too).
	const answered = async () => (await rt.journal.open(row.id)).items.some((i: any) => i.kind === "text" && i.text === "The subagent says 3.");
	const end = Date.now() + 20000;
	while (!(await answered()) && Date.now() < end) await new Promise((r) => setTimeout(r, 50));
	assert.ok(await answered(), "the parent answered with the child's answer");
	const child = offered.find((o) => o.child);
	assert.ok(child, "the child was asked");
	assert.ok(!child.tools.includes("delegate"), "the child has no delegate of its own");
	assert.ok(offered.find((o) => !o.child)?.tools.includes("delegate"), "the parent has it");
	const opened = await rt.journal.open(row.id);
	const result: any = opened.items.find((i: any) => i.kind === "result" && i.name === "delegate");
	assert.match(result.output, /There are 3 files/);
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
