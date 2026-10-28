// How fast the model wrote belongs to the thread: Bench measures the stream, sends the numbers
// over at the end of an answer (`thread.speed`), the agent keeps them in bench.sqlite, and
// `thread.open` returns them, so a reopened thread or a restarted app shows the chart again.
// Driven over the real WebSocket of a real server, where a wrong field name would go unnoticed.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { startServer } from "./server.ts";

const { SPEED_SAMPLES } = await import("../src/usage.ts");

test("the speed of an answer is kept with the thread and comes back when it opens", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-speed-"));
	const s = await startServer(home);
	try {
		const { thread } = await s.call("thread.create", { project: null, cwd: null });
		// 200 one-second samples: the thread is longer than the chart, so only the last minutes
		// are worth keeping.
		await s.call("thread.speed", { thread, speed: { samples: Array.from({ length: 200 }, (_, k) => 500 + k), prefill: 1200.5, generation: 990.25, ratio: 3.1 } });
		const back = (await s.call("thread.open", { thread, since: 0 })).speed;
		assert.equal(back.prefill, 1200.5, "the prefill of the last call");
		assert.equal(back.generation, 990.25, "the generation speed of the last call");
		assert.equal(back.ratio, 3.1, "the characters-per-token Bench learned on this model");
		assert.equal(back.samples.length, SPEED_SAMPLES, "the chart's tail, not the whole thread");
		assert.equal(back.samples.at(-1), 699, "the last second first of the tail");

		// A thread that never ran has nothing to show, and a broken report is not kept as a fact.
		const other = (await s.call("thread.create", { project: null, cwd: null })).thread;
		assert.deepEqual((await s.call("thread.open", { thread: other, since: 0 })).speed.samples, []);
		await s.call("thread.speed", { thread, speed: { samples: ["x", -5, 0, 40], prefill: "fast" } as any });
		const sane = (await s.call("thread.open", { thread, since: 0 })).speed;
		assert.deepEqual(sane.samples, [40], "only real readings are kept");
		assert.equal(sane.prefill, undefined, "a speed that is not a number is no speed");
	} finally {
		s.close();
		fs.rmSync(home, { recursive: true, force: true });
	}
});
