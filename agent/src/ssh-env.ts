// A project's machine over SSH, as Pi's coding tools see it: an `ExecutionEnv` (Pi Durable's
// environment: files and a shell) whose files and commands are on a server
// (`docs/remote-projects.md`). Pi is unchanged: the agent gives a conversation of such a project
// this environment instead of `NodeExecutionEnv`, and read/write/edit/bash work there.
//
// One connection per server: OpenSSH with ControlMaster (`~/.bench/ssh/%C`), so every command and
// file operation rides it without a new handshake; the connection itself is the alias's block in
// ~/.ssh/config. The server needs only `sh` (and GNU or BSD stat) — nothing is installed there.
// Commands run through `NodeExecutionEnv` locally as `ssh … host '<script>'`: its timeout, abort,
// output window and spill apply as they are. Files are POSIX commands over the same connection.

import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Context } from "@earendil-works/chord";
import {
	type BinaryReader,
	type DirReader,
	type ExecutionEnv,
	ExecutionError,
	FileError,
	type FileErrorCode,
	type FileInfo,
	type FileKind,
	type FileWatcher,
	LineScanner,
	type LineScan,
	type Result,
	type ShellExecOptions,
	type ShellExecResult,
	type TextLine,
	type TextLineReader,
	type WatchChange,
	type WatchTarget,
	err,
	ok,
} from "@earendil-works/pi-durable/env";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { BACKGROUND_CONTEXT as BACKGROUND } from "@earendil-works/chord/context";
import { BENCH_HOME } from "./config.ts";

const posix = path.posix;

/** The ssh program (tests put a stand-in that runs the script here). */
const sshProgram = () => process.env.BENCH_SSH ?? "ssh";

/** Options every call to `host` takes: the shared connection, no prompts. */
export function sshOptions(host: string): string[] {
	// A socket path is at most 104 bytes; %C is 40: a long home puts them in a short folder.
	const own = path.join(BENCH_HOME, "ssh");
	const dir = own.length + 41 < 100 ? own : path.join("/tmp", `bench-ssh-${os.userInfo().uid}`);
	fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
	return [
		"-o", "BatchMode=yes",
		"-o", "ConnectTimeout=10",
		"-o", "ServerAliveInterval=15",
		"-o", "ControlMaster=auto",
		"-o", `ControlPath=${dir}/%C`,
		"-o", "ControlPersist=600",
		host,
	];
}

/** `s` as one word for a POSIX shell (and for the login shell ssh hands the command to). */
export const quote = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;

// Exit codes the file scripts answer with, as Pi's file errors.
const CODES: Record<number, FileErrorCode> = { 10: "not_found", 11: "is_directory", 12: "not_directory", 13: "permission_denied", 14: "invalid" };

/**
 * `st PATH`: `kind|size|mtime` of PATH without following a final symbolic link (GNU or BSD stat);
 * the kind is stat's own words, `kindOf` reads them.
 */
const PRELUDE = `st() { if stat -c %s / >/dev/null 2>&1; then stat -c '%F|%s|%.9Y' -- "$1"; else stat -f '%HT|%z|%Fm' -- "$1"; fi; }\n`;

function kindOf(word: string): FileKind | undefined {
	const w = word.toLowerCase();
	if (w.startsWith("regular")) return "file";
	if (w === "directory") return "directory";
	if (w === "symbolic link") return "symlink";
	return undefined;
}

/** One `st` line as Pi's file info. */
function infoOf(p: string, line: string): FileInfo | undefined {
	const [word, size, mtime] = line.trim().split("|");
	const kind = kindOf(word ?? "");
	if (!kind) return undefined;
	return { name: posix.basename(p), path: p, kind, size: Number(size) || 0, mtimeMs: Math.round(Number(mtime) * 1000) || 0 };
}

const aborted = (context: Context, p?: string) => (context.abortSignal?.aborted ? err<never, FileError>(new FileError("aborted", "aborted", p)) : undefined);

type Run = { code: number; stdout: Buffer; stderr: string };

const envs = new Map<string, SshExecutionEnv>();
const homes = new Map<string, string>();

/**
 * The environment of `host` working in `folder` (empty: the login directory there), one per
 * place: conversations of a project share it.
 */
