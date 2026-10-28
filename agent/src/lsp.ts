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
//
// The Explorer's editor asks the same servers (`modules/lsp.ts`): a file's problems as it is
// typed, the hover at a place, where a symbol is defined. So does the agent's `lsp` tool
// (`builtin/navigate.ts`): definitions, references, a file's outline, symbols by name. One server
// per project and folder serves the agent and the editor alike.

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

/**
 * One server's entry as the person wrote it (JSON text, or the object itself), checked enough
 * that it can start: a command and the extensions it knows. Throws with what is wrong.
 */
export function parseLspSpec(input: unknown): LspSpec {
	let spec: any = input;
	if (typeof input === "string") {
		try {
			spec = JSON.parse(input);
		} catch (e) {
			throw new Error(`not JSON: ${(e as Error).message}`);
		}
	}
	if (!spec || typeof spec !== "object" || Array.isArray(spec)) throw new Error("a server is an object: {\"command\", \"args\", \"extensions\"}");
	if (typeof spec.command !== "string" || !spec.command.trim()) throw new Error("it needs a command");
	if (!Array.isArray(spec.extensions) || !spec.extensions.length || !spec.extensions.every((x: unknown) => typeof x === "string"))
		throw new Error("it needs extensions: the kinds of files it knows, as [\"ts\", \"tsx\"]");
	if (spec.args !== undefined && (!Array.isArray(spec.args) || !spec.args.every((x: unknown) => typeof x === "string"))) throw new Error("args is a list of strings");
	return spec as LspSpec;
}

