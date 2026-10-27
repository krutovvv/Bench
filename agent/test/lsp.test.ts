// A project's language servers (lsp.ts) with a fake server: a file's errors come back after it
// is checked, a fixed file says none, a file no server knows says nothing, and the same through
// SSH (the fake-ssh stand-in) for a project on a server.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-lsp-"));
process.env.BENCH_HOME = home;
process.env.BENCH_SSH = path.join(import.meta.dirname, "fake-ssh.sh");
const { writeLsp, readLsp, diagnosticsFor, knownDiagnostics, lspNeeds, lspStates, parseLspSpec, writeLspServer, problemsOf, hoverAt, definitionAt } = await import("../src/lsp.ts");
after(() => {
	writeLsp("project:p1", {});
	fs.rmSync(home, { recursive: true, force: true });
});

const root = fs.mkdtempSync(path.join(home, "code-"));
const fake = { command: process.execPath, args: [path.join(import.meta.dirname, "fake-lsp.mjs")], extensions: ["ts", ".tsx"] };

test("errors of a file come back; fixed, it says none; an unknown kind says nothing", async () => {
	writeLsp("project:p1", { fake });
	assert.deepEqual(Object.keys(readLsp("project:p1")), ["fake"]);
	const file = path.join(root, "a.ts");
	const bad = await diagnosticsFor("project:p1", root, undefined, file, "let x = 1;\nlet y = ERROR;\n// WARN here\n");
	assert.match(bad!, /a\.ts:2:9 error: found ERROR \(fake\)/);
	assert.match(bad!, /a\.ts:3:1 warning: a warning/);
	assert.equal(await diagnosticsFor("project:p1", root, undefined, file, "let x = 1;\n"), "Diagnostics for a.ts: none.");
	assert.equal(await diagnosticsFor("project:p1", root, undefined, path.join(root, "b.md"), "ERROR"), undefined);
	assert.match(knownDiagnostics("project:p1"), /none|No errors/i);
	// The environment shows that it is up and answers, and what it was last asked.
	assert.match(lspStates("project:p1").fake!, /^running · 2 files checked, all answered · last \d\d:\d\d, a\.ts$/);
});

test("the editor's questions: problems with their places, the hover, the definition", async () => {
	writeLsp("project:p1", { fake });
	const file = path.join(root, "e.ts");
	const text = "let x = 1;\nlet y = ERROR;\n";
	const problems = await problemsOf("project:p1", root, undefined, file, text);
	assert.deepEqual(problems, [{ range: { start: { line: 1, character: 8 }, end: { line: 1, character: 14 } }, severity: 1, message: "found ERROR", source: "fake" }]);
	assert.equal(await hoverAt("project:p1", root, undefined, file, text, { line: 1, character: 4 }), "at 1:4");
	assert.deepEqual(await definitionAt("project:p1", root, undefined, file, text, { line: 1, character: 4 }), [{ path: file, range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } } }]);
	assert.equal(await problemsOf("project:p1", root, undefined, path.join(root, "b.md"), "ERROR"), undefined);
	assert.equal(await hoverAt("project:p1", root, undefined, path.join(root, "b.md"), "x", { line: 0, character: 0 }), "");
});

test("a project on a server: its server runs there, through the SSH connection", async () => {
	const out = await diagnosticsFor("project:p1", root, "fake", path.join(root, "c.tsx"), "ERROR\n");
	assert.match(out!, /c\.tsx:1:1 error: found ERROR/);
});

test("what the machine lacks for a server is said before it starts", async () => {
	writeLsp("project:p2", {
		php: { command: "${LSP}/node_modules/.bin/intelephense", extensions: ["php"], npm: ["intelephense@1"] },
		ra: { command: "no-such-program-bench", extensions: ["rs"] },
		ours: fake,
	});
	const needs = await lspNeeds("project:p2", "fake");
	assert.deepEqual(Object.keys(needs), ["ra"], "node and npm are here; the fake's own program is");
	assert.match(needs.ra!, /^failed: no no-such-program-bench on fake/);
	writeLsp("project:p2", {});
});

test("one server's entry: checked before it is written, written beside the others", () => {
	assert.throws(() => parseLspSpec("{nope"), /not JSON/);
	assert.throws(() => parseLspSpec("[]"), /object/);
	assert.throws(() => parseLspSpec({ extensions: ["ts"] }), /command/);
	assert.throws(() => parseLspSpec({ command: "x", extensions: [] }), /extensions/);
	assert.throws(() => parseLspSpec({ command: "x", extensions: ["ts"], args: "--stdio" }), /args/);
	assert.equal(parseLspSpec('{"command": "x", "extensions": ["ts"]}').command, "x");
	writeLsp("project:p3", { a: fake });
	writeLspServer("project:p3", "b", { command: "y", extensions: ["php"] });
	assert.deepEqual(Object.keys(readLsp("project:p3")), ["a", "b"]);
	writeLsp("project:p3", {});
});
