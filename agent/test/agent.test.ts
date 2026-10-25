// The agent's own pieces, on a temporary BENCH_HOME: schedules, notes, skills, Telegram markup.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-test-"));
process.env.BENCH_HOME = home;
after(() => fs.rmSync(home, { recursive: true, force: true }));

const { setSetting, writeAgent, agentDir } = await import("../src/config.ts");
const { setHost } = await import("../src/host.ts");
const { nextRun, describe } = await import("../src/builtin/cron.ts");
const { memoryTool, readEntries } = await import("../src/builtin/memory.ts");
const { skillManage, skillView, listSkills } = await import("../src/builtin/skills.ts");
const { toHtml, split, extractMedia } = await import("../src/telegram.ts");
const { notes, skills } = await import("../src/prompt.ts");

setSetting("timezone", "Asia/Yekaterinburg");
writeAgent("default", { provider: "dev", model: "m" });
const session = { id: "s1", profile: "default", cwd: home, source: "local" };
setHost({ session: () => session } as any);
const run = async (tool: any, args: any) => {
	const r = await tool.execute(args, { conversationId: 1 }, undefined);
	return JSON.parse(r.output[0].text);
};

test("schedules: cron expressions in the set timezone, intervals, one-shots", () => {
	const at = Date.parse("2026-10-24T10:00:00Z"); // 15:00 in Yekaterinburg (+05)
	assert.equal(new Date(nextRun("0 8 * * 1", at)!).toISOString(), "2026-10-26T03:00:00.000Z");
	assert.equal(nextRun("30m", at, at), at + 30 * 60_000);
	assert.equal(nextRun("every 2h", at, null), at);
	assert.equal(nextRun("2026-11-01T09:00:00Z", at), Date.parse("2026-11-01T09:00:00Z"));
	assert.equal(nextRun("2026-11-01T09:00:00Z", at, at), null);
	assert.equal(describe("every 15 minutes"), "every 15m");
	assert.throws(() => nextRun("whenever", at));
});

test("memory: add, replace, remove, budget; shown in the prompt", async () => {
	await run(memoryTool, { action: "add", target: "user", content: "Prefers short answers" });
	await run(memoryTool, { action: "add", target: "memory", content: "Local model: Qwen" });
	await run(memoryTool, { action: "replace", target: "user", old_text: "short", content: "Prefers short answers in Russian" });
	assert.deepEqual(readEntries("default", "user"), ["Prefers short answers in Russian"]);
	assert.match(notes(session)!, /Prefers short answers in Russian/);
	await run(memoryTool, { action: "remove", target: "memory", old_text: "Qwen" });
	assert.deepEqual(readEntries("default", "memory"), []);
	await assert.rejects(() => run(memoryTool, { action: "add", target: "user", content: "x".repeat(5000) }), /budget/);
});

test("skills: create, list in the prompt, view, patch, delete", async () => {
	const body = "---\nname: deploy\ndescription: Ship the app to the server\n---\n\nRun ./ship.sh\n";
	await run(skillManage, { action: "create", name: "deploy", category: "devops", content: body });
	assert.equal(listSkills("default")[0].name, "deploy");
	assert.match(skills(session)!, /- deploy: Ship the app to the server/);
	await run(skillManage, { action: "patch", name: "deploy", old_string: "./ship.sh", new_string: "scripts/ship.sh" });
	const r = await skillView.execute({ name: "deploy" }, { conversationId: 1 } as any, undefined as any);
	assert.match((r as any).output[0].text, /scripts\/ship\.sh/);
	await run(skillManage, { action: "delete", name: "deploy" });
	assert.equal(fs.existsSync(path.join(agentDir("default"), "skills", "devops", "deploy")), false);
});

test("telegram: markdown to HTML, splitting, MEDIA files", () => {
	assert.equal(toHtml("**a** `b<c` _d_ [e](https://x.y)"), '<b>a</b> <code>b&lt;c</code> <i>d</i> <a href="https://x.y">e</a>');
	assert.equal(toHtml("| A | B |\n|---|---|\n| 1 | 2 |"), "• <b>A:</b> 1\n  <b>B:</b> 2");
	assert.equal(split("x".repeat(9000)).length, 3);
	const f = path.join(home, "a.png");
	fs.writeFileSync(f, "png");
	assert.deepEqual(extractMedia(`see\nMEDIA:${f}\n`).files, [f]);
});

test("tasks: add, list by tag, complete, import without doubles", async () => {
	const { tasks, tasksTool } = await import("../src/builtin/tasks.ts");
	const add = await run(tasksTool, { action: "add", title: "Pay rent", tags: ["Дом"], day: "2026-11-01", important: true });
	assert.equal(add.task.title, "Pay rent");
	assert.equal((await run(tasksTool, { action: "list", tag: "дом" })).count, 1);
	await run(tasksTool, { action: "complete", id: add.task.id });
	assert.equal((await run(tasksTool, { action: "list" })).count, 0);
	assert.equal((await run(tasksTool, { action: "list", done_since: "2000-01-01" })).count, 1);
	assert.equal(tasks.import([{ title: "From Reminders", source_id: "r1" }]), 1);
	assert.equal(tasks.import([{ title: "From Reminders", source_id: "r1" }]), 0);
});

test("telegram: a batch keeps the order the messages came in and waits for voice", async () => {
	const { Batch } = await import("../src/telegram.ts");
	const b = new Batch("me");
	const voice = b.reserve(1, false);
	const link = b.reserve(2, false);
	b.fill(link, "https://example.com/schedule");
	assert.equal(b.ready, false);
	b.fill(voice, "make a table from this schedule");
	assert.equal(b.ready, true);
	assert.equal(b.text(), "make a table from this schedule\n\nhttps://example.com/schedule");
	assert.equal(b.last, 2);
	const f = new Batch("me");
	f.fill(f.reserve(3, true), undefined);
	assert.equal(f.onlyForwarded, true);
	assert.equal(f.text(), "");
});

const { Store } = await import("../src/store.ts");
const { totalsOf, usagePayload } = await import("../src/usage.ts");

test("usage: a thread's counters are kept and come back on a fresh store", () => {
	const st = new Store();
	const row = st.create({ id: "u1", conversation: 1, profile: "default", source: "local", cwd: home, model: "m", provider: "dev", effort: "" });
	assert.deepEqual(row.usage, { input: 0, output: 0, calls: 0 });
	st.update("u1", { usage: { input: 1200, output: 340, calls: 2, cached: 900, tps: 31.4 } });
	// A new Store = the agent process restarted: the same numbers are still there.
	assert.deepEqual(new Store().get("u1")!.usage, { input: 1200, output: 340, calls: 2, cached: 900, tps: 31.4 });
	const frame = usagePayload("m", totalsOf(st.get("u1")!.usage), 8000);
	assert.equal(frame.context_used, 1200);
	assert.equal(frame.context_max, 8000);
	assert.equal(frame.cache_hit_pct, 75);
	assert.equal(frame.avg_tps, 31.4);
	assert.deepEqual(totalsOf("junk"), { input: 0, output: 0, calls: 0 });
	st.delete("u1");
});
