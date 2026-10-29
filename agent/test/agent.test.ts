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
writeAgent("default", { defaultProvider: "dev", defaultModel: "m" });
const session = { id: "s1", agent: "default", cwd: home, source: "local" };
setHost({ thread: () => session } as any);
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
	// The refusal says the room exactly: a note cut to it fits at once.
	const room = 1375 - "Prefers short answers in Russian".length - "\n§\n".length;
	await assert.rejects(() => run(memoryTool, { action: "add", target: "user", content: "y".repeat(room + 1) }), new RegExp(`${room} characters are free`));
	await run(memoryTool, { action: "add", target: "user", content: "y".repeat(room) });
	await run(memoryTool, { action: "remove", target: "user", old_text: "yyy" });
});

test("memory: a project's notes and what it knows of the person are its own; Host's are Host's", async () => {
	await run(memoryTool, { action: "add", target: "memory", content: "General fact" });
	await run(memoryTool, { action: "add", target: "user", content: "Likes tea" });
	session.agent = "project:shop";
	try {
		assert.deepEqual(readEntries("project:shop", "memory"), [], "the general notes are not the project's");
		await run(memoryTool, { action: "add", target: "memory", content: "Shop fact" });
		assert.deepEqual(readEntries("project:shop", "memory"), ["Shop fact"]);
		assert.deepEqual(readEntries("default", "memory"), ["General fact"]);
		assert.match(notes(session)!, /Shop fact/);
		assert.doesNotMatch(notes(session)!, /General fact|Likes tea/);
		await run(memoryTool, { action: "add", target: "user", content: "Buys for the shop" });
		assert.deepEqual(readEntries("project:shop", "user"), ["Buys for the shop"]);
		assert.ok(readEntries("default", "user").includes("Likes tea") && !readEntries("default", "user").includes("Buys for the shop"));
	} finally {
		session.agent = "default";
	}
	await run(memoryTool, { action: "remove", target: "memory", old_text: "General fact" });
	await run(memoryTool, { action: "remove", target: "user", old_text: "Likes tea" });
});

test("skills: create, list in the prompt, view, patch, delete", async () => {
	const body = "---\nname: deploy\ndescription: Ship the app to the server\n---\n\nRun ./ship.sh\n";
	await run(skillManage, { action: "create", name: "deploy", category: "devops", content: body });
	assert.equal(listSkills("default")[0].name, "deploy");
	assert.match((await skills(session))!, /- deploy: Ship the app to the server/);
	await run(skillManage, { action: "patch", name: "deploy", old_string: "./ship.sh", new_string: "scripts/ship.sh" });
	const r = await skillView.execute({ name: "deploy" }, { conversationId: 1 } as any, undefined as any);
	assert.match((r as any).output[0].text, /scripts\/ship\.sh/);
	await run(skillManage, { action: "delete", name: "deploy" });
	assert.equal(fs.existsSync(path.join(agentDir("default"), "skills", "devops", "deploy")), false);
});

