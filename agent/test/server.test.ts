// The server itself starts (a clash in its imports or its first-start import stops it before any
// route answers) and its projects API works, on a home of its own.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

test("the server starts and answers /api/projects", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-server-"));
	const child = spawn(process.execPath, ["--no-warnings", "src/server.ts"], { env: { ...process.env, BENCH_HOME: home, BENCH_AGENT_NO_MCP: "1" }, stdio: ["ignore", "ignore", "pipe"] });
	let err = "";
	child.stderr.on("data", (d) => (err += d));
	try {
		const file = path.join(home, "server.json");
		for (let i = 0; i < 100 && !fs.existsSync(file); i++) await new Promise((r) => setTimeout(r, 200));
		assert.ok(fs.existsSync(file), `the server did not start:\n${err}`);
		const { port, token } = JSON.parse(fs.readFileSync(file, "utf8"));
		const call = (method: string, p: string, body?: unknown) =>
			fetch(`http://127.0.0.1:${port}${p}`, { method, headers: { "X-Hermes-Session-Token": token, "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
		const made = await call("POST", "/api/projects", { name: "Site" });
		assert.equal(made.project.name, "Site");
		const env = await call("GET", `/api/projects/${made.project.id}/env`);
		assert.deepEqual([env.soul, env.mcp, env.skills], ["", [], []]);
		// A skill of the project, read back by name.
		const skill = path.join(env.dir, "skills", "publish");
		fs.mkdirSync(skill, { recursive: true });
		fs.writeFileSync(path.join(skill, "SKILL.md"), "---\nname: publish\ndescription: d\n---\nSteps.");
		const read = await call("GET", `/api/projects/${made.project.id}/skills/publish`);
		assert.match(read.text, /Steps\./);
		assert.equal(read.scope, "project");
		// A skill is a folder: its tree, a file renamed in it.
		fs.mkdirSync(path.join(skill, "references"));
		fs.writeFileSync(path.join(skill, "references", "a.md"), "A");
		const tree = await call("GET", `/api/fs/tree?path=${encodeURIComponent(skill)}`);
		assert.deepEqual(tree.entries, [{ path: "references", dir: true }, { path: "references/a.md", dir: false }, { path: "SKILL.md", dir: false }]);
		await call("POST", "/api/fs/rename", { from: path.join(skill, "references", "a.md"), to: path.join(skill, "references", "b.md") });
		assert.ok(fs.existsSync(path.join(skill, "references", "b.md")));
		const clash = await call("POST", "/api/fs/rename", { from: path.join(skill, "references", "b.md"), to: path.join(skill, "SKILL.md") });
		assert.match(String(clash.detail ?? ""), /exists/);
		// A skill pasted whole: its front matter names it; twice is refused.
		const made2 = await call("POST", `/api/projects/${made.project.id}/skills`, { text: "---\nname: digest\ndescription: d\n---\nSum up." });
		assert.equal(made2.name, "digest");
		const again = await call("POST", `/api/projects/${made.project.id}/skills`, { text: "---\nname: digest\ndescription: d\n---\n" });
		assert.match(String(again.detail ?? ""), /already/);
		// The project's schedule: a job made, changed, paused, removed; a bad schedule refused.
		const job = await call("POST", `/api/projects/${made.project.id}/cron`, { name: "Digest", prompt: "Sum up the week", schedule: "every 1d" });
		assert.equal(job.schedule_display, "every 1d");
		assert.ok(job.next_run_at);
		const bad = await call("POST", `/api/projects/${made.project.id}/cron`, { name: "x", prompt: "y", schedule: "whenever" });
		assert.match(String(bad.detail ?? ""), /schedule/);
		const changed = await call("PUT", `/api/cron/jobs/${job.id}`, { updates: { prompt: "Sum up the month", schedule: "every 2d" } });
		assert.equal(changed.prompt, "Sum up the month");
		await call("POST", `/api/cron/jobs/${job.id}/pause`);
		let listed = (await call("GET", `/api/projects/${made.project.id}/env`)).cron;
		assert.deepEqual(listed.map((j: any) => [j.name, j.enabled]), [["Digest", false]]);
		await call("DELETE", `/api/cron/jobs/${job.id}`);
		listed = (await call("GET", `/api/projects/${made.project.id}/env`)).cron;
		assert.deepEqual(listed, []);
		// A project's reasoning level and approvals are its own: they go into its agent's folder (a
		// thread of the project runs with it), not into the machine's settings. The model and its
		// fallback are one for every thread: they go to the general agent, from wherever they come.
		await call("PUT", `/api/config?profile=${encodeURIComponent(`project:${made.project.id}`)}`, {
			config: { model: { provider: "p1", default: "m1" }, agent: { reasoning_effort: "low" }, approvals: { mode: "smart" } },
		});
		const own = JSON.parse(fs.readFileSync(path.join(env.dir, "settings.json"), "utf8"));
		assert.deepEqual([own.defaultProvider, own.defaultModel, own.defaultThinkingLevel, own.approvals], [undefined, undefined, "low", "smart"]);
		// A remote project shows the server agent's general environment.
		const general = await call("GET", "/api/projects/general/env");
		assert.equal(general.project.name, "Host");
		assert.ok(Array.isArray(general.skills) && Array.isArray(general.hosts));
		// The machine's own agent kept its approvals, and has the model for everyone.
		const hostOwn = JSON.parse(fs.readFileSync(path.join(home, "agents", "default", "settings.json"), "utf8"));
		assert.notEqual(hostOwn.approvals, "smart");
		assert.deepEqual([hostOwn.defaultProvider, hostOwn.defaultModel], ["p1", "m1"]);
		const raw = await call("GET", `/api/config/raw?profile=${encodeURIComponent(`project:${made.project.id}`)}`);
		assert.match(raw.yaml, /default: m1/);
	} finally {
		child.kill();
	}
});
