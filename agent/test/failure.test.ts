// A run that dies must leave a reason, not an empty turn: the person sees why it stopped while
// it happens, and the reason stays with the answer, so a reopened thread still says what went
// wrong. Driven over the real WebSocket against an endpoint that refuses connections, which is
// the same path a dead provider or a hung upstream takes.
import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { message, startServer } from "./server.ts";

test("a stopped run says why, live and in the history", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-fail-"));
	// No retries: the default backs off for about a minute, and this run is meant to fail.
	process.env.BENCH_HOME = home;
	const { setSetting, saveProvider } = await import("../src/config.ts");
	setSetting("agent", { api_max_retries: 0 });
	// An endpoint of our own that answers 500 at once: the model call fails as surely as a dead
	// upstream, and in a second rather than a timeout.
	const dead = http.createServer((_req, res) => {
		res.writeHead(500, { "content-type": "application/json" });
		res.end(JSON.stringify({ error: { message: "upstream refused: user abnormal", type: "server_error" } }));
	});
	await new Promise<void>((res) => dead.listen(0, "127.0.0.1", res));
	const base = `http://127.0.0.1:${(dead.address() as any).port}/v1`;
	// The provider goes in through the agent's own code, in the same home the server reads.
	saveProvider({ id: "dead", name: "dead", base_url: base, api_key: "k", api_mode: "chat_completions", models: { m: {} } });
	const s = await startServer(home);
	try {
		const { thread } = await s.call("thread.create", { project: null, cwd: null });
		await s.call("thread.command", { thread, command: "/model m --provider dead" });
		const ended = (n: number) => () => s.of(thread).filter((e) => e.kind === "run" && !e.value).length >= n;
		const fails = async () => (await s.call("thread.open", { thread, since: 0 })).items.filter((i) => i.kind === "fail");
		// The reason is written to the journal as the run ends: it may land a moment after.
		const failsAtLeast = async (n: number) => {
			for (let i = 0; i < 50 && (await fails()).length < n; i++) await new Promise((r) => setTimeout(r, 100));
			return fails();
		};

		await s.call("thread.send", message(thread, "say hello"));
		await s.until(ended(1));
		// The reason stays: a reopened thread reads it out of the journal.
		const first = await failsAtLeast(1);
		assert.equal(first.length, 1, "the dead turn left its reason in the journal");
		assert.match(first[0].kind === "fail" ? first[0].text : "", /upstream refused|500/);

		// The next turn dies the same way: it says so again, on its own. A turn's failure is its
		// own; it never sticks to the next one nor silences its own failure.
		await s.call("thread.send", message(thread, "try again"));
		await s.until(ended(2));
		assert.equal((await failsAtLeast(2)).length, 2, "each dead turn left its own note");
	} finally {
		s.close();
		dead.close();
		fs.rmSync(home, { recursive: true, force: true });
	}
});
