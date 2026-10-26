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
const { writeLsp, readLsp, diagnosticsFor, knownDiagnostics } = await import("../src/lsp.ts");
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
});

test("a project on a server: its server runs there, through the SSH connection", async () => {
	const out = await diagnosticsFor("project:p1", root, "fake", path.join(root, "c.tsx"), "ERROR\n");
	assert.match(out!, /c\.tsx:1:1 error: found ERROR/);
});
