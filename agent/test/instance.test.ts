// One agent per home: a second start on a home whose agent lives exits saying so; a lock left by
// an agent that is gone does not stop the next one.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { startServer } from "./server.ts";

const env = (home: string) => ({ ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1", PI_OFFLINE: "1" });
const cwd = path.join(import.meta.dirname, "..");

test("a second agent on the same home exits; the first goes on", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-instance-"));
	const first = await startServer(home);
	try {
		const second = spawnSync(process.execPath, ["--no-warnings", "src/server.ts"], { env: env(home), cwd, encoding: "utf8", timeout: 20_000 });
		assert.equal(second.status, 1);
		assert.match(second.stderr, /another agent runs on/);
		const status = await first.rest("GET", "/api/status");
		assert.ok(status.version, "the first still answers");
	} finally {
		first.close();
	}
});

test("a lock left by an agent that is gone is taken over, and released at exit", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-instance-"));
	// A pid that is no agent: this test's own shell child, already exited.
	const gone = spawnSync("true").pid;
	fs.writeFileSync(path.join(home, "agent.pid"), String(gone));
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], { env: env(home), cwd, stdio: ["ignore", "ignore", "pipe"] });
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	const file = path.join(home, "server.json");
	for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
	assert.ok(fs.existsSync(file), `the agent did not start:\n${err}`);
	assert.equal(fs.readFileSync(path.join(home, "agent.pid"), "utf8"), String(child.pid));
	const exited = new Promise((r) => child.on("exit", r));
	child.kill("SIGTERM");
	await exited;
	assert.ok(!fs.existsSync(path.join(home, "agent.pid")), "released at exit");
});
