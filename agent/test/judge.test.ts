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
const { markNotRun, seeRequest, seeResult, turnOf } = await import("../src/builtin/turn.ts");
const { stepGate, stepNote } = await import("../src/builtin/steps.ts");
const { endpointOf, checkJudge, fitState } = await import("../src/judge/client.ts");
const { atResult, atAnswer } = await import("../src/judge/index.ts");
const { DRIFT_EVERY } = await import("../src/judge/drift.ts");
const { PLAN_EVERY } = await import("../src/judge/plan.ts");
const { itemsOf, PLAN_KIND } = await import("../src/runtime/journal.ts");

const counted: string[] = [];
const stopped: string[] = [];
const written: any[] = [];
setHost({
	thread: () => ({ id: "t", agent: "project:p", cwd: home, source: "local", project: "p", inserts: [] }),
	count: (_c: string, what: string) => void counted.push(what),
	stop: (id: string) => void stopped.push(id),
	write: (_c: string, entry: any) => void written.push(entry),
} as any);

// The project's switch on (off by default); each test turns the gear's connection on or off.
writeAgent("project:p", { ...readAgent("project:p"), evaluator: { enabled: true } });

// The old single-point helpers, through the dispatcher: what the model would read.
const rankSearch = async (c: string, call: any, result: any) => (await atResult(c, call, result)).result;
const checkCourse = async (c: string) => (await atResult(c, { name: "bash", arguments: { command: "true" } }, out("ok"))).notes[0];
const checkDone = (c: string, answer: any) => atAnswer(c, answer);

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
	assert.deepEqual([turnOf("c1").request, turnOf("c1").steps, turnOf("c1").plan.map((p) => `[${p.status}] ${p.content}`)], ["Now add sounds", 0, ["[in_progress] Write it"]]);
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
	reply = () => choice("search.place", { "7": 0.6, "12": 0.3, "1": 0.01 });
	const r = await rankSearch("c3", grep, out(lines + "\n… 4 more: narrow the path or the pattern."));
	const text = r.output[0].text as string;
	assert.match(text, /^\[Evaluator[^\n]*\]\nsrc\/game\.js:7:line 7 +\(60%\)\nsrc\/game\.js:12:line 12 +\(30%\)\n/);
	assert.equal(text.split("\n").filter((l) => l.startsWith("src/game.js:")).length, 20, "nothing dropped");
	assert.match(text, /The other 18, in their order:\nsrc\/game\.js:1:line 1\n/);
	assert.match(text, /4 more/);
	const req = seen.at(-1);
	assert.deepEqual([req.url, req.auth, req.body.model, Object.keys(req.body.questions["search.place"].criteria).length], ["/v1/systemone", "Bearer sk-none", "local", 20]);
	assert.equal(req.body.state.request, "Make the tap jump");
});

test("a short list, an unsure answer or a failing server leave the result as it is", async () => {
	assert.equal(await rankSearch("c3", grep, out(lines.split("\n").slice(0, 5).join("\n"))), undefined);
	reply = () => choice("search.place", { "1": 0.08, "2": 0.07 });
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
	reply = () => choice("course.course", { on_track: 0.1, drifting: 0.85, looping: 0.04, done: 0.01 });
	steps("c4", DRIFT_EVERY - 1);
	assert.equal(await checkCourse("c4"), undefined, "not yet");
	steps("c4", 1);
	assert.match((await checkCourse("c4"))!, /did not ask for/);
	assert.equal(seen.at(-1).body.state.last_steps.length, DRIFT_EVERY);
	assert.equal(await checkCourse("c4"), undefined, "once per stretch");
	steps("c4", DRIFT_EVERY);
	reply = () => choice("course.course", { on_track: 0.4, drifting: 0.6, looping: 0, done: 0 });
	assert.equal(await checkCourse("c4"), undefined, "unsure: quiet");
});

