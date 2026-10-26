// A project's machine over SSH meets Pi Durable's own conformance suite for execution
// environments — the same one `NodeExecutionEnv` passes. Without a network: `fake-ssh.sh` stands
// in for ssh and runs each script here (BSD tools on a Mac, GNU on Linux: both are covered).
// BENCH_TEST_SSH_HOST=<alias> runs it against a real server too.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { getOrThrow } from "@earendil-works/pi-durable/env";
import { createEnvConformance } from "@earendil-works/pi-durable/testing";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-ssh-env-test-"));
process.env.BENCH_HOME = home;
after(() => fs.rmSync(home, { recursive: true, force: true }));

const { SshExecutionEnv } = await import("../src/ssh-env.ts");
const { BACKGROUND_CONTEXT } = await import("@earendil-works/chord/context");

const real = process.env.BENCH_TEST_SSH_HOST;
if (!real) process.env.BENCH_SSH = path.join(import.meta.dirname, "fake-ssh.sh");

const assertions = {
	ok: (v: unknown, m?: string) => assert.ok(v, m),
	strictEqual: (a: unknown, b: unknown) => assert.strictEqual(a, b),
	deepEqual: (a: unknown, b: unknown) => assert.deepStrictEqual(a, b),
	partialDeepEqual: (a: any, b: any) => {
		for (const [k, v] of Object.entries(b)) assert.deepStrictEqual(a?.[k], v, k);
	},
	greaterThan: (a: number, b: number) => assert.ok(a > b, `${a} > ${b}`),
	rejects: async (p: Promise<unknown>, m: string) => {
		await assert.rejects(p, (e: Error) => e.message.includes(m));
	},
};

const cases = createEnvConformance({
	assertions,
	async withEnv(use) {
		const host = real ?? "fake";
		const probe = new SshExecutionEnv(host, "/");
		const dir = getOrThrow(await probe.createTempDir("bench-conformance-", BACKGROUND_CONTEXT));
		const env = new SshExecutionEnv(host, dir);
		try {
			await use(env);
		} finally {
			await env.remove(dir, { recursive: true, force: true }, BACKGROUND_CONTEXT);
			await env.cleanup(BACKGROUND_CONTEXT);
		}
	},
});

for (const c of cases) test(`ssh env: ${c.name}`, { timeout: c.timeoutMs ?? 20_000 }, () => c.run());

test("a project on another machine: its environment is there, in its folder or the login directory", async () => {
	const { machineEnv } = await import("../src/ssh-env.ts");
	const host = real ?? "fake";
	const home = await machineEnv(host, "");
	assert.ok(home.cwd.startsWith("/"), `login directory: ${home.cwd}`);
	assert.strictEqual(home.id, `ssh:${host}`);
	assert.strictEqual(await machineEnv(host, ""), home, "one environment per place");
	assert.strictEqual((await machineEnv(host, "/srv/site")).cwd, "/srv/site");
});

test("the prompt says where a machine project's work is", async () => {
	const { projects } = await import("../src/builtin/projects.ts");
	const { soul } = await import("../src/prompt.ts");
	const p = projects.save({ name: "Shop", folder: "/var/www/shop", machine: "wp-prod" });
	assert.strictEqual(projects.get(p.id)?.machine, "wp-prod");
	assert.strictEqual(projects.dir(p.id), "/var/www/shop", "its folder there, not looked for here");
	const text = soul({ id: "s", profile: `project:${p.id}`, source: "local" } as any);
	assert.match(text, /server `wp-prod`, in \/var\/www\/shop/);
});
