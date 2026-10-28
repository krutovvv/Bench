// The fallback model takes over when the main one fails: a refused request, a dead endpoint, a
// model that goes silent. And it stays out of the way when the main one answers.
import assert from "node:assert/strict";
import http from "node:http";
import { test } from "node:test";
import { createModels, createProvider, type Model } from "@earendil-works/pi-ai";
import { openAICompletionsApi } from "@earendil-works/pi-ai/api/openai-completions.lazy";
import { fallbackProvider } from "../src/runtime/fallback.ts";

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
	const { startServer, message } = await import("./server.ts");
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-fallback-"));
	const spare = await endpoint("answered by the fallback");
	const s = await startServer(home);
	try {
		// Two providers, as the gear adds them; the main one points at a port nobody listens on.
		await s.rest("POST", "/api/providers/custom-endpoints", { name: "dead", base_url: "http://127.0.0.1:9/v1", api_key: "k", model: "m" });
		await s.rest("POST", "/api/providers/custom-endpoints", { name: "spare", base_url: spare.base, api_key: "k", model: "m" });
		const listed = (await s.rest("GET", "/api/providers/custom-endpoints")).endpoints;
		const id = (name: string) => listed.find((e: any) => e.name === name).id;
		// The gear writes both: the main model, and the fallback as `fallback_providers`.
		await s.rest("PUT", "/api/config", { config: { model: { provider: id("dead"), default: "m" }, fallback_providers: [{ provider: id("spare"), model: "m" }] } });

		// A project's thread: the model is the general one for it too.
		const project = (await s.rest("POST", "/api/projects", { name: "Site" })).project;
		const { thread } = await s.call("thread.create", { project: project.id, cwd: null });
		await s.call("thread.send", message(thread, "say hello"));
		await s.until(() => s.of(thread).some((e) => e.kind === "run" && !e.value));
		const answer = (await s.call("thread.open", { thread, since: 0 })).items.filter((i) => i.kind === "text").at(-1);
		assert.match(answer?.kind === "text" ? answer.text : "", /answered by the fallback/);
		assert.match(s.log(), /answering with/);
		// The bar is told: the fallback answered, and why.
		const note = s.of(thread).flatMap((e) => (e.kind === "info" && e.value.fallback ? [e.value.fallback] : [])).at(-1);
		assert.equal(note?.model, "m");
		assert.match(String(note?.reason), /Connection error|ECONNREFUSED|fetch failed/i);
	} finally {
		s.close();
		spare.close();
		fs.rmSync(home, { recursive: true, force: true });
	}
});
