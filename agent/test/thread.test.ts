// The thread as a journal (src/thread.ts): keys, order, the queue.

import assert from "node:assert/strict";
import { test } from "node:test";
import { changesOf, compactionsOf, COMPACTION_KIND, FAIL_KIND, INBOX_KIND, INSERTS_KIND, itemsOf, itemsOfAll, queueOf } from "../src/thread.ts";

const user = (id: number, text: string) => ({ id, conversationId: 7, kind: "pi.user", model: [{ role: "user", content: text, timestamp: 1000 + id }] });
const assistant = (id: number, content: any[], extra: any = {}) => ({ id, conversationId: 7, kind: "pi.assistant", model: [{ role: "assistant", content, timestamp: 1000 + id, ...extra }] });
const result = (id: number, call: string, text: string, isError = false) => ({
	id,
	conversationId: 7,
	kind: "pi.tool-result",
	model: [{ role: "toolResult", toolCallId: call, toolName: "bash", content: [{ type: "text", text }], isError }],
});

test("a user entry: its words without the block state", () => {
	const [it] = itemsOf(user(5, "fix it\n\n<block-state>\nterminal: ok\n</block-state>"));
	assert.deepEqual(it, { key: "5.0", seq: 5, kind: "user", text: "fix it", at: 1005 });
});

test("an assistant entry: thought, text, tool calls, each with its own key, in the entry's order", () => {
	const items = itemsOf(
		assistant(9, [
			{ type: "thinking", thinking: "hmm" },
			{ type: "text", text: "Looking." },
			{ type: "toolCall", id: "c1", name: "bash", arguments: { command: "ls" } },
			{ type: "toolCall", id: "c2", name: "read", arguments: { path: "a" } },
		]),
	);
	assert.deepEqual(
		items.map((i) => [i.key, i.kind]),
		[
			["9.0", "thought"],
			["9.1", "text"],
			["9.2", "tool"],
			["9.3", "tool"],
		],
	);
	assert.equal((items[2] as any).call_id, "c1");
	assert.equal((items[2] as any).preview, "ls");
});

test("empty blocks give nothing; a model error is said", () => {
	assert.deepEqual(itemsOf(assistant(3, [{ type: "text", text: "  " }])), []);
	const [err] = itemsOf(assistant(4, [], { stopReason: "error", errorMessage: "503" }));
	assert.equal(err.kind, "text");
	assert.equal((err as any).error, "503");
});

test("a tool result, an insert, a fail note; compaction and system entries are not shown", () => {
	assert.deepEqual(itemsOf(result(10, "c1", "out", true)), [{ key: "10.0", seq: 10, kind: "result", call_id: "c1", name: "bash", output: "out", failed: true, at: undefined }]);
	const edit = { ...result(15, "c2", "ok"), data: { details: { patch: "--- a\n+++ b" } } };
	assert.equal((itemsOf(edit)[0] as any).diff, "--- a\n+++ b", "an edit carries its patch");
	assert.deepEqual(itemsOf({ id: 11, kind: "bench.insert", data: { id: "i1", key: "tasks" } }), [{ key: "11", seq: 11, kind: "insert", id: "i1", insert_key: "tasks" }]);
	assert.deepEqual(itemsOf({ id: 12, kind: FAIL_KIND, data: { text: "no model" } }), [{ key: "12", seq: 12, kind: "fail", text: "no model" }]);
	assert.deepEqual(itemsOf({ id: 13, kind: "pi.compaction", data: {} }), []);
	assert.deepEqual(itemsOf({ id: 14, kind: "pi.system", model: [{ role: "user", content: "x" }] }), []);
});

test("the plan, the questions and their answers are journal items of their own", () => {
	assert.deepEqual(itemsOf({ id: 12, kind: "bench.plan", data: { id: "p1", todos: [{ id: "1", content: "a", status: "in_progress" }, { id: "2", content: "b", status: "pending", parent: "1" }] } }), [
		{ key: "12", seq: 12, kind: "plan", id: "p1", todos: [{ id: "1", content: "a", status: "in_progress" }, { id: "2", content: "b", status: "pending", parent: "1" }] },
	]);
	assert.deepEqual(itemsOf({ id: 13, kind: "bench.ask", data: { id: "q1", ask_kind: "clarify", question: "Which?", choices: ["a", "b"], multi: false } }), [
		{ key: "13", seq: 13, kind: "ask", id: "q1", ask_kind: "clarify", question: "Which?", choices: ["a", "b"], multi: false },
	]);
	assert.deepEqual(itemsOf({ id: 14, kind: "bench.asked", data: { id: "q1", answer: "a" } }), [{ key: "14", seq: 14, kind: "asked", id: "q1", answer: "a" }]);
	// a decline is an empty answer, kept as it is
	assert.deepEqual(itemsOf({ id: 15, kind: "bench.asked", data: { id: "q1", answer: "" } }), [{ key: "15", seq: 15, kind: "asked", id: "q1", answer: "" }]);
});

