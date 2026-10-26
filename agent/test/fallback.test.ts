// The fallback model takes over when the main one fails: a refused request, a dead endpoint, a
// model that goes silent. And it stays out of the way when the main one answers.
import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";
import { createModels, createProvider, type Model } from "@earendil-works/pi-ai";
import { openAICompletionsApi } from "@earendil-works/pi-ai/api/openai-completions.lazy";
import { fallbackProvider } from "../src/fallback.ts";

/** An OpenAI-compatible endpoint: `say` streams an answer, "500" refuses, "hang" never answers. */
async function endpoint(say: string): Promise<{ base: string; hits: () => number; key: () => string; close: () => void }> {
	let hits = 0;
	let key = "";
	const server = http.createServer((req, res) => {
		hits++;
		key = String(req.headers.authorization ?? "");
		req.resume();
		if (say === "hang") return;
		if (say === "500") {
			res.writeHead(500, { "content-type": "application/json" });
			return res.end(JSON.stringify({ error: { message: "engine glitch", type: "server_error" } }));
		}
		res.writeHead(200, { "content-type": "text/event-stream" });
		const chunk = (delta: any, finish: string | null) => `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m", choices: [{ index: 0, delta, finish_reason: finish }] })}\n\n`;
		res.write(chunk({ role: "assistant", content: say }, null));
		res.write(chunk({}, "stop"));
		res.end("data: [DONE]\n\n");
	});
	await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
	return { base: `http://127.0.0.1:${(server.address() as any).port}/v1`, hits: () => hits, key: () => key, close: () => (server.closeAllConnections(), server.close()) };
}

function setup(mainBase: string, spareBase: string, watch = { firstMs: 120_000, idleMs: 60_000 }) {
	const models = createModels();
	const model = (provider: string, baseUrl: string): Model<any> => ({ id: "m", name: "m", api: "openai-completions", provider, baseUrl, reasoning: false, input: ["text"], cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }, contextWindow: 32000, maxTokens: 1000 }) as any;
	for (const [id, base] of [["main", mainBase], ["spare", spareBase]])
		models.setProvider(createProvider({ id, baseUrl: base, auth: { apiKey: { name: id, resolve: async () => ({ auth: { apiKey: `key-${id}` } }) } }, models: [model(id, base)], api: openAICompletionsApi() } as any));
	models.setProvider(fallbackProvider("pair", models, models.getModel("main", "m")!, models.getModel("spare", "m")!, watch));
	return { models, pair: models.getModel("pair", "m")! };
}

const ask = (models: any, pair: any, signal?: AbortSignal) => models.completeSimple(pair, { messages: [{ role: "user", content: "hi", timestamp: Date.now() }] }, { signal });
const text = (m: any) => m.content.filter((b: any) => b.type === "text").map((b: any) => b.text).join("");

test("the main model answers: the fallback is not asked", async () => {
	const main = await endpoint("from main");
	const spare = await endpoint("from spare");
	const { models, pair } = setup(main.base, spare.base);
	const m = await ask(models, pair);
	assert.equal(m.stopReason, "stop");
	assert.equal(text(m), "from main");
	assert.equal(spare.hits(), 0);
	main.close();
	spare.close();
});

test("the main model fails: the fallback answers the same request", async () => {
	const main = await endpoint("500");
	const spare = await endpoint("from spare");
	const { models, pair } = setup(main.base, spare.base);
	const m = await ask(models, pair);
	assert.equal(m.stopReason, "stop", m.errorMessage);
	assert.equal(text(m), "from spare");
	assert.equal(spare.hits(), 1);
	// Each model is asked with its own key.
	assert.equal(main.key(), "Bearer key-main");
	assert.equal(spare.key(), "Bearer key-spare");
	main.close();
	spare.close();
});

test("the main endpoint is dead: the fallback answers", async () => {
	const spare = await endpoint("from spare");
	const { models, pair } = setup("http://127.0.0.1:9/v1", spare.base);
	const m = await ask(models, pair);
	assert.equal(text(m), "from spare");
	spare.close();
});

