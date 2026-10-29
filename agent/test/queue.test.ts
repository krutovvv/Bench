// The thread's waiting messages (src/queue.ts, Runtime.sendNow) on a real Pi harness in a
// temporary BENCH_HOME: "send now" puts the message at the HEAD of the queue. Pi's inbox is an
// array and a boundary places the first item of its kind, so the queue's order is the order the
// agent answers in — and a fresh submission arrives at the end, which is why "send now" has to
// move it. The model is a stub server that never answers, so the run stays live and the queue
// stays put while the test looks at it.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-queue-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";

const { moveToFront } = await import("../src/runtime/queue.ts");
const { saveProvider, writeAgent } = await import("../src/config.ts");
const { Runtime } = await import("../src/runtime/runtime.ts");
const { listen } = await import("./listen.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

// A model server that reads the request and never replies: the run lives, so messages sent
// after it wait in the inbox instead of starting a run of their own.
const stall = http.createServer((req) => {
	req.resume();
});
await new Promise<void>((r) => stall.listen(0, "127.0.0.1", r));
const port = (stall.address() as any).port;

after(() => {
	stall.close();
	fs.rmSync(home, { recursive: true, force: true });
});

test("moveToFront: the head, an absent id, no move of the head itself", () => {
	const items = [{ id: 11, t: "a" }, { id: 12, t: "b" }, { id: 13, t: "c" }];
	assert.equal(moveToFront(items, "13"), 0);
	assert.deepEqual(items.map((i) => i.t), ["c", "a", "b"], "the rest keep their order behind it");
	assert.equal(moveToFront(items, "c" as any), -1, "not an id: nothing moves");
	assert.equal(moveToFront(items, "13"), 0, "already first");
	assert.equal(moveToFront([], "1"), -1);
});

test("send now: the message goes to the head of the queue, not its end", async () => {
	saveProvider({
		id: "stub",
		name: "Stub",
		base_url: `http://127.0.0.1:${port}/v1`,
		api_key: "k",
		api_mode: "chat_completions",
		models: { m: {} },
	});
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m" } as any);
	const told: any[][] = [];
	const requests: Record<string, string | undefined> = {};
	const runs: boolean[] = [];
	const rt = new Runtime(
		listen((e) => {
			if (e.kind === "queue") told.push(e.value.map((i) => i.text));
			if (e.kind === "queue") for (const i of e.value) requests[i.text] = i.request;
			if (e.kind === "run") runs.push(e.value);
		}),
	);
	await rt.open();
	const { row } = await rt.create("default", undefined, "local");
	const id = row.id;

	await rt.submit(id, "the run's first message");
	// Two more, sent while it works: they wait, in the order they were sent.
	await rt.submit(id, "second", [], "", "queue", "bench-2");
	await rt.submit(id, "third", [], "", "queue", "bench-3");
	assert.equal(rt.isBusy(id), true, "the stalled run is live");
	assert.deepEqual(runs, [true], "Bench is told the run is live as soon as the message starts it");
	const queue = async () => (await rt.journal.open(id)).queue;
	const before = await queue();
	// Pi keeps Bench's id with each submission, and the queue's rows carry it.
	assert.equal(requests["second"], "bench-2");
	assert.equal(requests["third"], "bench-3");
	assert.deepEqual(before.map((i) => i.text), ["second", "third"]);
	// The placed message's words are found by search: the index is fed by Pi's commits.
	assert.deepEqual(rt.store.search("first message").map((h) => h.thread_id), [id]);
	assert.equal(rt.store.get(id)?.message_count, 1, "the waiting ones are not messages of the thread yet");

	// The person presses "send now" on the LAST one: it must be the next thing answered.
	const third = before.find((i) => i.text === "third")!.sub;
	await rt.queue.sendNow(id, third);
	const after = await queue();
	assert.deepEqual(after.map((i) => i.text), ["third", "second"], "it rose to the head");
	assert.equal(after[0].mode, "steer", "and it joins the work that runs");

	// The other one waits behind it, and is sent as itself, not doubled.
	const second = after.find((i) => i.text === "second")!.sub;
	await rt.queue.sendNow(id, second);
	assert.deepEqual((await queue()).map((i) => i.text), ["second", "third"]);
	// Bench hears each order as thread.queue, straight from the inbox's commits.
	assert.deepEqual(told.at(-1), ["second", "third"]);
	assert.ok(told.some((q) => q.join() === "third,second"));

	// An approval for the thread is kept in its document: the same command is not asked again; one
	// for always goes into its settings.
	const waits = async (call: string) => {
		while (!rt.live.get(id)!.prompts.has(call)) await new Promise((r) => setTimeout(r, 10));
	};
	const asked = rt.asks.approve(id, "c1", "rm -rf build", "deletes");
	await waits("c1");
	rt.asks.respond(id, "c1", "session");
	assert.equal(await asked, "session");
	assert.equal(await rt.asks.approve(id, "c2", "rm -rf build", "deletes"), "session", "not asked again");
	const always = rt.asks.approve(id, "c3", "git push -f", "rewrites");
	await waits("c3");
	rt.asks.respond(id, "c3", "always");
	assert.equal(await always, "always");
	const { readAgent } = await import("../src/config.ts");
	assert.deepEqual(readAgent("default").allowed, ["git push -f"]);

	// The person's name for the thread holds; a made-up one never replaces it.
	rt.titles.rename(id, "  Spring   landing ");
	assert.equal(rt.store.get(id)?.title, "Spring landing");
	(rt.titles as any).nameIfUnnamed(id, "Made up");
	assert.equal(rt.store.get(id)?.title, "Spring landing");

	// Compaction: one share of the window per agent, as Pi's policy under its own provider.
	const { compactionPolicy } = await import("../src/config.ts");
	assert.deepEqual(rt.models.compactionOverrides(), {}, "nothing set: Pi's own policy");
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m", compaction: 0.9 } as any);
	const over = rt.models.compactionOverrides();
	const key = Object.keys(over)[0]!;
	assert.match(key, /^default\.stub.*\/m$/);
	assert.deepEqual(over[key], compactionPolicy(0.9, 131072));
	assert.deepEqual(compactionPolicy(0.9, 100000), { reserveTokens: 10000, backgroundTokens: 5000, keepRecentTokens: 18000 });
	writeAgent("default", { defaultProvider: "stub", defaultModel: "m" } as any);

	// A stalled run never ends on its own: stop it, or closing waits for it forever.
	await rt.interrupt(id);
	stall.closeAllConnections();
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});