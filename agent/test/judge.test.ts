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
const { markNotRun, seeNestedResult, seeRequest, seeResult, turnOf } = await import("../src/turn/turn.ts");
const { keptThrough } = await import("../src/judge/keep.ts");
const { resultsOf: results, personsWords, touchedFiles, afterSummary } = await import("../src/judge/summary.ts");
const { stepGate, stepNote } = await import("../src/turn/steps.ts");
const { checkJudge, fitState } = await import("../src/judge/client.ts");
const { baseOf } = await import("../src/judge/models.ts");
const { atResult, atAnswer } = await import("../src/judge/index.ts");
const { DRIFT_EVERY } = await import("../src/judge/drift.ts");
const { PLAN_EVERY } = await import("../src/judge/plan.ts");
const { SKILL_EVERY } = await import("../src/judge/skill.ts");
const { itemsOf, PLAN_KIND } = await import("../src/runtime/journal.ts");

const counted: string[] = [];
const stopped: string[] = [];
const written: any[] = [];
const compacted: [string, string][] = [];
setHost({
	thread: () => ({ id: "t", agent: "project:p", cwd: home, source: "local", project: "p", inserts: [] }),
	count: (_c: string, what: string) => void counted.push(what),
	stop: (id: string) => void stopped.push(id),
	write: (_c: string, entry: any) => void written.push(entry),
	compact: (c: string, why: string) => void compacted.push([c, why]),
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
		// A real server answers every question; one the test leaves out is answered unsure.
		const r = reply(body);
		const neutral = (q: any) => {
			const options = Object.keys(q.criteria ?? {});
			return q.type === "noul" ? { type: "noul", noul: 0.5 } : { type: "choice", choice: options[0], confidence: 0, probabilities: Object.fromEntries(options.map((o) => [o, 1 / options.length])) };
		};
		const answers = Object.fromEntries(Object.entries(body.questions ?? {}).map(([id, q]) => [id, r?.answers?.[id] ?? neutral(q)]));
		res.end(JSON.stringify(r?.answers ? { ...r, answers } : r));
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

test("no server in the gear, the evaluator is never asked and nothing changes", async () => {
	setSetting("judge", {});
	seeRequest("c3", [user("Make the tap jump")]);
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined);
	assert.equal(seen.length, 0);
});

test("a long search comes likeliest first, every line kept", async () => {
	setSetting("judge", { servers: [{ id: "e", base_url: `${base}/v1/`, model: "local", api_key: "sk-none" }] });
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
	setSetting("judge", { servers: [{ id: "e", base_url: "http://127.0.0.1:9" }] });
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined, "refused: the classic way");
	assert.equal(await rankSearch("c3", grep, out(lines)), undefined, "and quiet a while after");
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
});

test("an address is taken with or without /v1", () => {
	assert.equal(baseOf("https://api.typesafe.ai"), "https://api.typesafe.ai/v1");
	assert.equal(baseOf("http://h:10292/v1/"), "http://h:10292/v1");
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
	assert.match((await checkCourse("c4"))!, /Stop here: answer the person now/, "told once, still off: a stop");
	seeRequest("c4", [user("Make the game better")]);
	steps("c4", DRIFT_EVERY);
	reply = () => choice("course.course", { on_track: 0.4, drifting: 0.6, looping: 0, done: 0 });
	assert.equal(await checkCourse("c4"), undefined, "unsure, untold: quiet");
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
	setSetting("judge", { servers: [{ id: "e", base_url: base, api_key: "sk-saved" }] });
	reply = () => ({ model: "h2oai/h2o-lightning-4b", answers: { billing: { type: "noul", noul: 0.93 } } });
	const ok = await checkJudge({ id: "e", base_url: base, model: "" });
	assert.equal(ok.ok, true);
	assert.match(ok.message, /^Works · jev-latest · \d+ ms$/);
	assert.equal(seen.at(-1).auth, "Bearer sk-saved");
	reply = () => ({ result: "not System One" });
	assert.deepEqual(await checkJudge({ base_url: base }), { ok: false, message: "It answered, but not as a System One server" });
	assert.equal((await checkJudge({ base_url: "http://127.0.0.1:9" })).ok, false);
	assert.equal((await checkJudge({})).ok, false);
});