test("an unfinished answer after work is sent back once; a question, a short turn or the limit are not", async () => {
	const answer = { role: "assistant", content: [{ type: "text", text: "The game is ready." }] };
	seeRequest("c5", [user("Make the game")]);
	steps("c5", 2);
	reply = () => choice("done.verdict", { done: 0.05, question: 0.01, unfinished: 0.9, unchecked: 0.02, off_target: 0.02 });
	assert.equal(await checkDone("c5", answer), undefined, "a short turn");
	steps("c5", 5);
	const back = await checkDone("c5", answer);
	assert.match(back!, /^<evaluator>\n.*not done.*\n<\/evaluator>$/s);
	assert.equal(await checkDone("c5", answer), undefined, "only once a turn");
	seeRequest("c6", [user("Fix the bug")]);
	steps("c6", 6);
	reply = () => choice("done.verdict", { done: 0.02, question: 0.9, unfinished: 0.04, unchecked: 0.02, off_target: 0.02 });
	assert.equal(await checkDone("c6", answer), undefined, "a question to the person stands");
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: 4 });
	seeRequest("c7", [user("Fix the bug")]);
	steps("c7", 6);
	reply = () => choice("done.verdict", { done: 0.02, question: 0.01, unfinished: 0.95, unchecked: 0.01, off_target: 0.01 });
	assert.equal(await checkDone("c7", answer), undefined, "stopped by the limit: the answer stands");
	writeAgent("project:p", { ...readAgent("project:p"), stepLimit: undefined });
	assert.ok(counted.includes("call") && counted.includes("nudge"), "the thread's meters count both");
	assert.ok(fs.readFileSync(path.join(home, "judge.log"), "utf8").includes('"point":"done"'));
});

test("the gear's Check asks one question with the fields as typed, the saved key when none is typed", async () => {
	setSetting("judge", { enabled: false, base_url: base, api_key: "sk-saved" });
	reply = () => ({ model: "h2oai/h2o-lightning-4b", answers: { billing: { type: "noul", noul: 0.93 } } });
	const ok = await checkJudge({ base_url: base, model: "" });
	assert.equal(ok.ok, true);
	assert.match(ok.message, /^Works · h2oai\/h2o-lightning-4b · \d+ ms$/);
	assert.equal(seen.at(-1).auth, "Bearer sk-saved");
	reply = () => ({ answers: {} });
	assert.deepEqual(await checkJudge({ base_url: base }), { ok: false, message: "It answered, but not as a System One server" });
	assert.equal((await checkJudge({ base_url: "http://127.0.0.1:9" })).ok, false);
	assert.equal((await checkJudge({})).ok, false);
});

const todo = (todos: { content: string; status: string }[]) => ({ id: "call-plan", name: "todo", arguments: { todos: todos.map((t, i) => ({ id: `${i + 1}`, ...t })) } });

test("the evaluator marks the plan forward every few steps, in one request with the course; the thread gets an entry", async () => {
	setSetting("judge", { enabled: true, base_url: base });
	seeRequest("c8", [user("Make the game")]);
	seeResult("c8", todo([{ content: "Write the game", status: "in_progress" }, { content: "Test it", status: "pending" }, { content: "Deliver", status: "pending" }]), out("ok"));
	written.length = 0;
	reply = (body) => ({
		answers: {
			...(body.questions["plan.item0"] ? { "plan.item0": choice("x", { completed: 0.92, in_progress: 0.07, pending: 0.01 }).answers.x } : {}),
			...(body.questions["plan.item1"] ? { "plan.item1": choice("x", { pending: 0.6, in_progress: 0.3, completed: 0.1 }).answers.x } : {}),
			...(body.questions["plan.item2"] ? { "plan.item2": choice("x", { pending: 0.9, in_progress: 0.05, completed: 0.05 }).answers.x } : {}),
		},
	});
	for (let i = 0; i < PLAN_EVERY - 1; i++) {
		stepGate("c8");
		assert.deepEqual((await atResult("c8", { name: "bash", arguments: { command: "true" } }, out("ok"))).notes, []);
	}
	stepGate("c8");
	const r = await atResult("c8", { name: "bash", arguments: { command: "true" } }, out("ok"));
	const q = seen.at(-1).body.questions;
	assert.deepEqual(Object.keys(q.plan ? {} : q).sort(), ["plan.item0", "plan.item1", "plan.item2"]);
	assert.deepEqual(Object.keys(q["plan.item0"].criteria), ["in_progress", "pending", "completed"], "its status as it stands first");
	assert.match(r.notes[0], /done: "Write the game"; now: "Test it"/);
	assert.deepEqual(turnOf("c8").plan.map((p) => p.status), ["completed", "in_progress", "pending"]);
	assert.equal(written.length, 1);
	assert.equal(written[0].kind, PLAN_KIND);
	const items = itemsOf({ id: 9, kind: PLAN_KIND, data: written[0].data });
	assert.deepEqual([items[0].kind, (items[0] as any).id, (items[0] as any).todos.map((t: any) => t.status)], ["plan", "call-plan", ["completed", "in_progress", "pending"]]);
});