export async function machineEnv(host: string, folder: string): Promise<SshExecutionEnv> {
	let dir = folder;
	if (!dir.startsWith("/")) {
		let home = homes.get(host);
		if (!home) {
			const r = await new SshExecutionEnv(host, "/").run(`printf %s "$HOME"`, BACKGROUND);
			home = r.code === 0 && r.stdout.length ? r.stdout.toString() : "/";
			homes.set(host, home);
		}
		dir = posix.resolve(home, dir || ".");
	}
	const key = `${host}\0${dir}`;
	let env = envs.get(key);
	if (!env) envs.set(key, (env = new SshExecutionEnv(host, dir)));
	return env;
}

export class SshExecutionEnv implements ExecutionEnv {
	readonly id: string;
	cwd: string;
	readonly host: string;
	private local = new NodeExecutionEnv({ cwd: os.tmpdir() });
	private links: string | undefined;

	constructor(host: string, cwd: string) {
		this.host = host;
		this.id = `ssh:${host}`;
		this.cwd = cwd || ".";
	}

	/** Run a shell script on the server, `input` on its stdin. */
	run(script: string, context: Context, input?: Uint8Array): Promise<Run> {
		return new Promise((resolve) => {
			const child = spawn(sshProgram(), [...sshOptions(this.host), `sh -c ${quote(PRELUDE + script)}`], { stdio: ["pipe", "pipe", "pipe"] });
			const out: Buffer[] = [];
			let stderr = "";
			const onAbort = () => child.kill("SIGTERM");
			context.abortSignal?.addEventListener("abort", onAbort, { once: true });
			child.stdout.on("data", (d: Buffer) => out.push(d));
			child.stderr.on("data", (d: Buffer) => (stderr += d.toString()));
			child.on("error", (e) => {
				context.abortSignal?.removeEventListener("abort", onAbort);
				resolve({ code: 255, stdout: Buffer.alloc(0), stderr: e.message });
			});
			child.on("close", (code) => {
				context.abortSignal?.removeEventListener("abort", onAbort);
				resolve({ code: code ?? 255, stdout: Buffer.concat(out), stderr });
			});
			child.stdin.on("error", () => {});
			child.stdin.end(input ?? undefined);
		});
	}

	/** A file script's answer: its output, or the file error its exit code names. */
	private async file(script: string, p: string, context: Context, input?: Uint8Array): Promise<Result<Buffer, FileError>> {
		const stop = aborted(context, p);
		if (stop) return stop;
		const r = await this.run(script, context, input);
		if (context.abortSignal?.aborted) return err(new FileError("aborted", "aborted", p));
		if (r.code === 0) return ok(r.stdout);
		const code = CODES[r.code] ?? (/permission denied/i.test(r.stderr) ? "permission_denied" : /no such file/i.test(r.stderr) ? "not_found" : "unknown");
		const said = code === "unknown" ? `failed (${r.code})` : code.replace("_", " ");
		return err(new FileError(code, r.stderr.trim() || `${this.host}: ${said}: ${p}`, p));
	}

	private resolve(p: string): string {
		return posix.resolve(this.cwd, p);
	}

	async absolutePath(p: string): Promise<Result<string, FileError>> {
		return ok(this.resolve(p));
	}

	async joinPath(parts: string[]): Promise<Result<string, FileError>> {
		return ok(posix.join(...parts));
	}

	// ---- commands ------------------------------------------------------------------------------

	/**
	 * A string runs through bash there (sh without it), as the local shell runs it here; an argv
	 * runs its program without shell parsing. In the working directory, with the given variables.
	 */
	async exec(command: string | readonly string[], options: ShellExecOptions | undefined, context: Context): Promise<Result<ShellExecResult, ExecutionError>> {
		if (context.abortSignal?.aborted) return err(new ExecutionError("aborted", "aborted"));
		const cwd = options?.cwd ? this.resolve(options.cwd) : this.cwd;
		const env = Object.entries(options?.env ?? {})
			.filter(([k]) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(k))
			.map(([k, v]) => `export ${k}=${quote(v)}; `)
			.join("");
		let run: string;
		if (typeof command === "string") {
			run = `if command -v bash >/dev/null 2>&1; then exec bash -c ${quote(command)}; else exec sh -c ${quote(command)}; fi`;
		} else {
			if (command.length === 0) return err(new ExecutionError("spawn_error", "Empty argv: no program to run"));
			// A missing program is a spawn error, as here — not the shell's 127.
			const check = await this.run(`command -v ${quote(command[0]!)} >/dev/null 2>&1`, context);
			if (check.code !== 0) {
				if (context.abortSignal?.aborted) return err(new ExecutionError("aborted", "aborted"));
				return err(new ExecutionError("spawn_error", check.code === 255 ? `${this.host}: ${check.stderr.trim()}` : `${command[0]}: not found on ${this.host}`));
			}
			run = `exec ${command.map(quote).join(" ")}`;
		}
		const script = `cd ${quote(cwd)} 2>/dev/null || { echo "Working directory does not exist: ${cwd.replace(/"/g, "")}" >&2; exit 254; }; ${env}${run}`;
		// The local environment runs ssh: its timeout, abort, window and spill hold as they are.
		const { cwd: _cwd, env: _env, inheritEnv: _inherit, ...rest } = options ?? {};
		return this.local.exec([sshProgram(), ...sshOptions(this.host), `sh -c ${quote(script)}`], rest, context);
	}

