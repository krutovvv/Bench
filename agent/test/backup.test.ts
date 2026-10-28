// A project's environment exported as one file and imported into another project: its soul,
// skills (with a binary file), prompts, MCP and language servers merged by name, settings without
// the machine's model, sign-ins, jobs, the Telegram bot moved; a stranger's file refused.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { startServer } from "./server.ts";

test("an environment goes from one project to another through its file", async () => {
	const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-backup-"));
	const s = await startServer(home, { websocket: false });
	try {
		const a = (await s.rest("POST", "/api/projects", { name: "Old" })).project;
		const b = (await s.rest("POST", "/api/projects", { name: "New" })).project;
		const envA = (await s.rest("GET", `/api/projects/${a.id}/env`)).dir;
		const envB = (await s.rest("GET", `/api/projects/${b.id}/env`)).dir;
		fs.writeFileSync(path.join(envA, "SYSTEM.md"), "You sell.");
		fs.mkdirSync(path.join(envA, "skills", "calls", "assets"), { recursive: true });
		fs.writeFileSync(path.join(envA, "skills", "calls", "SKILL.md"), "---\nname: calls\ndescription: d\n---\nCall.");
		fs.writeFileSync(path.join(envA, "skills", "calls", "assets", "logo.png"), Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 1, 255]));
		fs.writeFileSync(path.join(envA, "skills", ".DS_Store"), "x");
		fs.writeFileSync(path.join(envA, "prompts", "brief.md"), "Brief it.");
		fs.mkdirSync(path.join(envA, "files"), { recursive: true });
		fs.writeFileSync(path.join(envA, "files", "work.txt"), "not carried");
		fs.writeFileSync(path.join(envA, "mcp.json"), JSON.stringify({ mcpServers: { crm: { url: "https://crm.example/mcp" }, docs: { command: "docs" } } }));
		fs.writeFileSync(path.join(envA, "lsp.json"), JSON.stringify({ servers: { rust: { command: "rust-analyzer" } } }));
		fs.writeFileSync(path.join(envA, "settings.json"), JSON.stringify({ codeMode: true, defaultModel: "m" }));
		fs.mkdirSync(path.join(home, "mcp-oauth"), { recursive: true });
		fs.writeFileSync(path.join(home, "mcp-oauth", `project_${a.id}__crm.json`), JSON.stringify({ tokens: { access_token: "t" } }));
		await s.rest("PUT", `/api/projects/${a.id}/telegram`, { token: "123:abc", allowed_users: ["42"] });
		await s.rest("POST", `/api/projects/${a.id}/cron`, { name: "Digest", prompt: "Sum up", schedule: "every 1d" });
		// The new project has a server of the same name and one of its own.
		fs.writeFileSync(path.join(envB, "mcp.json"), JSON.stringify({ mcpServers: { docs: { command: "old-docs" }, mine: { command: "mine" } } }));

		const file = await s.rest("GET", `/api/projects/${a.id}/export`);
		assert.equal(file.kind, "bench.environment");
		assert.equal(file.name, "Old");
		assert.deepEqual(Object.keys(file.files).sort(), ["SYSTEM.md", "lsp.json", "mcp.json", "prompts/brief.md", "settings.json", "skills/calls/SKILL.md", "skills/calls/assets/logo.png"]);
		assert.ok("base64" in file.files["skills/calls/assets/logo.png"]);
		assert.equal(JSON.parse(file.files["settings.json"].text).defaultModel, undefined);
		assert.deepEqual(Object.keys(file.oauth), ["crm"]);
		assert.deepEqual(file.cron.map((j: any) => j.name), ["Digest"]);
		assert.equal(file.telegram.token, "123:abc");

		// Through JSON, as a file on disk would be.
		const done = await s.rest("POST", `/api/projects/${b.id}/import`, JSON.parse(JSON.stringify(file)));
		assert.deepEqual([done.skills, done.prompts, done.mcp, done.lsp, done.cron, done.telegram, done.bot_moved_from], [1, 1, 2, 1, 1, true, "Old"]);
		assert.equal(fs.readFileSync(path.join(envB, "SYSTEM.md"), "utf8"), "You sell.");
		assert.deepEqual([...fs.readFileSync(path.join(envB, "skills", "calls", "assets", "logo.png"))], [0x89, 0x50, 0x4e, 0x47, 0, 1, 255]);
		assert.ok(!fs.existsSync(path.join(envB, "files", "work.txt")));
		const mcp = JSON.parse(fs.readFileSync(path.join(envB, "mcp.json"), "utf8")).mcpServers;
		assert.deepEqual(Object.keys(mcp).sort(), ["crm", "docs", "mine"]);
		assert.equal(mcp.docs.command, "docs");
		assert.ok(fs.existsSync(path.join(home, "mcp-oauth", `project_${b.id}__crm.json`)));
		const envNow = await s.rest("GET", `/api/projects/${b.id}/env`);
		assert.equal(envNow.code_mode, true);
		assert.deepEqual(envNow.cron.map((j: any) => j.name), ["Digest"]);
		assert.equal(envNow.telegram.token, "123:abc");
		assert.equal((await s.rest("GET", `/api/projects/${a.id}/env`)).telegram, null);

		// Twice: the job of the same name is replaced, not doubled.
		await s.rest("POST", `/api/projects/${b.id}/import`, file);
		assert.equal((await s.rest("GET", `/api/projects/${b.id}/env`)).cron.length, 1);

		const stranger = await s.rest("POST", `/api/projects/${b.id}/import`, { hello: 1 });
		assert.match(String(stranger.detail), /not a Bench environment/);
		const escape = await s.rest("POST", `/api/projects/${b.id}/import`, { ...file, files: { "../../evil.md": { text: "x" } } });
		assert.match(String(escape.detail), /outside/);
	} finally {
		s.close();
	}
});
