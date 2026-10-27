// A thread's numbers (src/usage.ts) on a real Pi harness in a temporary BENCH_HOME: they are
// written when a turn ends and are there again after the agent process restarts, so Bench shows
// tokens, context and speed in a reopened thread instead of an empty status bar.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-usage-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";
process.env.PI_OFFLINE = "1";
after(() => fs.rmSync(home, { recursive: true, force: true }));

const { Runtime } = await import("../src/runtime.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");
const { Store } = await import("../src/store.ts");
const { totalsOf, usagePayload } = await import("../src/usage.ts");

test("the counters of a turn are saved and read back after a restart", async () => {
	let rt = new Runtime(() => {});
	await rt.open();
	const live = await rt.create("default", undefined, "local");
	const id = live.row.id;
	await rt.submit(id, "say hi");
	for (let i = 0; i < 200 && rt.isBusy(id); i++) await new Promise((r) => setTimeout(r, 25));
	assert.equal(rt.isBusy(id), false, "the offline turn finished");

	const saved = new Store().get(id)!.usage;
	assert.ok(saved && typeof saved === "object", "finish() wrote the usage");
	assert.ok(Number.isFinite(saved.input) && Number.isFinite(saved.output) && Number.isFinite(saved.calls));

	// A second process opening the same thread continues from those numbers.
	await (rt.harness as any).close(BACKGROUND_CONTEXT);
	rt = new Runtime(() => {});
	await rt.open();
	const again = await rt.session(id);
	assert.deepEqual(
		{ input: again!.usage.input, output: again!.usage.output, calls: again!.usage.calls },
		{ input: saved.input, output: saved.output, calls: saved.calls },
	);
	const frame = usagePayload("m", totalsOf(saved), 128000);
	assert.equal(frame.model, "m");
	assert.equal(frame.context_max, 128000);
	assert.equal(frame.context_used, saved.input);
	assert.equal(frame.output, saved.output);
	assert.equal(frame.calls, saved.calls);

	// Reading the thread settles how often it was compacted (its journal is read there anyway).
	await rt.thread(id);
	assert.equal(totalsOf(new Store().get(id)!.usage).compactions, 0, "never compacted says 0, and is saved");
});

test("the bar's compactions are the thread's number, not a written zero", () => {
	const frame = usagePayload("m", totalsOf({ input: 10, output: 5, calls: 2, compactions: 22 }), 128000);
	assert.equal(frame.compressions, 22);
	assert.equal(usagePayload("m", totalsOf({ input: 1 }), 1).compressions, 0, "a thread that never had one");
	assert.equal(totalsOf({ input: 1, compactions: -3 }).compactions, undefined, "junk is nothing");
});