test("how often the thread was compacted comes from its journal", () => {
	// The name the journal uses is the name the count looks for; a rename must not quietly
	// return the status bar's zero.
	assert.equal(COMPACTION_KIND, "pi.compaction");
	const journal = [user(1, "a"), { id: 2, kind: COMPACTION_KIND }, assistant(3, [{ type: "text", text: "b" }]), { id: 4, kind: COMPACTION_KIND }, { id: 5, kind: "pi.system" }];
	assert.equal(compactionsOf(journal), 2);
	assert.equal(compactionsOf([]), 0);
});

test("items follow the journal's ids whatever order the entries came in", () => {
	const items = itemsOfAll([assistant(20, [{ type: "text", text: "b" }]), user(18, "a"), result(22, "c", "c")]);
	assert.deepEqual(items.map((i) => i.seq), [18, 20, 22]);
	const keys = items.map((i) => i.key);
	assert.equal(new Set(keys).size, keys.length, "keys are unique");
});

test("the queue keeps the inbox's order and leaves writes out", () => {
	const q = queueOf({
		items: [
			{ id: 31, mode: "followUp", content: "second" },
			{ id: 30, mode: "write", entry: { kind: "bench.insert" } },
			{ id: 29, mode: "steer", content: [{ type: "text", text: "first\n\n<block-state>x</block-state>" }] },
		],
	});
	assert.deepEqual(q, [
		{ key: "q31", sub: "31", mode: "followUp", text: "second" },
		{ key: "q29", sub: "29", mode: "steer", text: "first" },
	]);
	assert.deepEqual(queueOf(undefined), []);
});

test("a commit: entries of watched threads in id order, the inbox as the queue, the inserts' state", () => {
	const changes = [
		{ type: "entry", value: assistant(41, [{ type: "text", text: "done" }]) },
		{ type: "entry", value: user(40, "go") },
		{ type: "entry", value: { ...user(42, "elsewhere"), conversationId: 8 } },
		{ type: "document", conversationId: 7, record: { kind: INBOX_KIND }, value: { items: [{ id: 50, mode: "followUp", content: "next" }] } },
		{ type: "document", conversationId: 7, record: { kind: INSERTS_KIND }, value: { items: { i1: { key: "tasks" } } } },
		{ type: "task", value: {} },
	];
	const out = changesOf(changes, (c) => c === "7");
	assert.equal(out.length, 1, "conversation 8 is not watched");
	const [t] = out;
	assert.equal(t.conversation, "7");
	assert.deepEqual(t.append!.map((i) => i.key), ["40.0", "41.0"]);
	assert.deepEqual(t.queue!.map((i) => i.text), ["next"]);
	assert.deepEqual(t.inserts, { i1: { key: "tasks" } });
});

test("a commit that only empties the queue says so, so the waiting row leaves", () => {
	const [t] = changesOf([{ type: "document", conversationId: 7, record: { kind: INBOX_KIND }, value: { items: [] } }], () => true);
	assert.deepEqual(t.queue, []);
	assert.equal(t.append, undefined);
});

test("on a real Pi: commits reach the thread as thread.* events, and thread.open since a head", async () => {
	const fs = await import("node:fs");
	const os = await import("node:os");
	const path = await import("node:path");
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-thread-"));
	process.env.BENCH_HOME = home;
	process.env.BENCH_AGENT_NO_MCP = "1";
	const { Runtime } = await import("../src/runtime.ts");
	const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");
	const events: [string, any][] = [];
	const rt = new Runtime((_sid, type, payload) => {
		if (type.startsWith("thread.")) events.push([type, payload]);
	});
	await rt.open();
	const { row } = await rt.create("default", undefined, "local");
	await rt.insert(row.id, "ia", "tasks", { list: "today" });
	const first = await rt.thread(row.id);
	assert.deepEqual(first.items.map((i) => [i.kind, i.id]), [["insert", "ia"]]);
	assert.deepEqual(first.queue, []);
	assert.equal((first.inserts as any).ia.key, "tasks");
	assert.equal(first.running, false);

	await rt.insert(row.id, "ib", "ssh:local");
	const appended = events.filter(([t]) => t === "thread.append").flatMap(([, p]) => p.items);
	assert.deepEqual(appended.map((i: any) => i.id), ["ia", "ib"], "each entry was told once, in order");
	assert.ok(events.some(([t, p]) => t === "thread.inserts" && p.items.ib), "the state document too");

	const later = await rt.thread(row.id, first.head);
	assert.deepEqual(later.items.map((i) => i.id), ["ib"], "only what came after the head");
	assert.ok(later.head > first.head);
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
	fs.rmSync(home, { recursive: true, force: true });
});
