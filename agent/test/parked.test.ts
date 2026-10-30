// Work cut off again and again (`runtime/parked.ts`), on real Pi across real processes: killed, it
// counts until Pi parks it and the thread says so, and a retry runs it; closed, it never counts.

import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-parked-"));
process.env.BENCH_HOME = home;
process.env.BENCH_AGENT_NO_MCP = "1";
const { saveProvider, writeAgent } = await import("../src/config.ts");

/** The requests the model has not answered: it holds each, so a run stays at the model. */
let asked = 0;
const model = http.createServer((req, res) => {
	let body = "";
	req.on("data", (d) => (body += d));
	req.on("end", () => {
		if (body.includes("You name chat sessions")) {
			res.writeHead(200, { "content-type": "text/event-stream" });
			return void res.end('data: {"id":"x","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"A title"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n');
		}
		asked++;
	});
});
await new Promise<void>((r) => model.listen(0, "127.0.0.1", r));
const port = (model.address() as any).port;
const children: ChildProcess[] = [];
after(() => {
	for (const c of children) c.kill("SIGKILL");
	model.closeAllConnections();
	model.close();
	fs.rmSync(home, { recursive: true, force: true });
});

/** A life of the agent, and the words it says. */
function life(dir: string) {
	const child = spawn(process.execPath, [new URL("./parked.child.ts", import.meta.url).pathname], {
		env: { ...process.env, BENCH_HOME: dir },
		stdio: ["pipe", "pipe", "ignore"],
	});
	children.push(child);
	const said: string[] = [];
	child.stdout!.on("data", (d) => said.push(...String(d).split("\n").filter(Boolean)));
	const word = async (n = 1) => {
		const end = Date.now() + 15000;
		while (said.length < n) {
			if (Date.now() > end) throw new Error(`timed out; said ${said}`);
			await new Promise((r) => setTimeout(r, 20));
		}
		return said[n - 1];
	};
	const gone = new Promise((done) => child.on("exit", done));
	return { child, word, gone };
}

const untilAsked = async (n: number) => {
	const end = Date.now() + 15000;
	while (asked < n) {
		if (Date.now() > end) throw new Error(`the model was asked ${asked} times, not ${n}`);
		await new Promise((r) => setTimeout(r, 20));
	}
};

saveProvider({ id: "local", name: "local", base_url: `http://127.0.0.1:${port}/v1`, api_key: "k", api_mode: "chat_completions", models: { stub: { context_length: 100000 } } } as any);
writeAgent("default", { defaultProvider: "local", defaultModel: "stub" });

test("closed each time, more often than the limit, the work is never parked", async () => {
	for (let lives = 0; lives < 7; lives++) {
		const before = asked;
		const l = life(home);
		assert.equal(await l.word(), "not-parked");
		await untilAsked(before + 1);
		l.child.kill("SIGTERM");
		await l.gone;
	}
});

test("killed each time, the work is parked, the thread says so, and a retry runs it", async () => {
	asked = 0;
	let found = "";
	let lives = 0;
	for (; lives < 8; lives++) {
		const l = life(home);
		found = await l.word();
		if (found === "parked") {
			l.child.stdin!.write("retry\n");
			assert.equal(await l.word(2), "not-parked");
			const before = asked;
			await untilAsked(before + 1);
			l.child.kill("SIGKILL");
			await l.gone;
			break;
		}
		await untilAsked(lives + 1);
		l.child.kill("SIGKILL");
		await l.gone;
	}
	assert.equal(found, "parked", `not parked after ${lives} lives`);
});
