// How fast the model wrote belongs to the thread: Bench measures the stream, sends the numbers
// over at the end of an answer, the agent keeps them in bench.sqlite, and `session.resume`
// returns them, so a reopened thread or a restarted app shows the chart again. Driven over the
// real WebSocket of a real server, which is where a wrong field name would go unnoticed.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import WebSocket from "ws";

const { SPEED_SAMPLES } = await import("../src/usage.ts");

test("the speed of an answer is kept with the thread and comes back on resume", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-speed-"));
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], {
		env: { ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1", PI_OFFLINE: "1" },
		stdio: ["ignore", "ignore", "pipe"],
	});
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	try {
		const file = path.join(home, "server.json");
		for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
		assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
		const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));
		const ws = new WebSocket(`ws://127.0.0.1:${port}/api/ws?token=${token}`);
		await new Promise((res, rej) => {
			ws.on("open", res);
			ws.on("error", rej);
		});
		let n = 0;
		const wait = new Map<number, (v: any) => void>();
		ws.on("message", (d) => {
			const m = JSON.parse(String(d));
			if (m.id && wait.has(m.id)) (wait.get(m.id) as any)(m);
			wait.delete(m.id);
		});
		const call = (method: string, params: any = {}) =>
			new Promise<any>((res, rej) => {
				const id = ++n;
				wait.set(id, (m) => (m.error ? rej(new Error(m.error.message)) : res(m.result)));
				ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
			});

		const made = await call("session.create", { profile: "default" });
		const id = made.session_id;
		// 200 one-second samples: the thread is longer than the chart, so only the last minutes
		// are worth keeping.
		await call("session.speed", {
			session_id: id,
			speed: { samples: Array.from({ length: 200 }, (_, k) => 500 + k), prefill: 1200.5, generation: 990.25, ratio: 3.1 },
		});
		const back = await call("session.resume", { session_id: id });
		assert.equal(back.speed.prefill, 1200.5, "the prefill of the last call");
		assert.equal(back.speed.generation, 990.25, "the generation speed of the last call");
		assert.equal(back.speed.ratio, 3.1, "the characters-per-token Bench learned on this model");
		assert.equal(back.speed.samples.length, SPEED_SAMPLES, "the chart's tail, not the whole thread");
		assert.equal(back.speed.samples.at(-1), 699, "the last second first of the tail");

		// A thread that never ran has nothing to show, and a broken report is not kept as a fact.
		const other = (await call("session.create", { profile: "default" })).session_id;
		assert.deepEqual((await call("session.resume", { session_id: other })).speed.samples, []);
		await call("session.speed", { session_id: id, speed: { samples: ["x", -5, 0, 40], prefill: "fast" } });
		const sane = (await call("session.resume", { session_id: id })).speed;
		assert.deepEqual(sane.samples, [40], "only real readings are kept");
		assert.equal(sane.prefill, null, "a speed that is not a number is no speed");
		ws.close();
		child.kill();
	} finally {
		fs.rmSync(home, { recursive: true, force: true });
	}
});