// A run that dies must leave a reason, not an empty turn: the person sees why it stopped while
// it happens, and the reason stays with the answer, so a reopened thread still says what went
// wrong. Driven over the real WebSocket against an endpoint that refuses connections, which is
// the same path a dead provider or a hung upstream takes.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import WebSocket from "ws";

test("a stopped run says why, live and in the history", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-fail-"));
	// No retries: the default backs off for about a minute, and this run is meant to fail.
	process.env.BENCH_HOME = home;
	const { setSetting } = await import("../src/config.ts");
	setSetting("agent", { api_max_retries: 0 });
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], {
		env: { ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1" },
		cwd: path.join(import.meta.dirname, ".."),
		stdio: ["ignore", "ignore", "pipe"],
	});
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	try {
		const file = path.join(home, "server.json");
		for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
		assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
		const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));

		// An endpoint of our own that answers 500 at once: the model call fails as surely as a
		// dead upstream, and in a second rather than a timeout.
		const dead = http.createServer((_req, res) => {
			res.writeHead(500, { "content-type": "application/json" });
			res.end(JSON.stringify({ error: { message: "upstream refused: user abnormal", type: "server_error" } }));
		});
		await new Promise<void>((res) => dead.listen(0, "127.0.0.1", res));
		const base = `http://127.0.0.1:${(dead.address() as any).port}/v1`;

		// The provider goes in through the agent's own code, in the same home the server reads.
		process.env.BENCH_HOME = home;
		const { saveProvider } = await import("../src/config.ts");
		saveProvider({ id: "dead", name: "dead", base_url: base, api_key: "k", api_mode: "chat_completions", models: { "m": {} } });

		const ws = new WebSocket(`ws://127.0.0.1:${port}/api/ws?token=${token}`);
		await new Promise((res, rej) => {
			ws.on("open", res);
			ws.on("error", rej);
		});
		let n = 0;
		const wait = new Map<number, (v: any) => void>();
		const notes: any[] = [];
		let finishTurn: (v: any) => void = () => {};
		let done: Promise<any> = new Promise((res) => (finishTurn = res));
		ws.on("message", (d) => {
			const m = JSON.parse(String(d));
			if (m.id && wait.has(m.id)) (wait.get(m.id) as any)(m);
			wait.delete(m.id);
			// Events ride inside the gateway's envelope: {method:"event", params:{type,…}}.
			if (m.method !== "event") return;
			if (m.params.type === "message.fail") notes.push(m.params.payload);
			if (m.params.type === "message.complete") finishTurn(m.params.payload);
		});
		const call = (method: string, params: any = {}) =>
			new Promise<any>((res, rej) => {
				const id = ++n;
				wait.set(id, (m) => (m.error ? rej(new Error(m.error.message)) : res(m.result)));
				ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
			});

		const id = (await call("session.create", { profile: "default" })).session_id;
		await call("slash.exec", { session_id: id, command: "/model m --provider dead" });
		await call("prompt.submit", { session_id: id, text: "say hello" });

		const finished = await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error(`no turn ended:\n${err}`)), 60000))]);
		// The reason arrives on its own, and it is part of the finished answer too.
		assert.ok(notes.length, "the dead run said nothing about itself");
		assert.match(String(notes[0].text), /The run stopped/);
		assert.match(String(finished.text), /The run stopped/);
		// And it stays: a reopened thread reads it out of the journal.
		const opened = await call("thread.open", { session_id: id });
		const fail = opened.items.filter((i: any) => i.kind === "fail").at(-1);
		assert.ok(fail, "the dead turn left nothing behind in the journal");
		assert.match(String(fail.text), /\S/);
		// The next turn dies the same way: it says so again, on its own. A turn's failure is
		// its own; it never sticks to the next one nor silences its own failure.
		done = new Promise((res) => (finishTurn = res));
		await call("prompt.submit", { session_id: id, text: "try again" });
		const again = await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error("no second turn")), 60000))]);
		assert.equal(notes.length, 2, "the second failure was said too");
		assert.equal((String(again.text).match(/The run stopped/g) ?? []).length, 1, "once, not carried over");
		const twice = (await call("thread.open", { session_id: id })).items.filter((i: any) => i.kind === "fail");
		assert.equal(twice.length, 2, "each dead turn left its own note");
		ws.close();
		child.kill();
		dead.close();
	} finally {
		fs.rmSync(home, { recursive: true, force: true });
	}
});