test("an item never moves back, an unsure answer leaves it, and the model's own todo starts the marks again", async () => {
	reply = (body) => ({
		answers: Object.fromEntries(Object.keys(body.questions).map((k) => [k, choice("x", { pending: 0.95, in_progress: 0.04, completed: 0.01 }).answers.x])),
	});
	written.length = 0;
	for (let i = 0; i < PLAN_EVERY; i++) {
		stepGate("c8");
		await atResult("c8", { name: "bash", arguments: { command: "true" } }, out("ok"));
	}
	assert.deepEqual(turnOf("c8").plan.map((p) => p.status), ["completed", "in_progress", "pending"], "not back to pending");
	assert.equal(written.length, 0);
	seeResult("c8", todo([{ content: "Write the game", status: "completed" }, { content: "Add sounds", status: "in_progress" }]), out("ok"));
	assert.equal(turnOf("c8").planAt, turnOf("c8").steps, "read again from the model's plan in a few steps");
	assert.equal(turnOf("c8").plan[1].content, "Add sounds");
});

test("a project that did not turn the evaluator on is never asked about, the gear's connection on", async () => {
	setSetting("judge", { enabled: true, base_url: base });
	writeAgent("project:p", { ...readAgent("project:p"), evaluator: undefined });
	try {
		const before = seen.length;
		seeRequest("np", [user("Make the game")]);
		for (let i = 0; i < 11; i++) {
			stepGate("np");
			assert.deepEqual(await atResult("np", grep, out(lines)), { notes: [] });
		}
		assert.equal(seen.length, before);
	} finally {
		writeAgent("project:p", { ...readAgent("project:p"), evaluator: { enabled: true } });
	}
});

test("off, no point asks and the hooks change nothing", async () => {
	setSetting("judge", { enabled: false, base_url: base });
	const before = seen.length;
	seeRequest("c9", [user("Make the game")]);
	seeResult("c9", todo([{ content: "Write the game", status: "in_progress" }]), out("ok"));
	for (let i = 0; i < DRIFT_EVERY + 1; i++) {
		stepGate("c9");
		assert.deepEqual(await atResult("c9", grep, out(lines)), { notes: [] });
	}
	assert.equal(await atAnswer("c9", { role: "assistant", content: [{ type: "text", text: "Done." }] }), undefined);
	assert.equal(seen.length, before);
});

const { atRequest, atCall } = await import("../src/judge/index.ts");
const { fitResultFor } = await import("../src/builtin/window.ts");
const { seeAnswer } = await import("../src/builtin/turn.ts");

test("at a turn's start the fitting skill is named for the first request, once", async () => {
	setSetting("judge", { enabled: true, base_url: base });
	const dir = path.join(home, "projects", "p", "skills", "price-check");
	fs.mkdirSync(dir, { recursive: true });
	fs.writeFileSync(path.join(dir, "SKILL.md"), "---\nname: price-check\ndescription: Compare a product's prices across shops\n---\nSteps.");
	seeRequest("s1", [user("Compare Arctic prices")]);
	reply = (body) => {
		const k = Object.keys(body.questions.start ? {} : body.questions).find((q) => q === "start.skill")!;
		const opts = Object.keys(body.questions[k].criteria);
		assert.equal(opts[0], "none", "naming none first");
		return choice("start.skill", { none: 0.1, [opts[1]]: 0.85 });
	};
	const hints = await atRequest("s1");
	assert.equal(hints.length, 1);
	assert.match(hints[0], /the skill "price-check" fits it: load it with skill_view/);
	assert.deepEqual(await atRequest("s1"), [], "once a turn");
});

