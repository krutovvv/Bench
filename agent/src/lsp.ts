// Language servers of a project, as Crush has them: the project's lsp.json names them, and after
// every edit or write of a file they know, its errors and warnings come back with the tool's
// result — the model sees them as an editor shows them, without asking. A `diagnostics` tool
// shows what is known now.
//
// A server runs where the project's files are: on this agent's machine, or — a project this agent
// works on a server — on that server through the same SSH connection (`ssh-env.ts`), so its paths
// are the files' own. Bench installs what a server needs there itself, once, into its own folder
// (`~/.bench/lsp`, `${LSP}` in lsp.json): npm packages and stub files (`lsp-presets.ts` has the
// usual ones). It starts on the first file it knows and stays while the agent runs. JSON-RPC is
// Microsoft's vscode-jsonrpc, as VS Code speaks it.

import { execFile, spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { createMessageConnection, StreamMessageReader, StreamMessageWriter, type MessageConnection } from "vscode-jsonrpc/node.js";
import { agentDir, BENCH_HOME } from "./config.ts";
import { machineEnv, quote, sshOptions } from "./ssh-env.ts";

/** One server of a project's lsp.json. `${LSP}` anywhere in it is Bench's folder for them there. */
export interface LspSpec {
	command: string;
	args?: string[];
	/** File extensions it knows, without the dot: ["ts", "tsx"]. */
	extensions: string[];
	/** npm packages installed into `${LSP}` before it starts (once per machine). */
	npm?: string[];
	/** Files fetched into `${LSP}` before it starts: stubs a server reads. */
	fetch?: { url: string; to: string }[];
	env?: Record<string, string>;
	/** `initializationOptions` of its initialize request. */
	initialization?: unknown;
	/** What it is told in workspace/didChangeConfiguration and answered in workspace/configuration. */
	settings?: Record<string, unknown>;
}

/** A project's servers (`<its agent dir>/lsp.json`, `{"servers": {name: spec}}`). */
export function readLsp(profile: string): Record<string, LspSpec> {
	try {
		const v = JSON.parse(fs.readFileSync(path.join(agentDir(profile), "lsp.json"), "utf8"));
		return v?.servers && typeof v.servers === "object" ? v.servers : {};
	} catch {
		return {};
	}
}

/** The project's servers anew; the running ones stop (the next file starts the new ones). */
export function writeLsp(profile: string, servers: Record<string, LspSpec>): void {
	const dir = agentDir(profile);
	fs.mkdirSync(dir, { recursive: true });
	fs.writeFileSync(path.join(dir, "lsp.json"), JSON.stringify({ servers }, null, 2));
	for (const [key, c] of clients) if (key.startsWith(`${profile}|`)) (c.stop(), clients.delete(key));
}

/** How the project's servers are doing: running, installing, or why they failed. */
export function lspStates(profile: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const [key, c] of clients) if (key.startsWith(`${profile}|`)) out[key.split("|")[3]!] = c.state();
	return out;
}

/**
 * What a project's servers need and its machine lacks, before any of them is started: npm
 * packages need node and npm there, any other server its own program, fetched files curl. Read
 * in a login shell, as `install` runs; kept a little while (the environment asks on every open).
 */
export async function lspNeeds(profile: string, machine: string | undefined): Promise<Record<string, string>> {
	const servers = Object.entries(readLsp(profile));
	if (!servers.length) return {};
	const needs = (s: LspSpec) => [...(s.npm?.length ? ["node", "npm"] : s.command.includes("${LSP}") ? [] : [s.command]), ...(s.fetch?.length ? ["curl"] : [])];
	const programs = [...new Set(servers.flatMap(([, s]) => needs(s)))];
	if (!programs.length) return {};
	const key = `${machine ?? ""}|${programs.join(" ")}`;
	const hit = lacking.get(key);
	const missing = hit && Date.now() - hit.at < 60_000 ? hit.missing : await lacks(programs, machine).catch(() => [] as string[]);
	lacking.set(key, { at: Date.now(), missing });
	const out: Record<string, string> = {};
	const where = machine ?? "this machine";
	for (const [name, s] of servers) {
		const no = needs(s).filter((p) => missing.includes(p));
		if (no.length) out[name] = `failed: no ${no.map((p) => (p === "node" ? "Node" : p)).join(", ")} on ${where} — install ${no.includes("node") ? "Node.js (with npm)" : no.join(", ")} there`;
	}
	return out;
}