	// ---- files ---------------------------------------------------------------------------------

	async readBinaryFile(p: string, context: Context): Promise<Result<Uint8Array, FileError>> {
		const f = this.resolve(p);
		const r = await this.file(`p=${quote(f)}; [ -e "$p" ] || exit 10; [ -d "$p" ] && exit 11; cat -- "$p" || exit 13`, f, context);
		return r.ok ? ok(new Uint8Array(r.value)) : r;
	}

	async readTextFile(p: string, context: Context): Promise<Result<string, FileError>> {
		const r = await this.readBinaryFile(p, context);
		return r.ok ? ok(new TextDecoder().decode(r.value)) : r;
	}

	async readTextLines(p: string, options: { maxLines?: number } | undefined, context: Context): Promise<Result<string[], FileError>> {
		const r = await this.readTextFile(p, context);
		if (!r.ok) return r;
		const lines = r.value.split("\n");
		if (lines.at(-1) === "") lines.pop();
		return ok(options?.maxLines === undefined ? lines : lines.slice(0, options.maxLines));
	}

	async openTextLineReader(p: string, context: Context): Promise<Result<TextLineReader, FileError>> {
		const r = await this.readTextFile(p, context);
		if (!r.ok) return r;
		const parts = r.value.split("\n");
		const lines: TextLine[] = parts.map((text, k) => ({ text, terminated: k < parts.length - 1 }));
		if (lines.at(-1)?.text === "") lines.pop();
		let at = 0;
		let closed = false;
		return ok({
			async readLine(): Promise<Result<TextLine | undefined, FileError>> {
				if (closed) return err(new FileError("invalid", "Line reader is closed", p));
				return ok(lines[at++]);
			},
			async close() {
				closed = true;
			},
		});
	}

	/**
	 * The file as opened: a hard link to it in a folder of ours on the server keeps the same file
	 * when its path is renamed or replaced (a copy where links cannot be made: another disk).
	 */
	async openBinaryReader(p: string, options: { noFollow?: boolean } | undefined, context: Context): Promise<Result<BinaryReader, FileError>> {
		const f = this.resolve(p);
		const stop = aborted(context, f);
		if (stop) return stop;
		const dir = await this.linkDir(context);
		if (!dir.ok) return dir;
		const link = posix.join(dir.value, `r${Date.now()}${Math.random().toString(36).slice(2, 8)}`);
		const refuse = options?.noFollow ? `[ -L "$p" ] && exit 14; ` : "";
		const opened = await this.file(
			`p=${quote(f)}; l=${quote(link)}; ${refuse}[ -e "$p" ] || exit 10; [ -d "$p" ] && exit 11; [ -r "$p" ] || exit 13; ` +
				`t=$p; if [ -L "$p" ]; then t=$(readlink -f -- "$p" 2>/dev/null || echo "$p"); fi; ln -- "$t" "$l" 2>/dev/null || cp -- "$t" "$l" || exit 13; st "$l"`,
			f,
			context,
		);
		if (!opened.ok) {
			if (opened.error.code === "invalid") return err(new FileError("invalid", "Refusing to follow a symbolic link", f));
			return opened;
		}
		const info = infoOf(f, opened.value.toString());
		if (!info) return err(new FileError("invalid", "Unsupported file type", f));
		return ok(new SshBinaryReader(this, f, link, info));
	}

	/** Our folder on the server for opened files' links. */
	private async linkDir(context: Context): Promise<Result<string, FileError>> {
		if (this.links) return ok(this.links);
		const r = await this.file(`d=$(mktemp -d "\${TMPDIR:-/tmp}/bench-env-XXXXXX") || exit 13; printf %s "$d"`, "", context);
		if (!r.ok) return r;
		this.links = r.value.toString();
		return ok(this.links);
	}

