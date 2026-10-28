// A server that did not answer is tried again with a later message, a minute apart: before, one
// that failed once (the browser on dev, down when the agent started) stayed out of every thread
// until its mcp.json changed.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-mcp-retry-"));
process.env.BENCH_HOME = home;
const { writeAgentMcp } = await import("../src/config.ts");
const { ensureMcp } = await import("../src/mcp/mcp.ts");

// A server that refuses every request, counting them.
let hits = 0;
const server = http.createServer((_req, res) => {
	hits++;
	res.statusCode = 500;
	res.end("down");
});
await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
after(() => {
	server.close();
	fs.rmSync(home, { recursive: true, force: true });
});

test("a failed server is tried again a minute later, not with every message", async () => {
	writeAgentMcp("project:p", { browser: { url: `http://127.0.0.1:${(server.address() as any).port}/mcp` } });
	assert.equal(await ensureMcp("project:p"), true, "first connect");
	const first = hits;
	assert.ok(first > 0);
	assert.equal(await ensureMcp("project:p"), false);
	assert.equal(hits, first, "not again at once");
	const now = Date.now;
	Date.now = () => now() + 61_000;
	try {
		assert.equal(await ensureMcp("project:p"), false, "still down");
		assert.ok(hits > first, "tried again");
	} finally {
		Date.now = now;
	}
});
