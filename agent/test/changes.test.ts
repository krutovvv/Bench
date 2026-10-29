// What a turn changed (runtime/changes.ts): the working tree as a git tree at the run's start and
// end, the files that differ written into the journal — whatever changed them, an edit or a
// command — and the person's index left as it was.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-turns-"));
process.env.BENCH_HOME = home;
const { Changes, parseNumstat, CHANGES_KIND } = await import("../src/runtime/changes.ts");
after(() => fs.rmSync(home, { recursive: true, force: true }));

test("numstat reads as files under the root, a binary marked", () => {
	assert.deepEqual(parseNumstat("/r/", "2\t1\tsrc/a.ts\n-\t-\tlogo.png\n"), [
		{ path: "/r/src/a.ts", add: 2, del: 1 },
		{ path: "/r/logo.png", add: 0, del: 0, binary: true },
	]);
});

test("a turn's changes go into the journal; the index is untouched; nothing changed, nothing written", async () => {
	const repo = path.join(home, "repo");
	fs.mkdirSync(repo, { recursive: true });
	const git = (...a: string[]) => execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", ...a], { stdio: "pipe" }).toString();
	git("init", "-q");
	fs.writeFileSync(path.join(repo, "a.ts"), "one\ntwo\n");
	fs.writeFileSync(path.join(repo, "staged.ts"), "s\n");
	git("add", "a.ts");
	git("commit", "-qm", "first");
	git("add", "staged.ts");
	const indexBefore = git("diff", "--cached", "--name-only");

	const written: any[] = [];
	const live: any = { row: { id: "t1", cwd: repo, project: "" }, conversation: { submit: async (s: any) => void written.push(s) } };
	const changes = new Changes();

	changes.begin(live);
	await new Promise((r) => setTimeout(r, 300));
	fs.writeFileSync(path.join(repo, "a.ts"), "one\nTWO\nthree\n");
	fs.writeFileSync(path.join(repo, "made.ts"), "new\n");
	await changes.settle(live);

	assert.equal(written.length, 1);
	assert.equal(written[0].entry.kind, CHANGES_KIND);
	const { root, base, head, files } = written[0].entry.data;
	assert.equal(fs.realpathSync(root), fs.realpathSync(repo));
	assert.notEqual(base, head);
	assert.deepEqual(
		files.map((f: any) => [path.basename(f.path), f.add, f.del]).sort(),
		[
			["a.ts", 2, 1],
			["made.ts", 1, 0],
		],
	);
	assert.equal(git("diff", "--cached", "--name-only"), indexBefore, "the person's index is as it was");

	changes.begin(live);
	await changes.settle(live);
	assert.equal(written.length, 1, "a turn that changed nothing writes nothing");
});