test("a failed step is judged; a sure word goes with the error", async () => {
	seeRequest("f1", [user("Find the price on DNS")]);
	reply = (body) => {
		assert.match(body.state.error, /403/);
		return choice("failure.failure", { go_on: 0.1, retry: 0.05, fix: 0.05, other_way: 0.75, ask: 0.05 });
	};
	const r = await atResult("f1", { name: "bash", arguments: { command: "curl dns-shop.ru" } }, { output: [{ type: "text", text: "HTTP 403" }], isError: true });
	assert.match(r.notes[0], /this way does not work here/);
	reply = () => choice("failure.failure", { go_on: 0.5, retry: 0.3, fix: 0.1, other_way: 0.05, ask: 0.05 });
	assert.deepEqual((await atResult("f1", { name: "bash", arguments: {} }, { output: [{ type: "text", text: "boom" }], isError: true })).notes, []);
});

test("with approvals on, a command the evaluator judges hard to undo is asked; a harmless one is not", async () => {
	reply = () => choice("risk.risk", { reads: 0.1, changes: 0.2, harm: 0.7 });
	assert.match((await atCall("a1", { name: "bash", arguments: { command: "psql -c 'drop table users'" } }, true)).approve!, /hard to undo/);
	assert.equal((await atCall("a1", { name: "bash", arguments: { command: "psql -c 'drop table users'" } }, false)).approve, undefined, "approvals off: not asked");
	reply = () => choice("risk.risk", { reads: 0.9, changes: 0.05, harm: 0.05 });
	assert.equal((await atCall("a1", { name: "bash", arguments: { command: "ls" } }, true)).approve, undefined);
});

test("the plan's check sees the agent's own words and longer results", async () => {
	seeRequest("pe", [user("Compare prices")]);
	seeResult("pe", todo([{ content: "Ozon price", status: "in_progress" }, { content: "DNS price", status: "pending" }]), out("ok"));
	seeAnswer("pe", { content: [{ type: "text", text: "Ozon: the lowest is 9 753 ₽ from the seller Arctic." }] });
	reply = () => ({ answers: {} });
	for (let i = 0; i < PLAN_EVERY; i++) {
		stepGate("pe");
		const c = { name: "mcp__ozon__ozon_search", arguments: { q: "arctic" } };
		const r = out("price 9753 seller Arctic " + "x".repeat(400));
		seeResult("pe", c, r);
		await atResult("pe", c, r);
	}
	const st = seen.at(-1).body.state;
	assert.match(st.agent_said[0], /9 753/);
	assert.ok(st.recent_results.at(-1).length > 300, "results at more length than a line");
});