	async writeFile(p: string, content: string | Uint8Array, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		const bytes = typeof content === "string" ? new TextEncoder().encode(content) : content;
		const r = await this.file(`p=${quote(f)}; [ -d "$p" ] && exit 11; mkdir -p -- "$(dirname -- "$p")" || exit 13; cat > "$p" || exit 13`, f, context, bytes);
		return r.ok ? ok(undefined) : r;
	}

	async appendFile(p: string, content: string | Uint8Array, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		const bytes = typeof content === "string" ? new TextEncoder().encode(content) : content;
		const r = await this.file(`p=${quote(f)}; [ -d "$p" ] && exit 11; mkdir -p -- "$(dirname -- "$p")" || exit 13; cat >> "$p" || exit 13`, f, context, bytes);
		return r.ok ? ok(undefined) : r;
	}

	async truncateFile(p: string, size: number, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		if (!Number.isInteger(size) || size < 0) return err(new FileError("invalid", "Invalid size", f));
		const r = await this.file(`p=${quote(f)}; [ -e "$p" ] || exit 10; [ -d "$p" ] && exit 11; dd if=/dev/null of="$p" bs=1 seek=${size} 2>/dev/null || exit 13`, f, context);
		return r.ok ? ok(undefined) : r;
	}

	async flushFile(p: string, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		const r = await this.file(`[ -e ${quote(f)} ] || exit 10`, f, context);
		return r.ok ? ok(undefined) : r;
	}

	async renameFile(from: string, to: string, context: Context): Promise<Result<void, FileError>> {
		const [a, b] = [this.resolve(from), this.resolve(to)];
		const r = await this.file(`a=${quote(a)}; b=${quote(b)}; [ -e "$a" ] || [ -L "$a" ] || exit 10; [ -d "$b" ] && [ ! -d "$a" ] && exit 11; mv -f -- "$a" "$b" || exit 13`, a, context);
		return r.ok ? ok(undefined) : r;
	}

	async fileInfo(p: string, context: Context): Promise<Result<FileInfo, FileError>> {
		const f = this.resolve(p);
		const r = await this.file(`p=${quote(f)}; [ -e "$p" ] || [ -L "$p" ] || exit 10; st "$p"`, f, context);
		if (!r.ok) return r;
		const info = infoOf(f, r.value.toString());
		return info ? ok(info) : err(new FileError("invalid", "Unsupported file type", f));
	}

	/** Names in a folder (no `.`, `..`), NUL-separated. */
	private async names(dir: string, context: Context): Promise<Result<string[], FileError>> {
		const r = await this.file(
			`d=${quote(dir)}; [ -e "$d" ] || exit 10; [ -d "$d" ] || exit 12; cd -- "$d" || exit 13; for f in * .[!.]* ..?*; do [ -e "$f" ] || [ -L "$f" ] || continue; printf '%s\\0' "$f"; done`,
			dir,
			context,
		);
		return r.ok ? ok(r.value.toString().split("\0").filter(Boolean)) : r;
	}

	/** Info of each of `names` in `dir` that still exists, in their order. */
	private async infos(dir: string, names: string[], context: Context): Promise<Result<FileInfo[], FileError>> {
		if (names.length === 0) return ok([]);
		const r = await this.file(
			`cd -- ${quote(dir)} || exit 13; for f in ${names.map(quote).join(" ")}; do if [ -e "$f" ] || [ -L "$f" ]; then printf '%s\\0' "$f"; st "$f"; printf '\\0'; fi; done`,
			dir,
			context,
		);
		if (!r.ok) return r;
		const parts = r.value.toString().split("\0");
		const out: FileInfo[] = [];
		for (let k = 0; k + 1 < parts.length; k += 2) {
			const info = infoOf(posix.join(dir, parts[k]!), parts[k + 1]!);
			if (info) out.push(info);
		}
		return ok(out);
	}

	async listDir(p: string, context: Context): Promise<Result<FileInfo[], FileError>> {
		const d = this.resolve(p);
		const names = await this.names(d, context);
		return names.ok ? this.infos(d, names.value, context) : names;
	}

