// grep, glob and ls on a real folder through Pi's local environment (on a project's server the
// same tools run through `SshExecutionEnv`): matches with file and line, an include glob that is
// never expanded by the shell, skipped folders, folders marked with /, the cap.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bench-search-"));
after(() => fs.rmSync(dir, { recursive: true, force: true }));
const put = (p: string, t: string) => {
	fs.mkdirSync(path.dirname(path.join(dir, p)), { recursive: true });
	fs.writeFileSync(path.join(dir, p), t);
};
put("src/app.rs", "fn main() {}\nfn render_row() {}\n");
put("src/ui/mod.rs", "pub fn render_head() {}\n");
put("README.md", "render_ notes\n");
put("node_modules/x/index.js", "render_inside_deps\n");
put("main.rs", "decoy for *.rs expansion\n");

const { grepTool, globTool, lsTool } = await import("../src/builtin/search.ts");
const api = { env: new NodeExecutionEnv({ cwd: dir }) };
const run = async (tool: any, args: any) => ((await tool.execute(args, api, ctx)) as any).output[0].text as string;

test("grep: file:line:text, only the included files, never node_modules", async () => {
	const out = await run(grepTool, { pattern: "fn render_\\w+", include: "*.rs" });
	assert.match(out, /src\/app\.rs:2:fn render_row/);
	assert.match(out, /src\/ui\/mod\.rs:1:pub fn render_head/);
	assert.doesNotMatch(out, /README|node_modules/);
	assert.equal(await run(grepTool, { pattern: "nothing-like-this" }), "No matches.");
	assert.match(await run(grepTool, { pattern: "render_", limit: 1 }), /more: narrow/);
});

test("glob: by name, by path with **, skipping node_modules", async () => {
	const names = (await run(globTool, { pattern: "*.rs" })).split("\n").sort();
	assert.deepEqual(names, ["main.rs", "src/app.rs", "src/ui/mod.rs"]);
	assert.match(await run(globTool, { pattern: "src/**/mod.rs" }), /src\/ui\/mod\.rs/);
	assert.doesNotMatch(await run(globTool, { pattern: "*.js" }), /node_modules/);
});

test("ls: the tree with folders marked, deps left out", async () => {
	const out = await run(lsTool, { depth: 2 });
	assert.match(out, /^src\/$/m);
	assert.match(out, /^src\/ui\/$/m);
	assert.match(out, /^README\.md$/m);
	assert.doesNotMatch(out, /node_modules/);
});
