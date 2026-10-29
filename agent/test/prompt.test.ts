// The project's instructions in the prompt, read here as on a server (`repo.ts`): AGENTS.md, else
// CLAUDE.md, from the repository's root down to the working folder.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

process.env.BENCH_HOME = fs.mkdtempSync(path.join(os.tmpdir(), "bench-prompt-"));
const { projectContext } = await import("../src/prompt.ts");

test("a folder's instructions come from its repository's root down, nearest last", async () => {
	const root = fs.mkdtempSync(path.join(os.tmpdir(), "bench-repo-"));
	execFileSync("git", ["init", "-q", root]);
	fs.writeFileSync(path.join(root, "AGENTS.md"), "Root rules.");
	const sub = path.join(root, "theme");
	fs.mkdirSync(sub);
	fs.writeFileSync(path.join(sub, "CLAUDE.md"), "Theme rules.");
	const text = (await projectContext({ agent: "default", cwd: sub, source: "local", inserts: [] } as any))!;
	assert.match(text, /^# Project instructions\n\n## \.\.\/AGENTS\.md\n\nRoot rules\.\n\n## CLAUDE\.md\n\nTheme rules\.$/);
	assert.equal(await projectContext({ agent: "default", cwd: os.homedir(), source: "local", inserts: [] } as any), undefined, "none from the home folder");
});