test("the main model goes silent: the fallback answers after the watch", async () => {
	const main = await endpoint("hang");
	const spare = await endpoint("from spare");
	const { models, pair } = setup(main.base, spare.base, { firstMs: 300, idleMs: 300 });
	const m = await ask(models, pair);
	assert.equal(text(m), "from spare");
	main.close();
	spare.close();
});

test("the person's Stop is not a failure: the fallback is not asked", async () => {
	const main = await endpoint("hang");
	const spare = await endpoint("from spare");
	const { models, pair } = setup(main.base, spare.base);
	const stop = new AbortController();
	setTimeout(() => stop.abort(), 200);
	const m = await ask(models, pair, stop.signal);
	assert.equal(m.stopReason, "aborted");
	assert.equal(spare.hits(), 0);
	main.close();
	spare.close();
});

test("both fail: the error reaches Pi, which retries as before", async () => {
	const main = await endpoint("500");
	const spare = await endpoint("500");
	const { models, pair } = setup(main.base, spare.base);
	const m = await ask(models, pair);
	assert.equal(m.stopReason, "error");
	main.close();
	spare.close();
});

test("a thread's turn goes on with the fallback when the main endpoint is dead", async () => {
	const fs = await import("node:fs");
	const os = await import("node:os");
	const path = await import("node:path");
	const { spawn } = await import("node:child_process");
	const { default: WebSocket } = await import("ws");
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-fallback-"));
	const spare = await endpoint("answered by the fallback");
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
		const rest = (method: string, p: string, body?: unknown) =>
			fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "X-Hermes-Session-Token": token, "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
		// Two providers, as the gear adds them; the main one points at a port nobody listens on.
		await rest("POST", "/api/providers/custom-endpoints", { name: "dead", base_url: "http://127.0.0.1:9/v1", api_key: "k", model: "m" });
		await rest("POST", "/api/providers/custom-endpoints", { name: "spare", base_url: spare.base, api_key: "k", model: "m" });
		const listed = (await rest("GET", "/api/providers/custom-endpoints")).endpoints;
		const id = (name: string) => listed.find((e: any) => e.name === name).id;
		// The gear writes both: the main model, and the fallback as `fallback_providers`.
		await rest("PUT", "/api/config", { config: { model: { provider: id("dead"), default: "m" }, fallback_providers: [{ provider: id("spare"), model: "m" }] } });

		const ws = new WebSocket(`ws://127.0.0.1:${port}/api/ws?token=${token}`);
		await new Promise((res, rej) => (ws.on("open", res), ws.on("error", rej)));
		let n = 0;
		const wait = new Map<number, (v: any) => void>();
		const notes: any[] = [];
		let finish: (v: any) => void = () => {};
		const done = new Promise<any>((res) => (finish = res));
		ws.on("message", (d) => {
			const m = JSON.parse(String(d));
			if (m.id && wait.has(m.id)) (wait.get(m.id) as any)(m);
			if (m.method === "event" && m.params.type === "session.info" && "fallback" in m.params.payload) notes.push(m.params.payload.fallback);
			if (m.method === "event" && m.params.type === "message.complete") finish(m.params.payload);
		});
		const call = (method: string, params: any = {}) =>
			new Promise<any>((res, rej) => {
				const i = ++n;
				wait.set(i, (m) => (m.error ? rej(new Error(m.error.message)) : res(m.result)));
				ws.send(JSON.stringify({ jsonrpc: "2.0", id: i, method, params }));
			});
		// A project's thread: the model is the general one for it too.
		const project = (await rest("POST", "/api/projects", { name: "Site" })).project;
		const sid = (await call("session.create", { profile: `project:${project.id}` })).session_id;
		await call("prompt.submit", { session_id: sid, text: "say hello" });
		const finished = await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error(`no turn ended:\n${err}`)), 60000))]);
		assert.match(String(finished.text), /answered by the fallback/);
		assert.match(err, /answering with/);
		// The bar is told: the fallback answered, and why.
		assert.equal(notes.at(-1)?.model, "m");
		assert.match(String(notes.at(-1)?.reason), /Connection error|ECONNREFUSED|fetch failed/i);
		ws.close();
	} finally {
		child.kill();
		spare.close();
		fs.rmSync(home, { recursive: true, force: true });
	}
});
