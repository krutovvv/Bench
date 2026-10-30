// How many threads work at once (`runtime/slots.ts`): the line by itself, then on a real Pi harness
// in a temporary BENCH_HOME, whose stub model answers when the test lets it.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-slots-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";

const { saveProvider, writeAgent } = await import("../src/config.ts");
const { Line } = await import("../src/runtime/slots.ts");
const { Runtime } = await import("../src/runtime/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

const tick = () => new Promise((r) => setTimeout(r, 5));

const line = (limit = 1) => {
	const places: [string, number | null][] = [];
	const held = new Set<string>();
	const state = { limit };
	const l = new Line(
		() => state.limit,
		(id: string) => held.has(id),
		(id: string, place: number | null) => places.push([id, place]),
	);
	return { l, places, held, state };
};

/** The threads that got through, as they do. */
const go = (l: InstanceType<typeof Line>, gone: string[], id: string, signal?: AbortSignal) =>
	l.enter(id, signal).then(() => void gone.push(id));

test("threads go in the order they asked, as many as the limit, each until it leaves", async () => {
	const { l, places } = line();
	const gone: string[] = [];
	go(l, gone, "a");
	go(l, gone, "b");
	go(l, gone, "c");
	await tick();
	assert.deepEqual(gone, ["a"]);
	assert.equal(l.placeOf("b"), 1);
	assert.equal(l.placeOf("c"), 2);
	await l.enter("a"); // its next request: it holds the slot already
	l.leave("a");
	await tick();
	assert.deepEqual(gone, ["a", "b"]);
	assert.equal(l.placeOf("c"), 1);
	assert.deepEqual(places.filter(([id]) => id === "b"), [["b", 1], ["b", null]]);
	l.leave("b");
	await tick();
	assert.deepEqual(gone, ["a", "b", "c"]);
});

test("no limit, nobody waits; a limit raised lets the line go", async () => {
	const free = line(0);
	const all: string[] = [];
	go(free.l, all, "a");
	go(free.l, all, "b");
	await tick();
	assert.deepEqual(all, ["a", "b"]);

	const { l, state } = line(1);
	const gone: string[] = [];
	go(l, gone, "a");
	go(l, gone, "b");
	await tick();
	assert.deepEqual(gone, ["a"]);
	state.limit = 2;
	l.pump();
	await tick();
	assert.deepEqual(gone, ["a", "b"]);
});

test("a stop while it waits takes it out of the line; a held thread lets the next one past", async () => {
	const { l, held } = line();
	const gone: string[] = [];
	go(l, gone, "a");
	const stop = new AbortController();
	const stopped = l.enter("b", stop.signal);
	go(l, gone, "c");
	go(l, gone, "d");
	await tick();
	stop.abort(new Error("stopped"));
	await assert.rejects(stopped, /stopped/);
	assert.equal(l.placeOf("b"), undefined);
	assert.equal(l.placeOf("c"), 1);
	held.add("c");
	l.leave("a");
	await tick();
	assert.deepEqual(gone, ["a", "d"], "c is held: d goes");
	held.delete("c");
	l.leave("d");
	await tick();
	assert.deepEqual(gone, ["a", "d", "c"]);
});

// ---- on Pi --------------------------------------------------------------------------------------

const chunk = (delta: any, finish: string | null = null) =>
	`data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;

/** The requests waiting for an answer, the words of each. */
const open: { said: string; answer: () => void }[] = [];
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		const answer = (text: string) => {
			res.writeHead(200, { "content-type": "text/event-stream" });
			res.end(chunk({ role: "assistant", content: text }) + chunk({}, "stop") + "data: [DONE]\n\n");
		};
		if (body.includes("You name chat sessions")) return answer("A title");
		open.push({ said: body.match(/job-\w/)?.[0] ?? "", answer: () => answer("Done.") });
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

test("two threads at a limit of one: the second asks the model only when the first has answered; Stop takes it out", async () => {
	saveProvider({ id: "local", name: "local", base_url: `http://127.0.0.1:${port}/v1`, api_key: "k", api_mode: "chat_completions", models: { stub: { context_length: 100000 } }, parallel: 1 } as any);
	writeAgent("default", { defaultProvider: "local", defaultModel: "stub" });
	const told: [string, number | null][] = [];
	const rt = new Runtime((m: any) => {
		if (m.type === "thread" && m.event.kind === "line") told.push([m.thread, m.event.value]);
	});
	await rt.open();
	const [a, b, c] = [await rt.create("default", undefined, "local"), await rt.create("default", undefined, "local"), await rt.create("default", undefined, "local")];
	await rt.submit(a.row.id, "job-a");
	await until(() => open.length === 1);
	assert.equal(open[0].said, "job-a");
	await rt.submit(b.row.id, "job-b");
	await until(() => rt.slots.placeOf(b.row.id) === 1);
	await rt.submit(c.row.id, "job-c");
	await until(() => rt.slots.placeOf(c.row.id) === 2);
	assert.equal(open.length, 1, "the others do not reach the model");
	await rt.interrupt(b.row.id);
	await until(() => rt.slots.placeOf(b.row.id) === undefined && rt.slots.placeOf(c.row.id) === 1);
	await until(() => !rt.isBusy(b.row.id));
	open[0].answer();
	await until(() => open.length === 2);
	assert.equal(open[1].said, "job-c");
	assert.deepEqual(told.find(([id, p]) => id === c.row.id && p === null), [c.row.id, null]);
	open[1].answer();
	await until(() => !rt.isBusy(c.row.id));
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