const lacking = new Map<string, { at: number; missing: string[] }>();

async function lacks(programs: string[], machine: string | undefined): Promise<string[]> {
	const script = programs.map((p) => `command -v ${quote(p)} >/dev/null 2>&1 || echo ${quote(p)}`).join("; ");
	const out = machine
		? (await (await machineEnv(machine, "")).run(`bash -lc ${quote(script)} 2>/dev/null || sh -c ${quote(script)}`, BACKGROUND_CONTEXT)).stdout.toString()
		: (await run("bash", ["-lc", script], { timeout: 10_000 })).stdout;
	return out.split("\n").map((l) => l.trim()).filter(Boolean);
}

const LANGUAGE: Record<string, string> = {
	ts: "typescript", tsx: "typescriptreact", js: "javascript", jsx: "javascriptreact", mjs: "javascript", cjs: "javascript",
	php: "php", css: "css", scss: "scss", less: "less", html: "html", htm: "html", json: "json", jsonc: "jsonc",
	py: "python", rs: "rust", xml: "xml", vue: "vue", svelte: "svelte", md: "markdown", yaml: "yaml", yml: "yaml",
};

interface Diagnostic {
	range: { start: { line: number; character: number } };
	severity?: number;
	message: string;
	source?: string;
}

const run = promisify(execFile);

/** Bench's folder for language servers on the machine (`${LSP}`). */
async function lspDir(machine: string | undefined): Promise<string> {
	if (!machine) return path.join(BENCH_HOME, "lsp");
	const env = await machineEnv(machine, "");
	return path.posix.join(env.cwd, ".bench", "lsp");
}

/** `${LSP}` replaced everywhere in the spec. */
function resolved(spec: LspSpec, dir: string): LspSpec {
	return JSON.parse(JSON.stringify(spec).replaceAll("${LSP}", dir.replace(/\\/g, "/")));
}

/** What the spec needs, in `dir` on the machine, unless it is there: npm packages and fetched files. */
async function install(spec: LspSpec, dir: string, machine: string | undefined): Promise<void> {
	const pkgs = spec.npm ?? [];
	const files = spec.fetch ?? [];
	const missing = (p: string) => `[ -e ${quote(path.posix.join(dir, "node_modules", p.replace(/@[^@/]+$/, "")))} ]`;
	const steps = [
		`mkdir -p ${quote(dir)}`,
		...(pkgs.length ? [`if ! { ${pkgs.map(missing).join(" && ")}; }; then cd ${quote(dir)} && npm install --no-audit --no-fund --silent ${pkgs.map(quote).join(" ")} >/dev/null; fi`] : []),
		...files.map((f) => {
			const to = path.posix.join(dir, f.to);
			return `[ -s ${quote(to)} ] || { mkdir -p ${quote(path.posix.dirname(to))} && curl -fsSL ${quote(f.url)} -o ${quote(to)}; }`;
		}),
	];
	if (steps.length === 1 && !files.length) return;
	const script = steps.join(" && ");
	// A login shell: npm and curl are found as in a terminal there.
	if (machine) {
		const r = await (await machineEnv(machine, "")).run(`bash -lc ${quote(script)} 2>&1 || sh -c ${quote(script)}`, BACKGROUND_CONTEXT);
		if (r.code !== 0) throw new Error(`installing on ${machine}: ${r.stdout.toString().trim().split("\n").pop() ?? r.code}`);
	} else {
		await run("bash", ["-lc", script], { timeout: 600_000 });
	}
}

