// A project kept in its folder (confined-env.ts) on Pi's local environment: its file tools are
// refused outside the folder, reading too; inside they work; a command cannot write outside
// (sandbox-exec on a Mac, bubblewrap on Linux when installed) but works in the folder.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";

// Not under the temp folders: those stay writable for commands.
const top = fs.realpathSync(fs.mkdtempSync(path.join(os.homedir(), ".bench-confined-test-")));
const root = path.join(top, "project");
fs.mkdirSync(root);
fs.writeFileSync(path.join(top, "secret.txt"), "outside");
after(() => fs.rmSync(top, { recursive: true, force: true }));
const { confined } = await import("../src/confined-env.ts");
const env = confined(new NodeExecutionEnv({ cwd: root }), root, true);

test("file tools: inside works, outside is refused — reading too", async () => {
	assert.ok((await env.writeFile("a.txt", "in", ctx)).ok);
	assert.equal((await env.readTextFile("a.txt", ctx) as any).value, "in");
	const out = await env.readTextFile("../secret.txt", ctx);
	assert.ok(!out.ok && out.error.code === "permission_denied");
	assert.ok(!(await env.writeFile(path.join(top, "x.txt"), "no", ctx)).ok);
	assert.ok(!(await env.renameFile("a.txt", "../a.txt", ctx)).ok);
	assert.ok(!(await env.listDir(top, ctx)).ok);
	assert.equal(env.cwd, root);
});

test("commands run in the folder; writing outside it fails where a sandbox is there", { skip: process.platform !== "darwin" && "the sandbox here is bubblewrap, if installed" }, async () => {
	let out = "";
	const r = await env.exec("pwd; echo hi > inside.txt; echo no > ../outside.txt; echo done", { onOutput: (t: string) => (out += t) }, ctx);
	assert.ok(r.ok);
	assert.match(out, new RegExp(root));
	assert.ok(fs.existsSync(path.join(root, "inside.txt")));
	assert.ok(!fs.existsSync(path.join(top, "outside.txt")), "nothing written outside");
	assert.ok(!(await env.exec("ls", { cwd: top }, ctx)).ok, "a working directory outside is refused");
});