	/** The folder's names as it was opened; each page is looked at when asked (gone ones skipped). */
	async openDirReader(p: string, context: Context): Promise<Result<DirReader, FileError>> {
		const d = this.resolve(p);
		const names = await this.names(d, context);
		if (!names.ok) return names;
		const rest = names.value;
		let closed = false;
		return ok({
			next: async (maxEntries: number, ctx: Context) => {
				if (closed) return err(new FileError("invalid", "Directory reader is closed", d));
				if (!Number.isInteger(maxEntries) || maxEntries < 1) return err(new FileError("invalid", "Invalid page size", d));
				const stop = aborted(ctx, d);
				if (stop) return stop;
				const entries: FileInfo[] = [];
				while (entries.length < maxEntries && rest.length > 0) {
					const page = await this.infos(d, rest.splice(0, maxEntries - entries.length), ctx);
					if (!page.ok) return page;
					entries.push(...page.value);
				}
				return ok({ entries, done: rest.length === 0 });
			},
			close: async () => {
				closed = true;
			},
		});
	}

	async canonicalPath(p: string, context: Context): Promise<Result<string, FileError>> {
		const f = this.resolve(p);
		const r = await this.file(
			`p=${quote(f)}; [ -e "$p" ] || exit 10; if r=$(readlink -f -- "$p" 2>/dev/null); then printf %s "$r"; else cd -- "$(dirname -- "$p")" && printf '%s/%s' "$(pwd -P)" "$(basename -- "$p")"; fi`,
			f,
			context,
		);
		return r.ok ? ok(r.value.toString()) : r;
	}

	async exists(p: string, context: Context): Promise<Result<boolean, FileError>> {
		const r = await this.fileInfo(p, context);
		if (r.ok) return ok(true);
		return r.error.code === "not_found" ? ok(false) : r;
	}

	async createDir(p: string, options: { recursive?: boolean } | undefined, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		const flag = options?.recursive ?? true ? "-p " : "";
		const r = await this.file(`p=${quote(f)}; [ -e "$p" ] && [ ! -d "$p" ] && exit 12; mkdir ${flag}-- "$p" 2>/dev/null || { [ -d "$p" ] && [ -n "${flag}" ] || exit 13; }`, f, context);
		return r.ok ? ok(undefined) : r;
	}

	async remove(p: string, options: { recursive?: boolean; force?: boolean } | undefined, context: Context): Promise<Result<void, FileError>> {
		const f = this.resolve(p);
		const missing = options?.force ? "exit 0" : "exit 10";
		const dir = options?.recursive ? `rm -rf -- "$p" || exit 13` : `exit 11`;
		const r = await this.file(`p=${quote(f)}; [ -e "$p" ] || [ -L "$p" ] || ${missing}; if [ -d "$p" ] && [ ! -L "$p" ]; then ${dir}; else rm -f -- "$p" || exit 13; fi`, f, context);
		return r.ok ? ok(undefined) : r;
	}

	async createTempDir(prefix: string | undefined, context: Context): Promise<Result<string, FileError>> {
		const r = await this.file(`d=$(mktemp -d "\${TMPDIR:-/tmp}/"${quote((prefix ?? "tmp-").replace(/\//g, ""))}XXXXXX) || exit 13; printf %s "$d"`, "", context);
		return r.ok ? ok(r.value.toString()) : r;
	}

	async createTempFile(options: { prefix?: string; suffix?: string } | undefined, context: Context): Promise<Result<string, FileError>> {
		const dir = await this.createTempDir("tmp-", context);
		if (!dir.ok) return dir;
		const f = posix.join(dir.value, `${options?.prefix ?? ""}${Date.now()}${Math.random().toString(36).slice(2, 8)}${options?.suffix ?? ""}`);
		const w = await this.writeFile(f, "", context);
		return w.ok ? ok(f) : w;
	}

	/**
	 * Changes under the targets, by comparing snapshots (`polling`): a server's file system has no
	 * events that reach here. Every ~half second, one listing of all targets over the connection.
	 */
	async watch(targets: readonly WatchTarget[], onChange: (change: WatchChange) => void, context: Context): Promise<Result<FileWatcher, FileError>> {
		const stop = aborted(context);
		if (stop) return stop;
		const watched = targets.map((t) => ({ ...t, path: this.resolve(t.path) }));
		let closed = false;
		let last = await this.snapshot(watched, context);
		const tick = async () => {
			if (closed) return;
			const now = await this.snapshot(watched, context);
			if (closed) return;
			if (now === undefined) {
				if (last !== undefined) onChange({ overflow: true });
			} else if (last !== undefined) {
				const changed = new Set<string>();
				for (const [p, sig] of now) if (last.get(p) !== sig) changed.add(p);
				for (const p of last.keys()) if (!now.has(p)) changed.add(p);
				if (changed.size) onChange({ paths: [...changed] });
			}
			last = now;
			timer = setTimeout(tick, 500);
		};
		let timer = setTimeout(tick, 500);
		return ok({
			mode: "polling",
			close: async () => {
				closed = true;
				clearTimeout(timer);
			},
		});
	}

