// The agent's `lsp` tool (builtin/navigate.ts) on the test language server (fake-lsp.mjs): a place
// named by file, line and word; definitions and references with their code; a file's outline;
// symbols by name across the project.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-navigate-"));
process.env.BENCH_HOME = home;

const { writeLsp } = await import("../src/lsp.ts");
const { lspTool, spotOf } = await import("../src/builtin/navigate.ts");
const { setHost } = await import("../src/host.ts");
const { NodeExecutionEnv } = await import("@earendil-works/pi-durable/env/node");
const { BACKGROUND_CONTEXT: ctx } = await import("@earendil-works/chord/context");

after(() => {
	writeLsp("project:nav", {});
	fs.rmSync(home, { recursive: true, force: true });
});

const root = fs.mkdtempSync(path.join(home, "code-"));
fs.writeFileSync(path.join(root, "a.ts"), "class Outer {\n  inner() { return Outer; }\n}\n");
execFileSync("git", ["init", "-q", root]);
execFileSync("git", ["-C", root, "add", "a.ts"]);
writeLsp("project:nav", { fake: { command: process.execPath, args: [path.join(import.meta.dirname, "fake-lsp.mjs")], extensions: ["ts"] } });
setHost({ thread: () => ({ id: "s", agent: "project:nav", cwd: root, source: "local", project: "", inserts: [] }) } as any);

const call = async (args: any) => {
	const r: any = await lspTool.execute(args, { conversationId: "c", env: new NodeExecutionEnv({ cwd: root }) } as any, ctx);
	return r.output[0].text as string;
};

test("a place is a line and the word on it, or the word's first use", () => {
	const text = "let a = b;\nfoo(a, ab);\n";
	assert.deepEqual(spotOf(text, 2, "a"), { line: 1, character: 4 }, "the word, not inside another");
	assert.deepEqual(spotOf(text, undefined, "ab"), { line: 1, character: 7 });
	assert.deepEqual(spotOf("// a is here\nlet a;\n", undefined, "a"), { line: 1, character: 4 }, "code, not a comment");
	assert.deepEqual(spotOf(text, 2, undefined), { line: 1, character: 0 });
	assert.equal(spotOf(text, 1, "foo"), undefined);
});

test("definition and references come with their code; outline nests; symbols search the project", async () => {
	assert.equal(await call({ action: "definition", path: "a.ts", symbol: "Outer", line: 2 }), "a.ts:1: class Outer {");
	assert.equal(await call({ action: "references", path: "a.ts", symbol: "Outer" }), "a.ts:1: class Outer {\na.ts:2: inner() { return Outer; }");
	assert.equal(await call({ action: "hover", path: "a.ts", symbol: "inner" }), "at 1:2");
	assert.equal(await call({ action: "outline", path: "a.ts" }), "1  class Outer\n  2  method inner");
	assert.equal(await call({ action: "symbols", query: "Out" }), "a.ts:2  function OutThing");
	await assert.rejects(call({ action: "definition", path: "a.ts", symbol: "Nope" }), /"Nope" is not in a\.ts/);
});

test("a project without language servers says so and points to grep", async () => {
	writeLsp("project:nav", {});
	await assert.rejects(call({ action: "outline", path: "a.ts" }), /no language servers/);
});