/** A running server for one project and folder. */
class LspClient {
	private proc: ChildProcess | undefined;
	private conn: MessageConnection | undefined;
	private ready: Promise<void>;
	private started = false;
	private versions = new Map<string, number>();
	private diags = new Map<string, Diagnostic[]>();
	private waiters = new Map<string, () => void>();
	failed: string | undefined;
	/** What the agent asked of it since it started: files checked, how many it answered, the last. */
	private asked = 0;
	private answered = 0;
	private last: { at: number; file: string; answered: boolean } | undefined;

	constructor(spec: LspSpec, root: string, machine: string | undefined) {
		this.ready = this.start(spec, root, machine).catch((e) => {
			this.failed = String(e?.message ?? e);
		});
	}

	/** How it is doing, as the project's environment shows it: is it up, and does it answer. */
	state(): string {
		if (this.failed) return `failed: ${this.failed}`;
		if (!this.started) return "starting (installing on first use)";
		if (!this.last) return "running, not asked yet";
		const d = new Date(this.last.at);
		const at = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
		const silent = this.asked - this.answered;
		return `running · ${this.asked} ${this.asked === 1 ? "file" : "files"} checked, ${silent ? `${silent} without an answer` : "all answered"} · last ${at}, ${path.basename(this.last.file)}${this.last.answered ? "" : " (no answer)"}`;
	}

	private async start(raw: LspSpec, root: string, machine: string | undefined): Promise<void> {
		const dir = await lspDir(machine);
		const spec = resolved(raw, dir);
		await install(spec, dir, machine);
		const proc = (this.proc = machine
			? spawn(process.env.BENCH_SSH ?? "ssh", [...sshOptions(machine), `cd ${quote(root)} && exec ${[spec.command, ...(spec.args ?? [])].map(quote).join(" ")}`], { stdio: ["pipe", "pipe", "pipe"] })
			: spawn(spec.command, spec.args ?? [], { cwd: root, env: { ...process.env, ...(spec.env ?? {}) }, stdio: ["pipe", "pipe", "pipe"] }));
		proc.on("error", (e) => (this.failed = e.message));
		proc.stderr?.on("data", () => {});
		const conn = (this.conn = createMessageConnection(new StreamMessageReader(proc.stdout!), new StreamMessageWriter(proc.stdin!)));
		conn.onNotification("textDocument/publishDiagnostics", (p: any) => {
			this.diags.set(p.uri, p.diagnostics ?? []);
			this.waiters.get(p.uri)?.();
		});
		const settings = spec.settings ?? {};
		conn.onRequest("workspace/configuration", (p: any) =>
			(p?.items ?? []).map((i: any) => (i.section ? (i.section.split(".").reduce((o: any, k: string) => o?.[k], settings) ?? null) : settings)),
		);
		conn.onRequest("client/registerCapability", () => null);
		conn.onRequest("window/workDoneProgress/create", () => null);
		conn.onRequest("workspace/workspaceFolders", () => [{ uri: uriOf(root), name: path.basename(root) }]);
		conn.onError(() => {});
		conn.listen();
		await conn.sendRequest("initialize", {
			processId: process.pid,
			rootUri: uriOf(root),
			workspaceFolders: [{ uri: uriOf(root), name: path.basename(root) }],
			initializationOptions: spec.initialization ?? {},
			capabilities: {
				workspace: { configuration: true, workspaceFolders: true },
				textDocument: { synchronization: { didSave: true }, publishDiagnostics: { relatedInformation: false } },
			},
		});
		await conn.sendNotification("initialized", {});
		if (spec.settings) await conn.sendNotification("workspace/didChangeConfiguration", { settings: spec.settings });
		this.started = true;
	}