/** One server of the project set anew (or added); the project's running servers stop, as `writeLsp`. */
export function writeLspServer(profile: string, name: string, spec: LspSpec): void {
	writeLsp(profile, { ...readLsp(profile), [name]: spec });
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
	range: { start: Spot; end: Spot };
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
	/** The text each open file has on the server, so a hover after a check sends nothing again. */
	private texts = new Map<string, string>();
	private diags = new Map<string, Diagnostic[]>();
	private waiters = new Map<string, () => void>();
	failed: string | undefined;
	/** When it was started: a young server may still be indexing the project. */
	readonly born = Date.now();
	/** Whether it is done loading the project, when it says so (rust-analyzer's
	 * `experimental/serverStatus`); undefined for a server that never says. */
	private quiescent: boolean | undefined;
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
		// A server that dies on its own (a crash under load) must read as failed, or the stale
		// client is kept forever and every ask throws “Connection is closed.”. Its last stderr
		// lines say why, and go into the state the project's environment shows.
		let errTail = "";
		proc.stderr?.on("data", (b: Buffer) => (errTail = (errTail + b.toString()).slice(-600)));
		const died = (what: string) => {
			if (!this.failed) this.failed = `${what}${errTail.trim() ? `: ${errTail.trim().split("\n").pop()}` : ""}`;
		};
		proc.on("exit", (code, sig) => died(`exited (${sig ?? code})`));
		const conn = (this.conn = createMessageConnection(new StreamMessageReader(proc.stdout!), new StreamMessageWriter(proc.stdin!)));
		conn.onNotification("experimental/serverStatus", (p: any) => (this.quiescent = Boolean(p?.quiescent)));
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
		conn.onError((e: any) => died(String(e?.message ?? e)));
		conn.onClose(() => died("the connection closed"));
		conn.listen();
		await conn.sendRequest("initialize", {
			processId: process.pid,
			rootUri: uriOf(root),
			workspaceFolders: [{ uri: uriOf(root), name: path.basename(root) }],
			initializationOptions: spec.initialization ?? {},
			capabilities: {
				experimental: { serverStatusNotification: true },
				workspace: { configuration: true, workspaceFolders: true, symbol: {} },
				textDocument: {
					synchronization: { didSave: true },
					publishDiagnostics: { relatedInformation: false },
					hover: { contentFormat: ["markdown", "plaintext"] },
					definition: { linkSupport: true },
					references: {},
					documentSymbol: { hierarchicalDocumentSymbolSupport: true },
				},
			},
		});
		await conn.sendNotification("initialized", {});
		if (spec.settings) await conn.sendNotification("workspace/didChangeConfiguration", { settings: spec.settings });
		this.started = true;
	}

	/** The server has the file as `text` (opened, or changed when it differs). False: not up. */
	private async sync(file: string, text: string): Promise<boolean> {
		if (this.failed || !this.started || !this.conn) return false;
		const uri = uriOf(file);
		if (this.texts.get(uri) === text) return true;
		const ext = path.extname(file).slice(1).toLowerCase();
		const version = (this.versions.get(uri) ?? 0) + 1;
		try {
			if (version === 1) await this.conn.sendNotification("textDocument/didOpen", { textDocument: { uri, languageId: LANGUAGE[ext] ?? ext, version, text } });
			else await this.conn.sendNotification("textDocument/didChange", { textDocument: { uri, version }, contentChanges: [{ text }] });
		} catch (e) {
			this.failed ??= String((e as Error)?.message ?? e);
			return false;
		}
		this.texts.set(uri, text);
		return true;
	}

	/** A request about `file` as `text`, at most `ms`; undefined: not up, or no answer. */
	async ask(method: string, file: string, text: string, params: object, ms: number): Promise<any> {
		await Promise.race([this.ready, sleep(ms)]);
		if (!(await this.sync(file, text))) return undefined;
		const answer = this.conn!.sendRequest(method, { textDocument: { uri: uriOf(file) }, ...params }).catch(() => undefined);
		return Promise.race([answer, sleep(ms).then(() => undefined)]);
	}

	/** Whether it is still loading the project: by its own word, or, for a server that never says,
	 * in its first 15 s. */
	indexing(): boolean {
		if (this.failed) return false;
		return this.quiescent === undefined ? Date.now() - this.born < 15_000 : !this.quiescent;
	}

	/** A request about no one file (`workspace/symbol`), at most `ms`; undefined: not up, or no answer.
	 * `open` are files the server is shown first: some (tsserver) know a project only from open files. */
	async request(method: string, params: object, ms: number, open: { file: string; text: string }[] = []): Promise<any> {
		await Promise.race([this.ready, sleep(ms)]);
		if (this.failed || !this.started || !this.conn) return undefined;
		for (const f of open) if (!(await this.sync(f.file, f.text))) return undefined;
		const answer = this.conn!.sendRequest(method, params).catch(() => undefined);
		return Promise.race([answer, sleep(ms).then(() => undefined)]);
	}

	/** The file as it is now; its diagnostics once the server has looked (at most `ms`). */
	async check(file: string, text: string, ms: number): Promise<Diagnostic[] | undefined> {
		await Promise.race([this.ready, sleep(ms)]);
		if (this.failed || !this.started || !this.conn) return undefined;
		const uri = uriOf(file);
		const seen = new Promise<void>((resolve) => this.waiters.set(uri, resolve));
		// Checked again unchanged (a save): the server is told it is saved, and looks again.
		this.texts.delete(uri);
		try {
			await this.sync(file, text);
			await this.conn.sendNotification("textDocument/didSave", { textDocument: { uri }, text });
		} catch (e) {
			// The connection died between the checks: read as failed, the next ask starts anew.
			this.failed ??= String((e as Error)?.message ?? e);
			this.waiters.delete(uri);
			return undefined;
		}
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

/** The servers of `profile` still loading the project, with how long they have been at it (s): an
 * empty answer from one of them is not yet the truth. */
export function indexing(profile: string): { name: string; secs: number }[] {
	const out: { name: string; secs: number }[] = [];
	for (const [key, c] of clients) if (key.startsWith(`${profile}|`) && c.indexing()) out.push({ name: key.split("|").pop()!, secs: Math.round((Date.now() - c.born) / 1000) });
	return out;
}

/** Start every server of the project now, so it has loaded the project by the time it is asked
 * (rust-analyzer takes minutes on a large workspace). Started ones are left as they are. */
export function warmUp(profile: string, root: string, machine: string | undefined): void {
	for (const [name, spec] of Object.entries(readLsp(profile))) {
		const ext = (spec.extensions ?? [])[0]?.replace(/^\./, "");
		if (spec?.command && ext) clientsFor(profile, root, machine, `${root}/.${name}.${ext}`);
	}
}
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

/** A place in a file, 0-based, as LSP counts it. */
export interface Spot {
	line: number;
	character: number;
}

/** A problem as the editor underlines it: LSP's Diagnostic, as the server said it. */
export interface Problem {
	range: { start: Spot; end: Spot };
	severity: number;
	message: string;
	source?: string;
}

/** A file's problems from every server that knows it; undefined: none knows it, or none is up. */
export async function problemsOf(profile: string, root: string, machine: string | undefined, file: string, text: string, ms = 8000): Promise<Problem[] | undefined> {
	let looked = false;
	const out: Problem[] = [];
	for (const [, c] of clientsFor(profile, root, machine, file)) {
		const d = await c.check(file, text, ms);
		if (d === undefined) continue;
		looked = true;
		for (const x of d) out.push({ range: x.range, severity: x.severity ?? 1, message: x.message, ...(x.source ? { source: x.source } : {}) });
	}
	return looked ? out : undefined;
}

/** Errors and warnings the project's running servers know now, by file (a path where they run). */
export function knownProblems(profile: string): { path: string; problems: Problem[] }[] {
	const out = new Map<string, Problem[]>();
	for (const [key, c] of clients) {
		if (!key.startsWith(`${profile}|`)) continue;
		for (const [uri, d] of c.known()) {
			const found = d.filter((x) => (x.severity ?? 1) <= 2);
			if (!found.length) continue;
			const list = out.get(uri) ?? [];
			for (const x of found) list.push({ range: x.range, severity: x.severity ?? 1, message: x.message, ...(x.source ? { source: x.source } : {}) });
			out.set(uri, list);
		}
	}
	return [...out].map(([uri, problems]) => ({ path: pathOf(uri), problems })).sort((a, b) => a.path.localeCompare(b.path));
}

/** LSP's hover contents (a string, a marked string, a list of them, or markup) as Markdown. */
function markdownOf(c: any): string {
	if (!c) return "";
	if (typeof c === "string") return c;
	if (Array.isArray(c)) return c.map(markdownOf).filter(Boolean).join("\n\n");
	if (typeof c.language === "string") return "```" + c.language + "\n" + c.value + "\n```";
	return String(c.value ?? "");
}

/** What the first server that answers says of the symbol at `at`, as Markdown; "" for nothing. */
export async function hoverAt(profile: string, root: string, machine: string | undefined, file: string, text: string, at: Spot, ms = 4000): Promise<string> {
	for (const [, c] of clientsFor(profile, root, machine, file)) {
		const h = await c.ask("textDocument/hover", file, text, { position: at }, ms);
		const md = markdownOf(h?.contents).trim();
		if (md) return md;
	}
	return "";
}

/** A place a symbol is defined: the file (a path on the project's machine) and its range. */
export interface Place {
	path: string;
	range: { start: Spot; end: Spot };
}

const pathOf = (uri: string) => decodeURIComponent(uri.replace(/^file:\/\//, ""));

/** Where the symbol at `at` is defined, from the first server that knows: Location, Location[]
 * or LocationLink[] as the places themselves. */
export async function definitionAt(profile: string, root: string, machine: string | undefined, file: string, text: string, at: Spot, ms = 4000): Promise<Place[]> {
	for (const [, c] of clientsFor(profile, root, machine, file)) {
		const places = placesOf(await c.ask("textDocument/definition", file, text, { position: at }, ms));
		if (places.length) return places;
	}
	return [];
}

/** Places from LSP's Location, Location[] or LocationLink[], files only. */
function placesOf(d: any): Place[] {
	const list: any[] = !d ? [] : Array.isArray(d) ? d : [d];
	return list
		.map((l) => (l.targetUri ? { uri: l.targetUri, range: l.targetSelectionRange ?? l.targetRange } : l))
		.filter((l) => typeof l?.uri === "string" && l.uri.startsWith("file://") && l.range)
		.map((l) => ({ path: pathOf(l.uri), range: l.range }));
}

/** Every place the symbol at `at` is used, its definition among them, from the first server that knows. */
export async function referencesAt(profile: string, root: string, machine: string | undefined, file: string, text: string, at: Spot, ms = 8000): Promise<Place[]> {
	for (const [, c] of clientsFor(profile, root, machine, file)) {
		const places = placesOf(await c.ask("textDocument/references", file, text, { position: at, context: { includeDeclaration: true } }, ms));
		if (places.length) return places;
	}
	return [];
}

/** A symbol as a server names it: its name, its kind ("function", "struct"…), where, and inside what. */
export interface Symbol {
	name: string;
	kind: string;
	path: string;
	line: number;
	/** What it is in: "impl Thread", "class Runtime". */
	container?: string;
	/** Nesting in a file's outline, 0 at the top. */
	depth?: number;
}

const KINDS = ["", "file", "module", "namespace", "package", "class", "method", "property", "field", "constructor", "enum", "interface", "function", "variable", "constant", "string", "number", "boolean", "array", "object", "key", "null", "enum member", "struct", "event", "operator", "type parameter"];

/** A file's outline, in order: DocumentSymbol trees flattened with their depth, or SymbolInformation as given. */
export async function outlineOf(profile: string, root: string, machine: string | undefined, file: string, text: string, ms = 8000): Promise<Symbol[]> {
	for (const [, c] of clientsFor(profile, root, machine, file)) {
		const d = await c.ask("textDocument/documentSymbol", file, text, {}, ms);
		if (!Array.isArray(d) || !d.length) continue;
		const out: Symbol[] = [];
		// In line order (tsserver gives them by name), and not inside functions: their locals are
		// not the file's shape.
		const line = (s: any) => (s.selectionRange ?? s.range ?? s.location?.range)?.start?.line ?? 0;
		const walk = (list: any[], depth: number, container?: string) => {
			for (const s of [...list].sort((a, b) => line(a) - line(b))) {
				const range = s.selectionRange ?? s.range ?? s.location?.range;
				out.push({ name: String(s.name), kind: KINDS[s.kind] ?? "symbol", path: s.location?.uri ? pathOf(s.location.uri) : file, line: (range?.start?.line ?? 0) + 1, depth, ...(container ?? s.containerName ? { container: container ?? s.containerName } : {}) });
				if (Array.isArray(s.children) && ![6, 9, 12].includes(s.kind)) walk(s.children, depth + 1, String(s.name));
			}
		};
		walk(d, 0);
		return out;
	}
	return [];
}

/**
 * Symbols named like `query` in the whole project (`workspace/symbol`), from every server of the
 * project; `open` gives each server a file of its kind first (`files` by extension), as tsserver
 * needs one to know the project.
 */
export async function symbolsNamed(
	profile: string,
	root: string,
	machine: string | undefined,
	query: string,
	open: (extensions: string[]) => Promise<{ file: string; text: string }[]>,
	ms = 8000,
): Promise<Symbol[]> {
	const out: Symbol[] = [];
	for (const [name, spec] of Object.entries(readLsp(profile))) {
		const exts = (spec.extensions ?? []).map((e) => e.replace(/^\./, "").toLowerCase());
		const files = await open(exts);
		if (!files.length) continue;
		const c = clientsFor(profile, root, machine, files[0].file).find(([n]) => n === name)?.[1];
		const d = await c?.request("workspace/symbol", { query }, ms, files);
		for (const s of Array.isArray(d) ? d : []) {
			const uri = s.location?.uri;
			if (typeof uri !== "string" || !uri.startsWith("file://")) continue;
			out.push({ name: String(s.name), kind: KINDS[s.kind] ?? "symbol", path: pathOf(uri), line: (s.location.range?.start?.line ?? 0) + 1, ...(s.containerName ? { container: String(s.containerName) } : {}) });
		}
	}
	return out;
}
