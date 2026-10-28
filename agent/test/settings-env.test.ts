// A project's settings (machine/settings.ts) stay in reach of its file tools when its work is
// elsewhere — here kept in its folder: the settings are read and written, anything else outside
// the folder is still refused, and the folder itself works as before.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";

const top = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "bench-settings-test-")));
const root = path.join(top, "project");
const dir = path.join(top, "settings");
fs.mkdirSync(root);
fs.mkdirSync(dir);
fs.writeFileSync(path.join(dir, "SYSTEM.md"), "soul");
fs.writeFileSync(path.join(top, "secret.txt"), "outside");
after(() => fs.rmSync(top, { recursive: true, force: true }));
const { confined } = await import("../src/machine/confined.ts");
const { withSettings } = await import("../src/machine/settings.ts");
const env = withSettings(confined(new NodeExecutionEnv({ cwd: root }), root, true), new NodeExecutionEnv({ cwd: dir }), dir);

test("the settings are read and written; the rest outside stays refused", async () => {
	assert.equal((await env.readTextFile(path.join(dir, "SYSTEM.md"), ctx) as any).value, "soul");
	assert.ok((await env.writeFile(path.join(dir, "SYSTEM.md"), "soul, changed", ctx)).ok);
	assert.equal(fs.readFileSync(path.join(dir, "SYSTEM.md"), "utf8"), "soul, changed");
	const out = await env.readTextFile("../secret.txt", ctx);
	assert.ok(!out.ok && out.error.code === "permission_denied");
	assert.ok((await env.writeFile("a.txt", "in", ctx)).ok);
	assert.equal(env.cwd, root);
});
