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
	const text = soul({ id: "s", agent: `project:${p.id}`, source: "local" } as any);
	assert.match(text, /server `wp-prod`, in \/var\/www\/shop/);
});

test("/api/fs with a machine: read, write, list, rename and remove there", async () => {
	const files = (await import("../src/modules/files.ts")).default();
	const r = files.routes!;
	const machine = real ?? "fake";
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bench-fs-machine-"));
	const q = (o: Record<string, string>) => ({ query: new URLSearchParams(o), body: {} }) as any;
	const b = (o: any) => ({ query: new URLSearchParams(), body: o }) as any;
	try {
		if (real) return; // a real server has no such folder; the stand-in shares this disk
		await r["POST /api/fs/write-text"]!(b({ machine, path: `${dir}/a/one.txt`, content: "hello" }));
		assert.strictEqual(((await r["GET /api/fs/read-text"]!(q({ machine, path: `${dir}/a/one.txt` }))) as any).text, "hello");
		const listed = (await r["GET /api/fs/list"]!(q({ machine, path: dir }))) as any;
		assert.deepStrictEqual(listed.entries.map((e: any) => [e.name, e.isDirectory]), [["a", true]]);
		await r["POST /api/fs/rename"]!(b({ machine, from: `${dir}/a/one.txt`, to: `${dir}/a/two.txt` }));
		assert.ok(fs.existsSync(`${dir}/a/two.txt`));
		await r["POST /api/fs/delete"]!(b({ machine, path: `${dir}/a` }));
		assert.ok(!fs.existsSync(`${dir}/a`));
		// An attachment: the bytes land on the machine, in a folder made for them.
		const bytes = Buffer.from([0, 1, 2, 255]);
		await r["POST /api/files/upload"]!(b({ machine, path: `${dir}/att/1-x.bin`, data_url: `data:application/octet-stream;base64,${bytes.toString("base64")}` }));
		assert.deepStrictEqual(fs.readFileSync(`${dir}/att/1-x.bin`), bytes);
		await assert.rejects(r["GET /api/fs/read-text"]!(q({ machine, path: `${dir}/missing` })) as Promise<unknown>, /No such|not/i);
	} finally {
		fs.rmSync(dir, { recursive: true, force: true });
	}
});

test("a server folder's own instructions and skills are read there", async () => {
	if (real) return; // the stand-in shares this disk
	const { repoOf, skillsFor, readSkill, skillFiles, forgetRepos } = await import("../src/repo.ts");
	const { projects } = await import("../src/builtin/projects.ts");
	const { projectContext, skills } = await import("../src/prompt.ts");
	const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "bench-repo-")));
	try {
		const theme = path.join(root, "wp-content", "themes", "shop");
		fs.mkdirSync(path.join(root, ".git"), { recursive: true });
		fs.mkdirSync(theme, { recursive: true });
		fs.writeFileSync(path.join(root, "AGENTS.md"), "Root rules.");
		fs.writeFileSync(path.join(theme, "CLAUDE.md"), "Theme rules.");
		const skill = path.join(theme, ".agents", "skills", "wp", "acf-fields");
		fs.mkdirSync(path.join(skill, "references"), { recursive: true });
		fs.writeFileSync(path.join(skill, "SKILL.md"), "---\nname: acf-fields\ndescription: How fields are added\n---\nBody.");
		fs.writeFileSync(path.join(skill, "references", "api.md"), "API");
		fs.mkdirSync(path.join(root, ".claude", "skills", "acf-fields"), { recursive: true });
		fs.writeFileSync(path.join(root, ".claude", "skills", "acf-fields", "SKILL.md"), "---\nname: acf-fields\ndescription: farther\n---\n");

		const repo = await repoOf(theme, "fake");
		assert.deepStrictEqual(repo.instructions.map((i) => [i.file, i.text]), [[path.join(root, "AGENTS.md"), "Root rules."], [path.join(theme, "CLAUDE.md"), "Theme rules."]]);
		const p = projects.save({ name: "Shop", folder: theme, machine: "fake" });
		const list = await skillsFor(`project:${p.id}`, theme, "fake");
		const acf = list.find((k) => k.name === "acf-fields")!;
		assert.strictEqual(acf.dir, skill, "the nearest one of a name");
		assert.strictEqual(acf.category, "wp");
		assert.strictEqual(acf.machine, "fake");
		assert.match(await readSkill(acf), /Body\./);
		assert.deepStrictEqual(await skillFiles(acf), ["references/api.md"]);
		await assert.rejects(readSkill(acf, "../../x"), /outside/);

		const info = { id: "t", agent: `project:${p.id}`, cwd: theme, source: "bench", project: p.id };
		assert.match((await projectContext(info))!, /Root rules\.[\s\S]*Theme rules\./);
		assert.match((await skills(info))!, /acf-fields: How fields are added/);
		forgetRepos();
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test("one server's connection carries at most six commands at once; the rest wait their turn", async () => {
	if (real) return;
	const env = new SshExecutionEnv("fake", "/");
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bench-gate-"));
	try {
		const runs = await Promise.all(
			Array.from({ length: 16 }, () => env.run(`touch ${dir}/$$; sleep 0.3; ls ${dir} | wc -l; rm ${dir}/$$`, BACKGROUND_CONTEXT)),
		);
		assert.ok(runs.every((r) => r.code === 0));
		const most = Math.max(...runs.map((r) => Number(r.stdout.toString().trim())));
		assert.ok(most <= 6 && most >= 2, `at most six at once, ran ${most}`);
	} finally {
		fs.rmSync(dir, { recursive: true, force: true });
	}
});

test("a command whose connection was lost says so", async () => {
	if (real) return;
	const was = process.env.BENCH_SSH;
	const lost = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "bench-lost-")), "ssh");
	fs.writeFileSync(lost, "#!/bin/sh\necho partial\necho 'client_loop: send disconnect: Broken pipe' >&2\nexit 255\n", { mode: 0o755 });
	process.env.BENCH_SSH = lost;
	try {
		let out = "";
		const r = getOrThrow(await new SshExecutionEnv("fake", "/").exec("composer install", { onOutput: (t: string) => (out += t) }, BACKGROUND_CONTEXT));
		assert.strictEqual(r.exitCode, 255);
		assert.match(out, /partial[\s\S]*connection to fake was lost[\s\S]*nohup/);
	} finally {
		process.env.BENCH_SSH = was;
		fs.rmSync(path.dirname(lost), { recursive: true, force: true });
	}
});
