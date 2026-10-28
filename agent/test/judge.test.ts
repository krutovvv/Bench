// The step limit and the evaluator, on a temporary BENCH_HOME and a stub System One server
// (POST /v1/systemone, the answers it is told to give): a turn is counted from the person's
// message, past the limit calls are refused with the reason; with the evaluator on, a long search
// comes likeliest first, an off course is said, an unfinished answer is sent back once; off or
// failing, everything stays as it was.

import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-judge-"));
process.env.BENCH_HOME = home;

const { setHost } = await import("../src/host.ts");
const { setSetting, writeAgent, readAgent } = await import("../src/config.ts");
const { seeRequest, seeResult, turnOf } = await import("../src/builtin/turn.ts");
const { stepGate, stepNote } = await import("../src/builtin/steps.ts");
const { endpointOf } = await import("../src/judge/client.ts");
const { rankSearch } = await import("../src/judge/search.ts");
const { checkCourse, DRIFT_EVERY } = await import("../src/judge/drift.ts");
const { checkDone } = await import("../src/judge/done.ts");
const { itemsOf } = await import("../src/runtime/journal.ts");

const counted: string[] = [];
const stopped: string[] = [];
setHost({
	thread: () => ({ id: "t", agent: "project:p", cwd: home, source: "local", project: "p", inserts: [] }),
	count: (_c: string, what: string) => void counted.push(what),
	stop: (id: string) => void stopped.push(id),
} as any);

