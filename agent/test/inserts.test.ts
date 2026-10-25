// The person's inserts in a thread (src/inserts.ts), on a real Pi harness in a temporary
// BENCH_HOME: put where the conversation is, changed, removed, kept across a restart, and seen by
// the prompt; an old thread's subject becomes its first insert.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-inserts-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";
after(() => fs.rmSync(home, { recursive: true, force: true }));

const { insertsOf, patchInsert } = await import("../src/inserts.ts");
const { inserted } = await import("../src/prompt.ts");
const { Runtime } = await import("../src/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

test("inserts sit after the messages they were put after", () => {
	const entries = [
		{ kind: "bench.insert", data: { id: "i1", key: "tasks" } },
		{ kind: "pi.user" },
		{ kind: "pi.assistant" },
		{ kind: "bench.insert", data: { id: "i2", key: "ssh:web" } },
		{ kind: "bench.insert", data: { id: "i3", key: "files" } },
		{ kind: "pi.user" },
	];
	const doc = {
		items: {
			// i1 sits where its entry says; i2 was moved down the thread (a message that went
			// after it); i3's "after" is nonsense, so its entry decides.
			i1: { key: "tasks", state: { list: "upcoming" }, folded: true, removed: false },
			i2: { key: "ssh:db", state: {}, folded: false, removed: false, after: 4 },
			i3: { key: "files", state: {}, folded: false, removed: true, after: -2 },
		},
	};
	const got = insertsOf(entries, doc as any);
	assert.deepEqual(
		got.map((i) => [i.id, i.key, i.after, i.folded]),
		[
			["i1", "tasks", 0, true],
			["i2", "ssh:db", 4, false],
		],
	);
	assert.deepEqual(got[0].state, { list: "upcoming" });
	assert.equal(patchInsert(doc as any, "nope", { folded: true }), false);
	assert.equal(patchInsert(doc as any, "i2", { state: [1], key: 5, removed: false, after: "3" }), true);
	assert.deepEqual(doc.items.i2, { key: "ssh:db", state: {}, folded: false, removed: false, after: 4 }, "bad fields change nothing");
	assert.equal(patchInsert(doc as any, "i1", { after: 1 }), true);
	assert.equal(doc.items.i1.after, 1);
});

test("a thread's inserts: put, changed, removed, kept across a restart", async () => {
	let rt = new Runtime(() => {});
	await rt.open();
	const live = await rt.create("default", undefined, "local");
	const id = live.row.id;
	await rt.insert(id, "ia", "ssh:local");
	await rt.insert(id, "ib", "files", { host: "h1", path: "/www" });
	// The same id twice is one insert.
	await rt.insert(id, "ib", "files");
	await rt.patchInsert(id, "ib", { state: { host: "h1", path: "/www/blog" } });
	await rt.patchInsert(id, "ia", { removed: true });
	await rt.insert(id, "ic", "tasks", { list: "today" });
	// It was put while a message waited in the composer: it sits after that message.
	await rt.patchInsert(id, "ic", { after: 3 });
	await assert.rejects(rt.insert(id, "bad id!", "tasks"));
	await assert.rejects(rt.patchInsert(id, "missing", { folded: true }));
	const check = (list: any[]) => {
		assert.deepEqual(
			list.map((i) => [i.id, i.key, i.after]),
			[
				["ib", "files", 0],
				["ic", "tasks", 3],
			],
		);
		assert.deepEqual(list[0].state, { host: "h1", path: "/www/blog" });
	};
	check(await rt.inserts(id));
	// The prompt lists what is inserted.
	const text = inserted((await rt.inserts(id)).map((i) => ({ key: i.key, state: i.state })))!;
	assert.match(text, /file host/);
	assert.match(text, /tasks tool/);
	assert.doesNotMatch(text, /terminal/);
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
	// Another process: the same, from Pi's storage.
	rt = new Runtime(() => {});
	await rt.open();
	check(await rt.inserts(id));
});

test("an old thread's subject becomes its first insert", async () => {
	const rt = new Runtime(() => {});
	await rt.open();
	const live = await rt.create("default", undefined, "local", { subject: "files:h9" });
	const list = await rt.inserts(live.row.id);
	assert.deepEqual(
		list.map((i) => [i.key, i.state]),
		[["files", { host: "h9" }]],
	);
	assert.equal(rt.store.get(live.row.id)!.subject, "");
});
