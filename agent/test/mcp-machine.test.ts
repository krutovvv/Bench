// A project worked on a server runs its command MCP servers there, in its folder, over SSH — its
// own env going along — and one marked "local" here. `fake-ssh.sh` stands in for ssh (wrapped to
// leave a mark that it was used); a few lines of node stand in for an MCP server and name, in
// their one tool's description, the folder they run in and the env they were given.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "bench-mcp-machine-")));
process.env.BENCH_HOME = home;
after(() => fs.rmSync(home, { recursive: true, force: true }));
const mark = path.join(home, "ssh-used");
const ssh = path.join(home, "ssh.sh");
fs.writeFileSync(ssh, `#!/bin/sh\ntouch ${mark}\nexec ${path.join(import.meta.dirname, "fake-ssh.sh")} "$@"\n`, { mode: 0o755 });
process.env.BENCH_SSH = ssh;
const folder = path.join(home, "site");
fs.mkdirSync(folder);
const server = path.join(home, "server.mjs");
fs.writeFileSync(
	server,
	`import readline from "node:readline";
readline.createInterface({ input: process.stdin }).on("line", (l) => {
	const m = JSON.parse(l);
	const reply = (result) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: m.id, result }) + "\\n");
	if (m.method === "initialize") reply({ protocolVersion: m.params.protocolVersion, capabilities: { tools: {} }, serverInfo: { name: "where", version: "1" } });
	else if (m.method === "tools/list") reply({ tools: [{ name: "where", description: process.cwd() + "|" + (process.env.SITE ?? ""), inputSchema: { type: "object" } }] });
});
`,
);

const { projects } = await import("../src/builtin/projects.ts");
const { closeMcp, connectMcp, mcpStatus, mcpTools } = await import("../src/mcp/mcp.ts");
after(() => closeMcp());

async function run(local: boolean): Promise<string> {
	fs.rmSync(mark, { force: true });
	const p = projects.save({ name: "Shop", folder, machine: "web" });
	const spec = { command: process.execPath, args: [server], env: { SITE: "shop" }, ...(local ? { local: true } : {}) };
	fs.writeFileSync(path.join(projects.env(p.id), "mcp.json"), JSON.stringify({ mcpServers: { where: spec } }));
	await connectMcp(`project:${p.id}`, 10_000);
	const tools = mcpTools(`project:${p.id}`);
	assert.equal(tools.length, 1, `the server answered: ${JSON.stringify(mcpStatus())}`);
	return (tools[0] as any).description;
}

test("a server project's command MCP server runs on the server, in its folder, with its env", async () => {
	assert.equal(await run(false), `${folder}|shop`);
	assert.ok(fs.existsSync(mark), "started over ssh");
});

test('"local": true keeps it on this machine', async () => {
	assert.match(await run(true), /\|shop$/);
	assert.ok(!fs.existsSync(mark), "no ssh");
});