// The stub server answers each question with the probabilities the test sets.
let reply: (body: any) => any = () => ({ answers: {} });
const seen: any[] = [];
const server = http.createServer((req, res) => {
	let data = "";
	req.on("data", (c) => (data += c));
	req.on("end", () => {
		const body = JSON.parse(data);
		seen.push({ url: req.url, auth: req.headers.authorization, body });
		res.setHeader("Content-Type", "application/json");
		res.end(JSON.stringify(reply(body)));
	});
});
await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${(server.address() as any).port}`;
after(() => {
	server.close();
	fs.rmSync(home, { recursive: true, force: true });
});

const user = (text: string) => ({ role: "user", content: [{ type: "text", text }] });
const out = (text: string) => ({ output: [{ type: "text", text }] });
const choice = (name: string, probabilities: Record<string, number>) => {
	const top = Object.entries(probabilities).sort((a, b) => b[1] - a[1])[0][0];
	return { answers: { [name]: { type: "choice", choice: top, confidence: 0.9, probabilities } } };
};

test("a turn starts at the person's message, not at the evaluator's; the plan carries over", () => {
	seeRequest("c1", [user("Make the game")]);
	seeResult("c1", { name: "todo", arguments: { todos: [{ content: "Write it", status: "in_progress" }] } }, out("ok"));
	turnOf("c1").steps = 3;
	seeRequest("c1", [user("Make the game"), { role: "assistant", content: [] }, { role: "toolResult", content: [] }]);
	assert.equal(turnOf("c1").steps, 3, "a tool result goes on with the turn");
	seeRequest("c1", [user("<evaluator>\nCheck it.\n</evaluator>")]);
	assert.equal(turnOf("c1").steps, 3, "the evaluator's message is not a new turn");
	seeRequest("c1", [user("Now add sounds")]);
	assert.deepEqual([turnOf("c1").request, turnOf("c1").steps, turnOf("c1").plan], ["Now add sounds", 0, ["[in_progress] Write it"]]);
});

test("past the project's step limit calls are refused with the reason, and the run stops if they go on", () => {
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: 3 });
	seeRequest("c2", [user("Do it")]);
	for (let i = 0; i < 3; i++) assert.equal(stepGate("c2"), undefined);
	assert.equal(stepNote("c2")?.role, "user");
	assert.match(stepGate("c2")!, /allows 3 steps.*final answer/s);
	for (let i = 0; i < 5; i++) stepGate("c2");
	assert.deepEqual(stopped, ["t"], "stopped once the refusals go on");
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: undefined });
	seeRequest("c2", [user("Again")]);
	for (let i = 0; i < 50; i++) assert.equal(stepGate("c2"), undefined, "no limit: every step runs");
	assert.equal(stepNote("c2"), undefined);
});

const grep = { name: "grep", arguments: { pattern: "jump" } };
const lines = Array.from({ length: 20 }, (_, i) => `src/game.js:${i + 1}:line ${i + 1}`).join("\n");

test("off, the evaluator is never asked and nothing changes", async () => {
	setSetting("judge", { enabled: false, base_url: base });
	seeRequest("c3", [user("Make the tap jump")]);
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined);
	assert.equal(seen.length, 0);
});

test("a long search comes likeliest first, every line kept", async () => {
	setSetting("judge", { enabled: true, base_url: `${base}/v1/`, model: "local", api_key: "sk-none" });
	reply = () => choice("place", { "7": 0.6, "12": 0.3, "1": 0.01 });
	const r = await rankSearch("c3", grep, out(lines + "\n… 4 more: narrow the path or the pattern."));
	const text = r.output[0].text as string;
	assert.match(text, /^\[Evaluator[^\n]*\]\nsrc\/game\.js:7:line 7 +\(60%\)\nsrc\/game\.js:12:line 12 +\(30%\)\n/);
	assert.equal(text.split("\n").filter((l) => l.startsWith("src/game.js:")).length, 20, "nothing dropped");
	assert.match(text, /The other 18, in their order:\nsrc\/game\.js:1:line 1\n/);
	assert.match(text, /4 more/);
	const req = seen.at(-1);
	assert.deepEqual([req.url, req.auth, req.body.model, Object.keys(req.body.questions.place.criteria).length], ["/v1/systemone", "Bearer sk-none", "local", 20]);
	assert.equal(req.body.state.request, "Make the tap jump");
});

test("a short list, an unsure answer or a failing server leave the result as it is", async () => {
	assert.equal(await rankSearch("c3", grep, out(lines.split("\n").slice(0, 5).join("\n"))), undefined);
	reply = () => choice("place", { "1": 0.08, "2": 0.07 });
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined);
	reply = () => {
		throw new Error("down");
	};
	setSetting("judge", { enabled: true, base_url: "http://127.0.0.1:9" });
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined, "refused: the classic way");
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined, "and quiet a while after");
	setSetting("judge", { enabled: true, base_url: base });
});

test("an address is taken with or without /v1", () => {
	assert.equal(endpointOf("https://api.typesafe.ai"), "https://api.typesafe.ai/v1/systemone");
	assert.equal(endpointOf("http://h:10292/v1/"), "http://h:10292/v1/systemone");
});

test("the journal leaves the evaluator's message out of the thread", () => {
	const items = itemsOf({ id: 5, kind: "pi.user", model: [user("<evaluator>\nCheck it.\n</evaluator>")] });
	assert.deepEqual(items, []);
});

const steps = (c: string, n: number) => {
	for (let i = 0; i < n; i++) {
		stepGate(c);
		seeResult(c, { name: "bash", arguments: { command: `node tools/probe-${i}.mjs` } }, out("died at frame 800"));
	}
};

test("the course is checked every so many steps; only a sure 'off' is said", async () => {
	seeRequest("c4", [user("Make the game")]);
	reply = () => choice("course", { on_track: 0.1, drifting: 0.85, looping: 0.04, done: 0.01 });
	steps("c4", DRIFT_EVERY - 1);
	assert.equal(await checkCourse("c4"), undefined, "not yet");
	steps("c4", 1);
	assert.match((await checkCourse("c4"))!, /did not ask for/);
	assert.equal(seen.at(-1).body.state.last_steps.length, DRIFT_EVERY);
	assert.equal(await checkCourse("c4"), undefined, "once per stretch");
	steps("c4", DRIFT_EVERY);
	reply = () => choice("course", { on_track: 0.4, drifting: 0.6, looping: 0, done: 0 });
	assert.equal(await checkCourse("c4"), undefined, "unsure: quiet");
});

test("an unfinished answer after work is sent back once; a question, a short turn or the limit are not", async () => {
	const answer = { role: "assistant", content: [{ type: "text", text: "The game is ready." }] };
	seeRequest("c5", [user("Make the game")]);
	steps("c5", 2);
	reply = () => choice("verdict", { done: 0.05, question: 0.01, unfinished: 0.9, unchecked: 0.02, off_target: 0.02 });
	assert.equal(await checkDone("c5", answer), undefined, "a short turn");
	steps("c5", 5);
	const back = await checkDone("c5", answer);
	assert.match(back!, /^<evaluator>\n.*not done.*\n<\/evaluator>$/s);
	assert.equal(await checkDone("c5", answer), undefined, "only once a turn");
	seeRequest("c6", [user("Fix the bug")]);
	steps("c6", 6);
	reply = () => choice("verdict", { done: 0.02, question: 0.9, unfinished: 0.04, unchecked: 0.02, off_target: 0.02 });
	assert.equal(await checkDone("c6", answer), undefined, "a question to the person stands");
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: 4 });
	seeRequest("c7", [user("Fix the bug")]);
	steps("c7", 6);
	reply = () => choice("verdict", { done: 0.02, question: 0.01, unfinished: 0.95, unchecked: 0.01, off_target: 0.01 });
	assert.equal(await checkDone("c7", answer), undefined, "stopped by the limit: the answer stands");
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: undefined });
	assert.ok(counted.includes("call") && counted.includes("nudge"), "the thread's meters count both");
	assert.ok(fs.readFileSync(path.join(home, "judge.log"), "utf8").includes('"point":"done"'));
});
