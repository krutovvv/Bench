// A file's text at HEAD (modules/files.ts), which the Explorer's editor marks changed lines
// against: the committed text, blank lines kept; nothing for a new file or one outside a repo.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-head-"));
process.env.BENCH_HOME = home;
const { textAtHead } = await import("../src/modules/files.ts");
after(() => fs.rmSync(home, { recursive: true, force: true }));

test("the committed text comes back; a new file or one outside a repository has none", async () => {
	const repo = path.join(home, "repo");
	fs.mkdirSync(path.join(repo, "src"), { recursive: true });
	const git = (...a: string[]) => execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", ...a], { stdio: "pipe" });
	git("init", "-q");
	fs.writeFileSync(path.join(repo, "src", "a.ts"), "one\n\nthree\n");
	git("add", ".");
	git("commit", "-qm", "first");
	fs.writeFileSync(path.join(repo, "src", "a.ts"), "one\ntwo\nthree\n");
	fs.writeFileSync(path.join(repo, "src", "new.ts"), "x\n");
	assert.equal(await textAtHead(null, path.join(repo, "src", "a.ts")), "one\n\nthree\n");
	assert.equal(await textAtHead(null, path.join(repo, "src", "new.ts")), null);
	fs.writeFileSync(path.join(home, "loose.ts"), "x\n");
	assert.equal(await textAtHead(null, path.join(home, "loose.ts")), null);
});
