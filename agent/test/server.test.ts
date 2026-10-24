// The server itself starts (a clash in its imports or its first-start import stops it before any
// route answers) and its projects API works, on a home of its own.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

test("the server starts and answers /api/projects", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-server-"));
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], { env: { ...process.env, BENCH_HOME: home, HERMES_HOME: path.join(home, "no-hermes"), BENCH_AGENT_NO_MCP: "1" }, stdio: ["ignore", "ignore", "pipe"] });
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	try {
		const file = path.join(home, "server.json");
		for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
		assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
		const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));
		const call = (method: string, p: string, body?: unknown) =>
			fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "X-Hermes-Session-Token": token, "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
		const made = await call("POST", "/api/projects", { name: "Site" });
		assert.equal(made.project.name, "Site");
		const env = await call("GET", `/api/projects/${made.project.id}/env`);
		assert.deepEqual([env.soul, env.mcp, env.skills], ["", [], []]);
		// A skill of the project, read back by name.
		const skill = path.join(env.dir, "skills", "publish");
		fs.mkdirSync(skill, { recursive: true });
		fs.writeFileSync(path.join(skill, "SKILL.md"), "---\nname: publish\ndescription: d\n---\nSteps.");
		const read = await call("GET", `/api/projects/${made.project.id}/skills/publish`);
		assert.match(read.text, /Steps\./);
	} finally {
		child.kill();
	}
});
