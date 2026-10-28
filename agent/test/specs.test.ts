// A project's specs (src/specs.ts): found in the repository by their frontmatter, a file told the
// nearest module spec above it (or one that covers it), never the product's root spec, never a
// spec for itself.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-specs-"));
process.env.BENCH_HOME = home;
const { specsOf, specFor } = await import("../src/specs.ts");
after(() => fs.rmSync(home, { recursive: true, force: true }));

const spec = (id: string, title: string, extra = "") => `---\nid: ${id}\ntype: module-design\ntitle: ${title}\nparent: app\n${extra}---\n\n# ${title}\n`;

test("specs are found by frontmatter; a file is told its nearest module's", async () => {
	const repo = fs.realpathSync(fs.mkdtempSync(path.join(home, "repo-")));
	execFileSync("git", ["-C", repo, "init", "-q"]);
	const put = (rel: string, text: string) => {
		fs.mkdirSync(path.dirname(path.join(repo, rel)), { recursive: true });
		fs.writeFileSync(path.join(repo, rel), text);
	};
	put("SPEC.md", "---\nid: app\ntype: goal-and-requirements\ntitle: The app\n---\n");
	put("src/thread/SPEC.md", spec("thread", "A thread", "references: [ui]\n"));
	put("src/thread/rows/a.rs", "fn a() {}\n");
	put("docs/look.md", spec("look", "The look", "covers: [assets/themes]\n"));
	put("docs/notes.md", "# just notes\n");
	put("assets/themes/dark.json", "{}\n");
	put("src/app.rs", "\n");

	const { root, specs } = await specsOf(null, path.join(repo, "src"));
	assert.equal(root, repo);
	assert.deepEqual(specs.map((s) => s.id).sort(), ["app", "look", "thread"]);
	assert.deepEqual(specs.find((s) => s.id === "thread")!.references, ["ui"]);
	assert.equal(specFor(root, specs, path.join(repo, "src/thread/rows/a.rs"))?.id, "thread");
	assert.equal(specFor(root, specs, path.join(repo, "assets/themes/dark.json"))?.id, "look");
	assert.equal(specFor(root, specs, path.join(repo, "src/app.rs")), undefined, "the root spec is the product's, not a module's");
	assert.equal(specFor(root, specs, path.join(repo, "src/thread/SPEC.md")), undefined, "a spec is not under itself");
});