	/** The file as it is now; its diagnostics once the server has looked (at most `ms`). */
	async check(file: string, text: string, ms: number): Promise<Diagnostic[] | undefined> {
		await Promise.race([this.ready, sleep(ms)]);
		if (this.failed || !this.started || !this.conn) return undefined;
		const uri = uriOf(file);
		const ext = path.extname(file).slice(1).toLowerCase();
		const seen = new Promise<void>((resolve) => this.waiters.set(uri, resolve));
		const version = (this.versions.get(uri) ?? 0) + 1;
		this.versions.set(uri, version);
		if (version === 1) await this.conn.sendNotification("textDocument/didOpen", { textDocument: { uri, languageId: LANGUAGE[ext] ?? ext, version, text } });
		else await this.conn.sendNotification("textDocument/didChange", { textDocument: { uri, version }, contentChanges: [{ text }] });
		await this.conn.sendNotification("textDocument/didSave", { textDocument: { uri }, text });
		const answered = await Promise.race([seen.then(() => true), sleep(ms).then(() => false)]);
		// Servers publish more than once (syntax, then types): a moment for the later one.
		await sleep(400);
		this.waiters.delete(uri);
		this.asked++;
		if (answered) this.answered++;
		this.last = { at: Date.now(), file, answered };
		return this.diags.get(uri);
	}

	known(): Map<string, Diagnostic[]> {
		return this.diags;
	}

	stop(): void {
		try {
			this.conn?.dispose();
		} catch {}
		this.proc?.kill();
	}
}

const clients = new Map<string, LspClient>();
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const uriOf = (p: string) => `file://${p.split("/").map(encodeURIComponent).join("/")}`;

/** The servers of `profile` that know `file`, started on first use. */
function clientsFor(profile: string, root: string, machine: string | undefined, file: string): [string, LspClient][] {
	const ext = path.extname(file).slice(1).toLowerCase();
	const out: [string, LspClient][] = [];
	for (const [name, spec] of Object.entries(readLsp(profile))) {
		if (!spec?.command || !(spec.extensions ?? []).map((e) => e.replace(/^\./, "").toLowerCase()).includes(ext)) continue;
		const key = `${profile}|${machine ?? ""}|${root}|${name}`;
		let c = clients.get(key);
		if (!c || c.failed) {
			c?.stop();
			c = new LspClient(spec, root, machine);
			clients.set(key, c);
		}
		out.push([name, c]);
	}
	return out;
}

/** A file's errors and warnings as lines for the model, at most 20; undefined: no server knows it. */
export async function diagnosticsFor(
	profile: string,
	root: string,
	machine: string | undefined,
	file: string,
	text: string,
	ms = 8000,
): Promise<string | undefined> {
	const servers = clientsFor(profile, root, machine, file);
	if (servers.length === 0) return undefined;
	const lines: string[] = [];
	let looked = false;
	for (const [name, c] of servers) {
		const d = await c.check(file, text, ms);
		if (d === undefined) {
			lines.push(`(${name}: ${c.failed ? `did not start: ${c.failed}` : "still starting"})`);
			continue;
		}
		looked = true;
		for (const x of d.filter((x) => (x.severity ?? 1) <= 2))
			lines.push(`${path.basename(file)}:${x.range.start.line + 1}:${x.range.start.character + 1} ${x.severity === 2 ? "warning" : "error"}: ${x.message.split("\n")[0]}${x.source ? ` (${x.source})` : ""}`);
	}
	const found = lines.filter((l) => !l.startsWith("("));
	if (looked && found.length === 0 && lines.length === 0) return `Diagnostics for ${path.basename(file)}: none.`;
	const shown = lines.slice(0, 20);
	return `Diagnostics for ${path.basename(file)}:\n${shown.join("\n")}${lines.length > shown.length ? `\n… ${lines.length - shown.length} more` : ""}`;
}

/** What the project's running servers know now, for the diagnostics tool. */
export function knownDiagnostics(profile: string): string {
	const lines: string[] = [];
	for (const [key, c] of clients) {
		if (!key.startsWith(`${profile}|`)) continue;
		for (const [uri, d] of c.known())
			for (const x of d.filter((x) => (x.severity ?? 1) <= 2))
				lines.push(`${decodeURIComponent(uri.replace(/^file:\/\//, ""))}:${x.range.start.line + 1}:${x.range.start.character + 1} ${x.severity === 2 ? "warning" : "error"}: ${x.message.split("\n")[0]}`);
	}
	return lines.length ? lines.slice(0, 100).join("\n") : "No errors or warnings in the files the language servers have seen.";
}