test("a long result keeps the parts the request needs, in order; without the evaluator, head and tail", async () => {
	seeRequest("x1", [user("Find the price of Liquid Freezer III")]);
	const page = Array.from({ length: 60 }, (_, i) => (i === 41 ? "PRICE: Liquid Freezer III Pro 360 — 9 753 ₽\n" : `menu link ${i} `.repeat(90) + "\n")).join("");
	reply = (body) => {
		const parts = body.state.long_result;
		const hit = Object.keys(parts).find((k) => /PRICE/.test(parts[k]));
		return choice("part", hit ? { none: 0.03, [hit]: 0.94 } : { none: 0.9 });
	};
	const r = await fitResultFor("x1", out(page), undefined, 4000);
	const text = r.output[0].text as string;
	assert.match(text, /^\[Evaluator: a long result/);
	assert.match(text, /PRICE: Liquid Freezer III Pro 360/);
	assert.ok(text.length < page.length / 4);
	setSetting("judge", { enabled: false, base_url: base });
	const plain = await fitResultFor("x1", out(page), undefined, 4000);
	assert.match(plain.output[0].text, /characters cut out here/);
	setSetting("judge", { enabled: true, base_url: base });
});

test("a call that repeats a failed one is refused once; a new step, or the same again, runs", async () => {
	seeRequest("r1", [user("Find the price on DNS")]);
	const probe = { name: "bash", arguments: { command: "node probe.mjs" } };
	seeResult("r1", probe, { output: [{ type: "text", text: "died at frame 800" }], isError: true });
	reply = (body) => {
		assert.match(body.state.next_call, /probe\.mjs/);
		return choice("repeat.repeat", { new: 0.05, same_failed: 0.9, same_result: 0.05 });
	};
	assert.match((await atCall("r1", probe, false)).block!, /repeats one that just failed/);
	assert.equal((await atCall("r1", probe, false)).block, undefined, "the same again goes through");
	const before = seen.length;
	assert.equal((await atCall("r1", { name: "read", arguments: { path: "a" } }, false)).block, undefined, "another tool: not asked");
	assert.equal(seen.length, before);
});

test("repeat asks only about the same call, whole: other scripts, offsets and refused calls are not repeats", async () => {
	seeRequest("r2", [user("Restore the skills from the backup")]);
	const script = (body: string) => ({ name: "bash", arguments: { command: `cd /tmp && python3 - <<'EOF'\nimport json, glob, hashlib, os\n${body}\nEOF` } });
	seeResult("r2", script("for f in glob.glob('/tmp/atm/sessions/*.json'): print(f, os.path.getsize(f))"), out("a.json 80048"));
	reply = () => choice("repeat.repeat", { new: 0.01, same_failed: 0.01, same_result: 0.98 });
	const before = seen.length;
	// The same first 200 characters, another script: not asked at all.
	assert.equal((await atCall("r2", script("d=json.load(open('/tmp/atm/sessions/request_dump_1.json')); print(len(d['request']['body']['messages']))"), false)).block, undefined);
	// Another part of the same file: not asked.
	seeResult("r2", { name: "read", arguments: { path: "SKILL.md", offset: 1, limit: 60 } }, out("1:aa|---"));
	assert.equal((await atCall("r2", { name: "read", arguments: { path: "SKILL.md", offset: 60, limit: 80 } }, false)).block, undefined);
	assert.equal(seen.length, before, "nothing to ask");
	// The same call whole is asked, with both calls in full.
	reply = (body) => {
		assert.match(body.state.earlier_call, /getsize/);
		return choice("repeat.repeat", { new: 0.05, same_failed: 0.05, same_result: 0.9 });
	};
	assert.match((await atCall("r2", script("for f in glob.glob('/tmp/atm/sessions/*.json'): print(f, os.path.getsize(f))"), false)).block!, /already have/);
	// A refused call never ran: the next call is not read as repeating a failure, and no failure note comes.
	const probe = { id: "c9", name: "bash", arguments: { command: "sqlite3 /tmp/s.db 'select 1'" } };
	markNotRun("r2", "c9");
	seeResult("r2", probe, { output: [{ type: "text", text: "Tool call blocked: [Evaluator: …]" }], isError: true });
	assert.match(turnOf("r2").recent.at(-1)!, /NOT RUN/);
	assert.equal(turnOf("r2").calls.at(-1)!.failed, false);
	const asked = seen.length;
	assert.equal((await atCall("r2", probe, false)).block, undefined, "the refused call is no earlier result");
	assert.equal(seen.length, asked);
});

test("a near copy of a long call is stopped only when the evaluator is surer", async () => {
	seeRequest("r3", [user("Run the smoke test")]);
	const run = (n: number) => ({ name: "bash", arguments: { command: `cd /tmp && python3 /tmp/mcp_smoke.py --verbose --timeout 300 --server things --tags get_tags add_todo --retries ${n} 2>&1 | tail -20` } });
	seeResult("r3", run(1), out("ok"));
	reply = () => choice("repeat.repeat", { new: 0.1, same_failed: 0.0, same_result: 0.9 });
	assert.equal((await atCall("r3", run(2), false)).block, undefined, "0.9 is not enough for a near copy");
	reply = () => choice("repeat.repeat", { new: 0.02, same_failed: 0.0, same_result: 0.98 });
	assert.match((await atCall("r3", run(3), false)).block!, /already have/);
});

test("a state too long for the server's window keeps the latest results and its parts' names", () => {
	const found = Array.from({ length: 40 }, (_, i) => `result ${i} ` + "я".repeat(1990));
	const fitted: any = fitState({ final_answer: "x".repeat(4000), found }, 20_000);
	assert.ok(JSON.stringify(fitted).length <= 20_000);
	assert.match(fitted.found.at(-1), /^result 39 /);
	assert.equal(fitted.final_answer.length, 4000);
	const parts: any = fitState({ list: { i0: "a".repeat(30_000), i1: "b" } }, 20_000);
	assert.deepEqual(Object.keys(parts.list), ["i0", "i1"]);
});

test("an answer whose numbers none of the results gives is sent back once", async () => {
	seeRequest("c1", [user("Compare prices")]);
	for (let i = 0; i < 3; i++) {
		stepGate("c1");
		seeResult("c1", { name: "mcp__ozon__ozon_search", arguments: { q: "arctic" } }, out("Arctic LF III 360 — 9 753 ₽"));
	}
	reply = (body) => {
		assert.match(body.state.found[0], /9 753/);
		return choice("claims.claims", { supported: 0.1, unsupported: 0.9 });
	};
	const back = await atAnswer("c1", { content: [{ type: "text", text: "On Ozon it is 8 990 ₽." }] });
	assert.match(back!, /in none of the results/);
	assert.equal(await atAnswer("c1", { content: [{ type: "text", text: "8 990 ₽" }] }), undefined, "once a turn");
});

test("a JSON list a tool returned comes ordered by use, every item kept", async () => {
	seeRequest("l1", [user("Arctic Liquid Freezer III Pro 360 price")]);
	const items = Array.from({ length: 12 }, (_, i) => ({ title: i === 9 ? "Arctic Liquid Freezer III Pro 360" : `Cooler ${i}`, price: 1000 + i }));
	reply = (body) => {
		const hit = Object.keys(body.state.list).find((k) => /Arctic/.test(body.state.list[k]))!;
		return choice("listing.item", { [hit]: 0.8, i0: 0.01, i1: 0.01, none: 0.05 });
	};
	const r = await atResult("l1", { name: "mcp__ozon__ozon_search", arguments: {} }, out(JSON.stringify({ items, total: 12 })));
	const text = r.result.output[0].text as string;
	const json = JSON.parse(text.slice(text.indexOf("\n") + 1));
	assert.equal(json.items[0].title, "Arctic Liquid Freezer III Pro 360");
	assert.equal(json.items.length, 12);
	assert.equal(json.total, 12);
});

test("the MCP tools named at the start are offered from the next step, once", async () => {
	const { toolsToLoad } = await import("../src/judge/start.ts");
	const t = turnOf("m1");
	t.load = ["mcp__browser__browser_navigate"];
	assert.deepEqual(toolsToLoad(t), ["mcp__browser__browser_navigate"]);
	assert.deepEqual(toolsToLoad(t), []);
});

test("an overflowing request cuts what the next step needs least first", async () => {
	const { fitRequestFor } = await import("../src/builtin/window.ts");
	seeRequest("w1", [user("Find the DNS price")]);
	const big = (t: string) => ({ role: "toolResult", toolCallId: "1", content: [{ type: "text", text: t + " " + "x".repeat(200_000) }] });
	const msgs = [user("Find the DNS price"), big("OLD menu page"), big("DNS PRICE 10 499"), user("go on")];
	reply = (body) => {
		const k = Object.keys(body.state.earlier).find((b) => /PRICE/.test(body.state.earlier[b]))!;
		return choice("keep", { [k]: 0.9, none: 0.05 });
	};
	const out2 = (await fitRequestFor("w1", msgs, undefined, 131072))!;
	assert.match(out2[1].content[0].text, /cut here/, "the old page cut");
	assert.match(out2[2].content[0].text, /^DNS PRICE/);
	assert.ok(out2[2].content[0].text.length > 150_000, "the needed one kept whole");
});