const todo = (todos: { content: string; status: string }[]) => ({ id: "call-plan", name: "todo", arguments: { todos: todos.map((t, i) => ({ id: `${i + 1}`, ...t })) } });

test("the evaluator marks the plan forward every few steps, in one request with the course; the thread gets an entry", async () => {
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
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
	// The course is checked at the same step (both every 5), and the command's output: one request, each point its own questions.
	assert.deepEqual(Object.keys(q).sort(), ["course.course", "plan.item0", "plan.item1", "plan.item2", "quiet.quiet"]);
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
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
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
	setSetting("judge", {});
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
const { fitResultFor } = await import("../src/turn/window.ts");
const { seeAnswer } = await import("../src/turn/turn.ts");

test("at a turn's start the fitting skill is named for the first request, once", async () => {
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
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
	const { hints } = await atRequest("s1");
	assert.equal(hints.length, 1);
	assert.match(hints[0], /the skill "price-check" fits it: load it with skill_view/);
	assert.deepEqual((await atRequest("s1")).hints, [], "once a turn");
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
	setSetting("judge", {});
	const plain = await fitResultFor("x1", out(page), undefined, 4000);
	assert.match(plain.output[0].text, /characters cut out here/);
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
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
	const { fitRequestFor } = await import("../src/turn/window.ts");
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

test("the thread's opening message, leaving open where the work belongs, is asked about first; a later one is not", async () => {
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
	const { projects } = await import("../src/builtin/projects.ts");
	projects.save({ id: "p", name: "Blog" });
	projects.save({ id: "q", name: "Assistant TG" });
	seeRequest("o1", [user("restore the skills from the Hermes backup")]);
	reply = (body) => {
		assert.equal(body.state.thread_project, "Blog");
		assert.ok(body.state.projects.includes("Assistant TG"));
		assert.deepEqual(Object.keys(body.questions["start.clear"].criteria), ["clear", "ask_first"], "starting first");
		return { answers: { ...choice("start.clear", { clear: 0.15, ask_first: 0.85 }).answers, ...choice("start.skill", { none: 0.9 }).answers } };
	};
	const { hints } = await atRequest("o1");
	assert.match(hints[0], /ask them with clarify/);
	// A later message leans on what was said before: not asked.
	seeRequest("o1", [user("restore the skills from the Hermes backup"), { role: "assistant", content: [] }, user("yes, go on")]);
	reply = (body) => {
		assert.equal(body.questions["start.clear"], undefined);
		return choice("start.skill", { none: 0.9 });
	};
	await atRequest("o1");
});

test("leaning off course at two checks running is a soft line with the request; once is not", async () => {
	seeRequest("d1", [user("Restore the Things skills into Assistant TG")]);
	reply = () => choice("course.course", { on_track: 0.3, drifting: 0.45, looping: 0.15, done: 0.1 });
	steps("d1", DRIFT_EVERY);
	assert.equal(await checkCourse("d1"), undefined, "once: quiet");
	steps("d1", DRIFT_EVERY);
	assert.match((await checkCourse("d1"))!, /may be moving away from what the person asked: «Restore the Things skills/);
	steps("d1", DRIFT_EVERY);
	assert.match((await checkCourse("d1"))!, /Stop here/, "said, and still leaning: a stop");
});

test("a script's nested call is no way around repeat: it is weighed against the calls before it", async () => {
	seeRequest("n1", [user("Set up the cron jobs")]);
	const job = { name: "cron", arguments: { action: "list" } };
	seeResult("n1", job, out('{"jobs":[]}'));
	reply = () => choice("repeat.repeat", { new: 0.05, same_failed: 0.0, same_result: 0.95 });
	assert.match((await atCall("n1", { ...job, parent: "x" } as any, false)).block!, /already have/);
	seeNestedResult("n1", { name: "cron", arguments: { action: "create", name: "weekly" } }, out("ok"));
	assert.equal(turnOf("n1").calls.at(-1)!.args.name, "weekly");
	assert.equal(turnOf("n1").steps, 0, "not a step of the turn");
});

test("at compaction, the results the work needs word for word follow the summary", async () => {
	seeRequest("k1", [user("Restore the skills")]);
	const messages = [
		{ role: "assistant", content: [{ type: "toolCall", id: "a", name: "bash", arguments: { command: "sha256sum SKILL.md" } }] },
		{ role: "toolResult", toolCallId: "a", content: [{ type: "text", text: "54ebcfe4a69f0c SKILL.md" }] },
		{ role: "assistant", content: [{ type: "toolCall", id: "b", name: "bash", arguments: { command: "ls" } }] },
		{ role: "toolResult", toolCallId: "b", content: [{ type: "text", text: "a b c" }] },
	];
	const blocks = results(messages);
	assert.match(blocks[0], /sha256sum.*\n54ebcfe4a69f0c/s);
	reply = (body) => {
		assert.match(body.questions.keep.instructions, /word for word/);
		return choice("keep", { b0: 0.8, b1: 0.1, none: 0.1 });
	};
	const kept = await keptThrough("k1", blocks);
	assert.match(kept, /## Kept word for word/);
	assert.match(kept, /54ebcfe4a69f0c/);
	assert.doesNotMatch(kept, /a b c/);
	setSetting("judge", {});
	assert.equal(await keptThrough("k1", blocks), "", "no evaluator: nothing added");
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
});

test("a step is not asked whether it is plain while the template moves its head; a thread without reasoning says so on every request", async () => {
	const { steer, steered } = await import("../src/runtime/steer.ts");
	const { PLAIN_STEPS } = await import("../src/judge/step.ts");
	seeRequest("p1", [user("Run the tests and fix what fails")]);
	stepGate("p1");
	seeResult("p1", { name: "write", arguments: { path: "t.mjs", content: "x" } }, out("ok"));
	reply = (body) => {
		assert.equal(!!body.questions["step.think"], PLAIN_STEPS);
		return { answers: {} };
	};
	await atRequest("p1");
	const local = { api: "openai-completions", baseUrl: "http://gpu:8000/v1" };
	let seenOptions: any;
	const api = steered({ streamSimple: (_m: any, _c: any, o: any) => (seenOptions = o) });
	// A thread at "off": Pi sends no reasoning; the template's switch goes with every request.
	api.streamSimple({}, {}, { sessionId: "s" });
	assert.equal((await seenOptions.onPayload({ messages: [] }, local)).chat_template_kwargs.enable_thinking, false);
	// A thread with reasoning is left as Pi sends it.
	api.streamSimple({}, {}, { sessionId: "s", reasoning: "low" });
	assert.equal(seenOptions.reasoning, "low");
	assert.equal(seenOptions.onPayload, undefined);
	// A step's word, once.
	steer("s", { plain: true });
	api.streamSimple({}, {}, { sessionId: "s", reasoning: "low" });
	assert.equal(seenOptions.reasoning, undefined);
	api.streamSimple({}, {}, { sessionId: "s", reasoning: "low" });
	assert.equal(seenOptions.reasoning, "low", "once");
});

test("a command's output that shows a failure is said, though the call did not fail", async () => {
	seeRequest("q1", [user("Build it")]);
	reply = (body) => {
		assert.match(body.state.output, /No such file/);
		return choice("quiet.quiet", { worked: 0.1, failed: 0.9 });
	};
	const r = await atResult("q1", { name: "bash", arguments: { command: "ls /nope; true" } }, out("ls: /nope: No such file or directory"));
	assert.ok(r.notes.some((n) => /shows a failure/.test(n)));
});

test("a running process's log is read for what it shows", async () => {
	seeRequest("w1", [user("Start the dev server")]);
	reply = (body) => {
		assert.match(body.state.process_log, /ready on :3000/);
		return choice("watch.watch", { working: 0.05, finished: 0.9, failed: 0.03, stuck: 0.02 });
	};
	const r = await atResult("w1", { name: "process", arguments: { action: "log", id: "7" } }, out('{"id":"7","state":"running"}\nready on :3000'));
	assert.ok(r.notes.some((n) => /work is done/.test(n)));
	const before = seen.length;
	await atResult("w1", { name: "process", arguments: { action: "log", id: "7" } }, out('{"id":"7","state":"exited"}\nbye'));
	assert.ok(!seen.slice(before).some((s) => s.body.questions["watch.watch"]), "a finished one is not asked about");
});

test("mid-turn, a skill that fits what the agent does now is named once", async () => {
	seeRequest("k2", [user("Fix the bug, then publish")]);
	reply = (body) => {
		const q = body.questions["skill.skill"];
		if (!q) return { answers: {} };
		const s = Object.keys(q.criteria).find((k) => /price-check/.test(q.criteria[k]))!;
		return choice("skill.skill", { none: 0.05, [s]: 0.9 });
	};
	let notes: string[] = [];
	for (let i = 0; i < SKILL_EVERY; i++) {
		stepGate("k2");
		notes = (await atResult("k2", { name: "read", arguments: { path: `f${i}` } }, out("x"))).notes;
	}
	assert.ok(notes.some((n) => /the skill "price-check" fits what you are doing now/.test(n)));
	for (let i = 0; i < SKILL_EVERY; i++) {
		stepGate("k2");
		notes = (await atResult("k2", { name: "read", arguments: { path: `g${i}` } }, out("x"))).notes;
	}
	assert.ok(!notes.some((n) => /price-check/.test(n)), "once a turn");
});

test("well on its way to Pi's threshold, a part of the work that just ended is compacted now, once", async () => {
	const { thresholds } = await import("../src/judge/boundary.ts");
	const { fitRequest, tokensOf } = await import("../src/turn/window.ts");
	seeRequest("b1", [user("Do the three parts")]);
	const { at } = thresholds("project:p");
	const fill = (share: number) => {
		const chars = Math.floor(share * at * (4 * 1000)) / tokensOf(4000);
		fitRequest("b1", [{ role: "user", content: [{ type: "text", text: "x".repeat(Math.floor(chars)) }] }], 10_000_000);
	};
	reply = (body) => (body.questions["boundary.boundary"] ? choice("boundary.boundary", { mid_work: 0.1, ended: 0.9 }) : { answers: {} });
	fill(0.3);
	stepGate("b1");
	await atResult("b1", { name: "read", arguments: { path: "a" } }, out("x"));
	assert.equal(compacted.length, 0, "far from the threshold: not asked");
	fill(0.65);
	stepGate("b1");
	await atResult("b1", { name: "read", arguments: { path: "b" } }, out("x"));
	assert.deepEqual(compacted.map(([c]) => c), ["b1"]);
	for (let i = 0; i < 4; i++) {
		stepGate("b1");
		await atResult("b1", { name: "read", arguments: { path: `c${i}` } }, out("x"));
	}
	assert.equal(compacted.length, 1, "once, until a compaction brings it down");
});

test("after a summary: the person's words in force, the files in work as they are now", async () => {
	seeRequest("s9", [user("Restore the skills into Assistant TG")]);
	const file = path.join(home, "SKILL.md");
	fs.writeFileSync(file, "# Things skill\nnow edited");
	const messages = [
		user("restore the skills"),
		user("not in this project, in Assistant TG; leave the memory alone"),
		user("<evaluator>\ncheck it\n</evaluator>"),
		{ role: "assistant", content: [{ type: "toolCall", id: "r", name: "read", arguments: { path: file } }] },
		{ role: "toolResult", toolCallId: "r", content: [{ type: "text", text: "1:aa|# Things skill" }] },
		{ role: "assistant", content: [{ type: "toolCall", id: "o", name: "read", arguments: { path: "/elsewhere/old.md" } }] },
	];
	assert.deepEqual(personsWords(messages), ["restore the skills", "not in this project, in Assistant TG; leave the memory alone"]);
	assert.deepEqual(touchedFiles(messages), ["/elsewhere/old.md", file]);
	reply = (body) => {
		if (body.questions.keep) return choice("keep", { none: 0.9 });
		const a = (k: string, p: Record<string, number>) => choice(k, p).answers[k];
		return { answers: { w0: a("w0", { past: 0.9, in_force: 0.1 }), w1: a("w1", { past: 0.1, in_force: 0.9 }), f0: a("f0", { done_with: 0.9, in_work: 0.1 }), f1: a("f1", { done_with: 0.1, in_work: 0.9 }) } };
	};
	const after = await afterSummary("s9", messages);
	assert.match(after, /## The person's words still in force\n> not in this project, in Assistant TG/);
	assert.doesNotMatch(after, /> restore the skills/);
	assert.match(after, /## Files in work, as they are now[\s\S]*now edited/);
	assert.doesNotMatch(after, /old\.md/);
});

test("a queued message that corrects the work under way is judged a correction; a next task is not", async () => {
	const { correctsTheWork } = await import("../src/judge/correction.ts");
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
	seeRequest("m1", [user("Restore the skills")]);
	reply = (body) => {
		assert.match(body.state.new_message, /Assistant TG/);
		return choice("correction", { next: 0.1, corrects: 0.9 });
	};
	assert.equal(await correctsTheWork("m1", "not here — in Assistant TG"), true);
	reply = () => choice("correction", { next: 0.8, corrects: 0.2 });
	assert.equal(await correctsTheWork("m1", "and later, a weekly report"), false);
});

test("a section is held until the next compaction", async () => {
	const { frozen, thaw } = await import("../src/turn/frozen.ts");
	let n = 0;
	assert.equal(await frozen("z1", "notes", () => `v${++n}`), "v1");
	assert.equal(await frozen("z1", "notes", () => `v${++n}`), "v1", "held");
	thaw("z1");
	assert.equal(await frozen("z1", "notes", () => `v${++n}`), "v2", "read anew after a compaction");
});

test("what is kept of a conversation goes when its thread closes", async () => {
	const { frozen } = await import("../src/turn/frozen.ts");
	const { forget } = await import("../src/turn/kept.ts");
	const { seeUsage, fitRequest, usedTokens } = await import("../src/turn/window.ts");
	seeRequest("gone", [user("the work")]);
	stepGate("gone");
	await frozen("gone", "notes", () => "v1");
	fitRequest("gone", [user("x".repeat(3000))], 100_000);
	seeUsage("gone", { input: 5000 });
	assert.equal(turnOf("gone").steps, 1);
	assert.ok(usedTokens("gone") > 0);
	forget("gone");
	assert.equal(turnOf("gone").steps, 0, "a turn with nothing known");
	assert.equal(turnOf("gone").request, "");
	assert.equal(usedTokens("gone"), 0, "no count of the window");
	assert.equal(await frozen("gone", "notes", () => "v2"), "v2", "the sections read anew");
});

test("each message is read for what it asks; an approval is answered in words, and its course is checked sooner", async () => {
	setSetting("judge", { servers: [{ id: "e", base_url: base }] });
	const said = { role: "assistant", content: [{ type: "text", text: "Done: the tools are final. Three limits remain: the deadline does not clear, the trash stays." }] };
	seeRequest("i1", [user("set up Things"), said, user("the dash is fine, the deadline doesn't matter, the trash I'll empty myself")]);
	reply = (body) => {
		assert.match(body.state.agent_answer_before_it, /tools are final/);
		const q = body.questions["intent.intent"];
		assert.deepEqual(Object.keys(q.criteria)[0], "work", "working as asked first");
		return { answers: { ...choice("intent.intent", { work: 0.1, question: 0.05, approval: 0.8, correction: 0.03, choice: 0.02 }).answers } };
	};
	const { hints } = await atRequest("i1");
	assert.match(hints[0], /approves or comments on work already done/);
	assert.equal(turnOf("i1").intent, "approval");
	// Asked once a message; every later question of the evaluator sees what it asks.
	reply = (body) => {
		assert.equal(body.questions["intent.intent"], undefined);
		assert.match(body.state.what_the_message_asks, /change nothing, start nothing/);
		return choice("course.course", { on_track: 0.1, drifting: 0.85, looping: 0.03, done: 0.02 });
	};
	steps("i1", 3);
	assert.match((await checkCourse("i1"))!, /did not ask for/, "words wanted: checked at 3 steps");
});

test("a turn starts at the person's message though a section's update follows it; a compaction's summary is no new turn", () => {
	const said = { role: "assistant", content: [{ type: "text", text: "Done." }] };
	seeRequest("t9", [user("first"), said, user("write the skill"), { role: "system", content: "", sections: { notes: "x" } }]);
	assert.equal(turnOf("t9").request, "write the skill");
	assert.equal(turnOf("t9").previous, "Done.");
	turnOf("t9").steps = 7;
	seeRequest("t9", [user("The conversation history before this point was compacted into the following summary:\n\n<summary>…</summary>")]);
	assert.equal(turnOf("t9").request, "write the skill", "the turn goes on");
	assert.equal(turnOf("t9").steps, 7);
});
