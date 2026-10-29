// Approvals' patterns (`builtin/approvals.ts`): with a project's approvals on, a shell command that
// matches one asks the person first; off, or not a shell command, nothing is looked at.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

process.env.BENCH_HOME = fs.mkdtempSync(path.join(os.tmpdir(), "bench-approvals-"));
const { writeAgent, readAgent } = await import("../src/config.ts");
const { approvalOf } = await import("../src/builtin/approvals.ts");

test("a dangerous command under approvals on is named; off, or another tool, it is not", () => {
	const rm = { name: "bash", arguments: { command: "rm -rf build" } };
	assert.deepEqual(approvalOf("default", rm), { shell: false, command: "rm -rf build", hit: undefined }, "approvals off");
	writeAgent("default", { ...readAgent("default"), approvals: "manual" });
	assert.equal(approvalOf("default", rm).hit, "recursive delete");
	assert.equal(approvalOf("default", { name: "ssh_run", arguments: { command: "sudo reboot" } }).hit, "runs as root");
	assert.deepEqual(approvalOf("default", { name: "bash", arguments: { command: "ls" } }), { shell: true, command: "ls", hit: undefined }, "let through: the evaluator judges it");
	assert.equal(approvalOf("default", { name: "read", arguments: { path: "x" } }).shell, false);
	assert.equal(approvalOf(undefined, rm).shell, false, "no thread");
});
