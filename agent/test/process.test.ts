// Background processes (builtin/process.ts) on a real Pi harness in a temporary BENCH_HOME: a
// process is a background task of the conversation; its output and its end reach the thread as
// `thread.process` events from Pi's commits; Stop ends it; a restart of the agent ends it for good.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import { execSync } from "node:child_process";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-process-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";
const { Runtime } = await import("../src/runtime.ts");
const { listen } = await import("./listen.ts");
const { ProcessTask, keepTail } = await import("../src/builtin/process.ts");
const { BACKGROUND_CONTEXT: ctx } = await import("@earendil-works/chord/context");

/** Whether a command line is still running on this machine (`[s]` so pgrep's shell is not it). */
const alive = (pattern: string) => execSync(`pgrep -f '${pattern}' || true`).toString().trim() !== "";

after(() => fs.rmSync(home, { recursive: true, force: true }));

async function until<T>(what: () => T | undefined | Promise<T | undefined>, ms = 5000): Promise<T> {
	const end = Date.now() + ms;
	for (;;) {
		const v = await what();
		if (v) return v;
		if (Date.now() > end) throw new Error("timed out");
		await new Promise((r) => setTimeout(r, 20));
	}
}

/** What a `process start` call does: a background task of the conversation, given up on a restart. */
async function start(rt: any, sessionId: string, command: string, call: string): Promise<string> {
	const live = await rt.session(sessionId);
	const id = await live.conversation.commit((tx: any) => tx.createTask(ProcessTask, { command, call }, { ownership: { kind: "conversation" }, background: true, abandonOnRestart: true }), ctx);
	return String(id);
}

test("the tail keeps the last lines and bytes", () => {
	const many = Array.from({ length: 300 }, (_, i) => `line ${i}`).join("\n");
	const t = keepTail("", many);
	assert.equal(t.split("\n").length, 200);
	assert.ok(t.endsWith("line 299"));
	assert.ok(keepTail("", "x".repeat(40000)).length <= 16 * 1024);
});

test("a process runs in the background: its output and exit reach the thread, Stop ends another", async () => {
	const seen: any[] = [];
	const rt = new Runtime(
		listen((e) => {
			if (e.kind === "process") seen.push(e.value);
		}),
	);
	await rt.open();
	const { row } = await rt.create("default", home, "local");
	await rt.journal.open(row.id);

	const done = await start(rt, row.id, "echo hello; echo world; exit 3", "c1");
	const exited = await until(() => seen.find((p) => p.id === done && p.state !== "running"));
	assert.equal(exited.state, "exited");
	assert.equal(exited.exit_code, 3);
	assert.equal(exited.call_id, "c1", "it stands at the call that started it");
	assert.match(exited.tail, /hello\nworld/);
	assert.ok(exited.started && exited.ended && exited.ended >= exited.started);

	const long = await start(rt, row.id, "echo up; sleep 3031", "c2");
	await until(() => seen.find((p) => p.id === long && p.tail.includes("up")), 4000);
	await rt.processes.stop(long);
	const stopped = await until(() => seen.find((p) => p.id === long && p.state !== "running"));
	assert.equal(stopped.state, "stopped");
	assert.match(stopped.tail, /up/, "what it said before it stopped is kept");
	assert.ok(!alive("[s]leep 3031"), "the command itself is gone");

	// thread.open has them, from Pi's records.
	const open = await rt.journal.open(row.id);
	assert.deepEqual(open.processes.map((p) => [p.call_id, p.state]), [["c1", "exited"], ["c2", "stopped"]]);
	await (rt.harness as any).close(ctx);
});

test("a process running when the agent restarts ends with it and is not started again", async () => {
	const rt = new Runtime(() => {});
	await rt.open();
	const { row } = await rt.create("default", home, "local");
	const marker = path.join(home, "started-times");
	const id = await start(rt, row.id, `echo x >> ${marker}; sleep 3032`, "c9");
	await until(() => fs.existsSync(marker) || undefined);
	await (rt.harness as any).close(ctx);
	await new Promise((r) => setTimeout(r, 200));
	assert.ok(!alive("[s]leep 3032"), "the command does not outlive the agent");

	const rt2 = new Runtime(() => {});
	await rt2.open();
	await rt2.journal.open(row.id);
	const conversation = (await (rt2 as any).session(row.id)).conversation.id;
	const ended = await until(async () => (await rt2.processes.list(conversation)).find((p) => p.id === id && p.state !== "running"));
	assert.equal(ended.state, "restarted");
	await new Promise((r) => setTimeout(r, 300));
	assert.equal(fs.readFileSync(marker, "utf8").trim().split("\n").length, 1, "the command ran once");
	await (rt2.harness as any).close(ctx);
});
