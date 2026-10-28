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

const { Inserts, insertsOf, patchInsert } = await import("../src/runtime/inserts.ts");
const { inserted } = await import("../src/prompt.ts");
const { Runtime } = await import("../src/runtime/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

test("inserts sit after the messages they were put after", () => {
	const entries = [
		{ kind: "bench.insert", data: { id: "i1", key: "tasks" } },
		{ kind: "pi.user" },
		{ kind: "pi.assistant" },
		{ kind: "bench.insert", data: { id: "i2", key: "ssh:web" } },
		{ kind: "bench.insert", data: { id: "i3", key: "tools" } },
		{ kind: "pi.user" },
	];
	const doc: { items: Record<string, import("../src/runtime/inserts.ts").InsertState> } = {
		items: {
			// i1 sits where its entry says; i2 was moved down the thread (a message that went
			// after it); i3's "after" is nonsense, so its entry decides.
			i1: { key: "tasks", state: { list: "upcoming" }, folded: true, removed: false },
			i2: { key: "ssh:db", state: {}, folded: false, removed: false, after: 4 },
			i3: { key: "tools", state: {}, folded: false, removed: true, after: -2 },
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
	await rt.inserts.put(id, "ia", "ssh:local");
	await rt.inserts.put(id, "ib", "tools", { tool: "base64" });
	// The same id twice is one insert.
	await rt.inserts.put(id, "ib", "tools");
	await rt.inserts.patch(id, "ib", { state: { tool: "json" } });
	await rt.inserts.patch(id, "ia", { removed: true });
	await rt.inserts.put(id, "ic", "tasks", { list: "today" });
	// It was put while a message waited in the composer: it sits after that message.
	await rt.inserts.patch(id, "ic", { after: 3 });
	await assert.rejects(rt.inserts.put(id, "bad id!", "tasks"));
	await assert.rejects(rt.inserts.patch(id, "missing", { folded: true }));
	const check = (list: any[]) => {
		assert.deepEqual(
			list.map((i) => [i.id, i.key, i.after]),
			[
				["ib", "tools", 0],
				["ic", "tasks", 3],
			],
		);
		assert.deepEqual(list[0].state, { tool: "json" });
	};
	check(await rt.inserts.list(id));
	// The prompt lists what is inserted.
	const text = inserted((await rt.inserts.list(id)).map((i) => ({ key: i.key, state: i.state })))!;
	assert.match(text, /everyday utilities, now on "json"/);
	assert.match(text, /tasks tool/);
	assert.doesNotMatch(text, /terminal/);
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
	// Another process: the same, from Pi's storage.
	rt = new Runtime(() => {});
	await rt.open();
	check(await rt.inserts.list(id));
});

test("an insert whose saved state went missing is healed, not refused", async () => {
	const rt = new Runtime(() => {});
	await rt.open();
	const live = await rt.create("default", undefined, "local");
	const id = live.row.id;
	await rt.inserts.put(id, "ih", "ssh:local");
	// The entry stayed while the document lost the item: a thread from before the document, or a
	// commit that dropped it. Bench still shows the insert, because the entry says it is there.
	await live.conversation.commit(
		async (tx) => {
			const doc = (await tx.doc(Inserts as any, live.conversation.id)) as any;
			delete doc.items.ih;
		},
		BACKGROUND_CONTEXT,
	);
	assert.deepEqual((await rt.inserts.list(id)).map((i) => i.id), ["ih"], "shown from its entry");
	// Its next change must heal the state. It used to throw, and Bench answered a failed insert by
	// reading the whole thread again — which wiped the person's draft and their queue.
	await rt.inserts.patch(id, "ih", { folded: true });
	const healed = (await rt.inserts.list(id)).find((i) => i.id === "ih")!;
	assert.equal(healed.key, "ssh:local", "the entry's key is kept");
	assert.equal(healed.folded, true);
	// An id no entry knows is still nobody's insert.
	await assert.rejects(rt.inserts.patch(id, "ghost", { folded: true }));
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
});
