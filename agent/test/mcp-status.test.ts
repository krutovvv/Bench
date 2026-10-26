// What the block is told about a project's MCP servers. The point of the whole shape: a server
// nothing has asked yet must not be reported like one that answers. Before `checked` existed the
// two were the same payload (no tools, no error), and the interface drew a server nobody had
// touched as healthy.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

test("an MCP server reads as untried until something tries it", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-mcp-"));
	// NO_MCP: the server does not connect anything at its own start, so "untried" is real here.
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], { env: { ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1" }, stdio: ["ignore", "ignore", "pipe"] });
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	try {
		const file = path.join(home, "server.json");
		for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
		assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
		const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));
		const call = (method: string, p: string, body?: unknown) =>
			fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "X-Hermes-Session-Token": token, "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
		const made = await call("POST", "/api/projects", { name: "Shop" });
		const id = made.project.id;
		// One http server, at a port nothing listens on: Check will fail fast and honestly.
		await call("PUT", `/api/projects/${id}/mcp`, { text: JSON.stringify({ mcpServers: { api: { url: "http://127.0.0.1:1/mcp" } } }) });

		const before = (await call("GET", `/api/projects/${id}/env`)).mcp;
		assert.equal(before.length, 1);
		assert.equal(before[0].name, "api");
		assert.equal(before[0].kind, "http");
		assert.equal(before[0].enabled, true);
		// The crux: never tried, and said so — not tools 0 + error null, which reads as healthy.
		assert.equal(before[0].checked, false);
		assert.equal(before[0].connected, false);
		assert.equal(before[0].error, null);

		const after = (await call("POST", `/api/projects/${id}/mcp/check`)).mcp;
		assert.equal(after[0].checked, true, "Check must leave no server untried");
		assert.equal(after[0].connected, false);
		assert.ok(after[0].error, "and must say why it does not answer");

		// Off is a fact of its own, not a failure: the entry is gone from the tried set, but the
		// block is told it is switched off.
		await call("PUT", `/api/projects/${id}/mcp/api`, { enabled: false });
		const off = (await call("POST", `/api/projects/${id}/mcp/check`)).mcp;
		assert.equal(off[0].enabled, false);
		assert.equal(off[0].connected, false);
		assert.equal(off[0].checked, false);
	} finally {
		child.kill("SIGTERM");
		fs.rmSync(home, { recursive: true, force: true });
	}
});