test("skills: a project's own, its folder's .agents/skills and .claude/skills up to the repository; the Host's are Host's alone", async () => {
	const { projects } = await import("../src/builtin/projects.ts");
	const repo = path.join(home, "repo");
	const sub = path.join(repo, "app");
	const write = (dir: string, name: string, desc: string) => {
		fs.mkdirSync(path.join(dir, name, "references"), { recursive: true });
		fs.writeFileSync(path.join(dir, name, "SKILL.md"), `---\nname: ${name}\ndescription: ${desc}\n---\nSteps.`);
		fs.writeFileSync(path.join(dir, name, "references", "more.md"), "More.");
	};
	fs.mkdirSync(path.join(repo, ".git"), { recursive: true });
	fs.mkdirSync(sub, { recursive: true });
	write(path.join(repo, ".agents", "skills"), "kit", "the repo's kit");
	write(path.join(repo, ".agents", "skills"), "shared", "the repo's version");
	write(path.join(repo, ".claude", "skills"), "claude-one", "a Claude Code skill");
	write(path.join(agentDir("default"), "skills"), "shared", "the general version");
	write(path.join(agentDir("default"), "skills"), "general-only", "a general one");
	const p = projects.save({ name: "Repo", folder: sub });
	write(path.join(agentDir(`project:${p.id}`), "skills"), "own", "the project's own");
	const list = listSkills(`project:${p.id}`);
	assert.deepEqual(list.map((k: any) => [k.name, k.scope]), [["claude-one", "repo"], ["kit", "repo"], ["own", "project"], ["shared", "repo"]]);
	assert.deepEqual(listSkills("default").map((k: any) => [k.name, k.scope]), [["general-only", "general"], ["shared", "general"]]);
	// A thread working in the folder has them in its prompt and loads them, files and all.
	const thread = { id: "s2", agent: `project:${p.id}`, cwd: sub, source: "local" };
	setHost({ thread: () => thread } as any);
	try {
		assert.match((await skills(thread))!, /- kit: the repo's kit/);
		const r = await skillView.execute({ name: "kit" }, { conversationId: 1 } as any, undefined as any);
		assert.match((r as any).output[0].text, /Steps\.[\s\S]*references\/more\.md/);
	} finally {
		setHost({ thread: () => session } as any);
	}
});

test("a title asks a thinking-by-default server not to think; OpenRouter, OpenAI and Anthropic are left to Pi", async () => {
	const { noThinking } = await import("../src/runtime/thinking.ts");
	const local = { api: "openai-completions", baseUrl: "http://192.168.1.10:8000/v1" };
	assert.deepEqual(noThinking({ model: "q", max_tokens: 400 }, local), { model: "q", max_tokens: 400, chat_template_kwargs: { enable_thinking: false } });
	assert.equal(noThinking({}, { api: "openai-completions", baseUrl: "https://openrouter.ai/api/v1" }), undefined);
	assert.equal(noThinking({}, { api: "openai-completions", baseUrl: "https://api.openai.com/v1" }), undefined);
	assert.equal(noThinking({}, { api: "anthropic-messages", baseUrl: "https://api.anthropic.com" }), undefined);
});

test("compaction: the summary is made without reasoning, kept across a restart, left to Pi on a failure", async () => {
	const { compactWithoutReasoning, transcript } = await import("../src/turn/compaction.ts");
	const before = (compactWithoutReasoning as any).handlers.beforeCompact;
	const model = { api: "openai-completions", baseUrl: "http://10.0.0.2:8000/v1", maxTokens: 32000 };
	let asked: any;
	const answer = { stopReason: "stop", content: [{ type: "text", text: "## Goal\nShip it" }] };
	const memo = new Map<string, string>();
	const api = (reply: any) => ({
		conversationId: 1,
		snapshot: async () => ({ model: { provider: "p", modelId: "m" } }),
		memo: async (k: string, v?: any) => (typeof v === "string" ? (memo.has(k) || memo.set(k, v), memo.get(k)) : memo.get(k)),
		models: { getModel: () => model, complete: async (_m: any, c: any, o: any) => ((asked = { c, payload: o.onPayload({ model: "m" }, model) }), reply) },
	});
	const messages = [{ role: "user", content: "Deploy" }, { role: "assistant", content: [{ type: "text", text: "On it" }, { type: "toolCall", name: "bash", arguments: { command: "ls" } }] }];
	assert.equal(transcript(messages), '[User]: Deploy\n\n[Assistant]: On it\n\n[Assistant tool calls]: bash(command="ls")');
	assert.deepEqual(await before({ reason: "manual", messages, entries: [], firstKept: 1 }, api(answer), {}), { summary: "## Goal\nShip it" });
	assert.deepEqual(asked.payload.chat_template_kwargs, { enable_thinking: false });
	assert.match(asked.c.messages[0].content, /<conversation>\n\[User\]: Deploy/);
	// After a restart the memo answers; a failed call leaves the summary to Pi.
	asked = undefined;
	assert.deepEqual(await before({ reason: "manual", messages, entries: [], firstKept: 1 }, api(answer), {}), { summary: "## Goal\nShip it" });
	assert.equal(asked, undefined);
	memo.clear();
	assert.equal(await before({ reason: "manual", messages, entries: [], firstKept: 1 }, api({ stopReason: "error", content: [] }), {}), undefined);
});

test("telegram: markdown to HTML, splitting, MEDIA files", () => {
	assert.equal(toHtml("**a** `b<c` _d_ [e](https://x.y)"), '<b>a</b> <code>b&lt;c</code> <i>d</i> <a href="https://x.y">e</a>');
	assert.equal(toHtml("| A | B |\n|---|---|\n| 1 | 2 |"), "• <b>A:</b> 1\n  <b>B:</b> 2");
	assert.equal(split("x".repeat(9000)).length, 3);
	const f = path.join(home, "a.png");
	fs.writeFileSync(f, "png");
	assert.deepEqual(extractMedia(`see\nMEDIA:${f}\n`).files, [f]);
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
const { totalsOf, usagePayload } = await import("../src/runtime/usage.ts");

test("usage: a thread's counters are kept and come back on a fresh store", () => {
	const st = new Store();
	const row = st.create({ id: "u1", conversation: "1", agent: "default", source: "local", cwd: home, effort: "" });
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

test("a project's new folder takes the threads that worked in its old one, not one set apart", async () => {
	const { projects } = await import("../src/builtin/projects.ts");
	const st = new Store();
	const p = projects.save({ name: "Moves" });
	const old = projects.dir(p.id);
	const apart = fs.mkdtempSync(path.join(home, "apart-"));
	st.create({ id: "f1", conversation: "11", agent: `project:${p.id}`, source: "local", cwd: old, effort: "" });
	st.create({ id: "f2", conversation: "12", agent: `project:${p.id}`, source: "local", cwd: apart, effort: "" });
	const folder = fs.mkdtempSync(path.join(home, "folder-"));
	projects.setFolder(p.id, folder);
	assert.deepEqual([st.get("f1")!.cwd, st.get("f2")!.cwd], [folder, apart]);
	assert.throws(() => projects.setFolder(p.id, "/no/such/folder"), /no folder/);
	projects.setFolder(p.id, null);
	assert.equal(st.get("f1")!.cwd, old);
});

test("a waiting message keeps its attachments and block state when its words are fixed", async () => {
	const { withText } = await import("../src/runtime/queue.ts");
	// Plain words.
	assert.equal(withText("old", "new"), "new");
	// The block state Bench sent with it stays for the model.
	assert.equal(withText("old\n\n<block-state>\nthe terminal shows x\n</block-state>", "new"), "new\n\n<block-state>\nthe terminal shows x\n</block-state>");
	// Attachments stay, in their order, behind the new words.
	const withImage = [
		{ type: "text", text: "old" },
		{ type: "image", data: "AAA", mimeType: "image/png" },
	];
	assert.deepEqual(withText(withImage, "new"), [
		{ type: "text", text: "new" },
		{ type: "image", data: "AAA", mimeType: "image/png" },
	]);
	// Reading what we wrote gives the new words back, without the block state.
	const { stripBlock } = await import("../src/runtime/journal.ts");
	assert.equal(stripBlock(withText("old\n\n<block-state>\nx\n</block-state>", "new")), "new");
});

test("search finds the words of a thread's messages, a fork's through its entry", async () => {
	const { Store } = await import("../src/store.ts");
	const st = new Store();
	st.create({ id: "q1", conversation: "3", agent: "default", source: "local", cwd: home, effort: "" });
	st.create({ id: "q2", conversation: "4", agent: "default", source: "local", cwd: home, effort: "" });
	st.index("q1", 5, "user", "send the error log", 1);
	st.index("q1", 6, "assistant", "the log is attached", 2);
	st.index("q1", 9, "user", "and the access log", 3);
	assert.equal(st.get("q1")?.message_count, 3);
	assert.deepEqual(st.search("error log").map((h) => h.thread_id), ["q1"]);
	st.copyIndex("q1", "q2", 6);
	assert.equal(st.get("q2")?.message_count, 2, "a fork has its words through the entry it was made at");
	st.cutIndex("q1", 6);
	assert.deepEqual(st.search("access").map((h) => h.thread_id), [], "an exchange taken back is not found");
	st.delete("q1");
	st.delete("q2");
});

test("server agents: a list of their own; the old project rows with a host move into it once", async () => {
	const { projects } = await import("../src/builtin/projects.ts");
	const { servers, migrateServerRows } = await import("../src/builtin/servers.ts");
	const keep = projects.save({ name: "Plain" });
	projects.save({ name: "Dev", host: "dev" } as any);
	projects.save({ name: "RPi", host: "rpi" } as any);
	assert.deepEqual(migrateServerRows(), ["Dev (dev)", "RPi (rpi)"]);
	assert.deepEqual(servers.list(), [{ host: "dev", name: "Dev" }, { host: "rpi", name: "RPi" }]);
	assert.ok(projects.list().every((p) => !p.host), "no project stands for a server any more");
	assert.ok(projects.get(keep.id), "other projects stay");
	assert.deepEqual(migrateServerRows(), [], "once");
	servers.save({ host: "dev", name: "Dev box" });
	assert.deepEqual(servers.list().map((s) => s.name), ["Dev box", "RPi"], "renamed in place");
	servers.remove("rpi");
	assert.deepEqual(servers.list().map((s) => s.host), ["dev"]);
	servers.remove("dev");
});

test("presets: built-in until edited, one kind at a time; checked; back to built-in", async () => {
	const { presets, savePresets, DEFAULT_PRESETS } = await import("../src/builtin/presets.ts");
	assert.deepEqual(presets(), DEFAULT_PRESETS);
	savePresets("reasoning", ["low", "high"]);
	assert.deepEqual(presets().reasoning, ["low", "high"]);
	assert.deepEqual(presets().mcp, DEFAULT_PRESETS.mcp, "the other kinds stay built-in");
	assert.throws(() => savePresets("compaction", [{ at: 2, name: "x" }]));
	assert.throws(() => savePresets("mcp", { a: { title: "no spec" } }));
	savePresets("reasoning", null);
	assert.deepEqual(presets().reasoning, DEFAULT_PRESETS.reasoning);
});