	/** path → kind|size|mtime of everything the targets cover (following a target that is a link). */
	private async snapshot(targets: readonly WatchTarget[], context: Context): Promise<Map<string, string> | undefined> {
		const script = targets
			.map((t) => {
				const names = [...(t.exclude?.hidden ? ["'.*'"] : []), ...(t.exclude?.names ?? []).map(quote)];
				// Excluded names are pruned below the target, never the target itself.
				const prune = names.length ? `\\( ! -path "$t" \\( ${names.map((n) => `-name ${n}`).join(" -o ")} \\) \\) -prune -o ` : "";
				const depth = t.recursive ? "" : "-maxdepth 1 ";
				// -H: a target that is a link is listed as itself; its signature is what it points to.
				return `t=${quote(t.path)}; if [ -e "$t" ]; then find -H "$t" ${depth}${prune}-print 2>/dev/null | while IFS= read -r f; do printf '%s\\0' "$f"; if [ "$f" = "$t" ] && [ -L "$f" ]; then st "$(readlink -f -- "$f")"; else st "$f"; fi; printf '\\0'; done; fi`;
			})
			.join("; ");
		const r = await this.run(script, context);
		if (r.code === 255) return undefined; // the connection failed: coverage unknown
		const parts = r.stdout.toString().split("\0");
		const map = new Map<string, string>();
		for (let k = 0; k + 1 < parts.length; k += 2) map.set(parts[k]!, parts[k + 1]!.trim());
		return map;
	}

	async cleanup(context: Context): Promise<void> {
		await this.local.cleanup(context);
		if (this.links) {
			await this.run(`rm -rf -- ${quote(this.links)}`, context);
			this.links = undefined;
		}
	}
}

/** Reads of one opened file (its link on the server); line scans stream it once through Pi's scanner. */
class SshBinaryReader implements BinaryReader {
	private closed = false;
	private env: SshExecutionEnv;
	private path: string;
	private link: string;
	private opened: FileInfo;

	constructor(env: SshExecutionEnv, path: string, link: string, opened: FileInfo) {
		this.env = env;
		this.path = path;
		this.link = link;
		this.opened = opened;
	}

	async info(_context: Context): Promise<Result<FileInfo, FileError>> {
		if (this.closed) return err(new FileError("invalid", "Binary reader is closed", this.path));
		return ok(this.opened);
	}

	async read(offset: number, length: number, context: Context): Promise<Result<Uint8Array, FileError>> {
		if (this.closed) return err(new FileError("invalid", "Binary reader is closed", this.path));
		if (!Number.isInteger(offset) || offset < 0 || !Number.isInteger(length) || length < 0) return err(new FileError("invalid", "Invalid range", this.path));
		const stop = aborted(context, this.path);
		if (stop) return stop;
		if (length === 0) return ok(new Uint8Array(0));
		const r = await this.env.run(`tail -c +${offset + 1} -- ${quote(this.link)} | head -c ${length}`, context);
		if (context.abortSignal?.aborted) return err(new FileError("aborted", "aborted", this.path));
		if (r.code !== 0) return err(new FileError("unknown", r.stderr.trim() || "read failed", this.path));
		return ok(new Uint8Array(r.stdout));
	}

	async scanLines(options: { startLine: number; endLine?: number }, context: Context): Promise<Result<LineScan, FileError>> {
		if (this.closed) return err(new FileError("invalid", "Binary reader is closed", this.path));
		const stop = aborted(context, this.path);
		if (stop) return stop;
		let scanner: LineScanner;
		try {
			scanner = new LineScanner(options.startLine, options.endLine);
		} catch {
			return err(new FileError("invalid", "Invalid line range", this.path));
		}
		const r = await this.env.run(`cat -- ${quote(this.link)}`, context);
		if (context.abortSignal?.aborted) return err(new FileError("aborted", "aborted", this.path));
		if (r.code !== 0) return err(new FileError("unknown", r.stderr.trim() || "read failed", this.path));
		scanner.push(new Uint8Array(r.stdout));
		return ok(scanner.finish());
	}

	async close(context: Context): Promise<void> {
		if (this.closed) return;
		this.closed = true;
		await this.env.run(`rm -f -- ${quote(this.link)}`, context);
	}
}
