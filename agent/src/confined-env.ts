// A project kept in its folder: the person says its agent works there and nowhere else. Its
// conversations get this wrapper around their execution environment (Pi's `ExecutionEnv`, this
// machine's or a server's over SSH):
// - Pi's file tools (read, write, edit, grep, glob, ls, …) are refused any path outside the folder
//   — reading too, not only writing;
// - commands run in the system's sandbox with writing allowed only in the folder and the temp
//   folders: macOS's sandbox-exec here, bubblewrap on Linux when it is installed (else they run
//   as they are, in the folder, and the prompt says the rule). A command may still read outside.
// Paths are checked as written (`..` resolved); a symbolic link inside pointing out is not.

import os from "node:os";
import path from "node:path";
import type { Context } from "@earendil-works/chord";
import { type ExecutionEnv, ExecutionError, FileError, type Result, type ShellExecOptions, type ShellExecResult, err } from "@earendil-works/pi-durable/env";

const posix = path.posix;

/** The macOS sandbox agent: everything but writing outside `root` and the temp folders. */
function seatbelt(root: string): string {
	const q = (p: string) => JSON.stringify(p);
	return `(version 1)(allow default)(deny file-write* (require-not (require-any (subpath ${q(root)}) (subpath "/private/tmp") (subpath "/private/var/folders") (subpath "/dev") (subpath ${q(path.join(os.homedir(), ".bench", "lsp"))}))))`;
}

/** Linux: bubblewrap when there, with the system read-only and the folder and /tmp writable. */
const BWRAP = `if command -v bwrap >/dev/null 2>&1; then exec bwrap --ro-bind / / --bind "$1" "$1" --bind /tmp /tmp --dev /dev --proc /proc --chdir "$2" -- "$@"; else cd "$2" && shift 2 && exec "$@"; fi`;

export function confined(inner: ExecutionEnv, root: string, local: boolean): ExecutionEnv {
	const base = posix.resolve(root);
	const inside = (p: string) => {
		const abs = posix.resolve(inner.cwd, p);
		return abs === base || abs.startsWith(`${base}/`);
	};
	const refuse = (p: string) => err<never, FileError>(new FileError("permission_denied", `${p} is outside the project's folder ${base}: the project is kept in it`, p));
	/** A method whose first argument is a path. */
	const guard =
		(name: keyof ExecutionEnv) =>
		(p: string, ...rest: unknown[]) =>
			inside(p) ? (inner as any)[name](p, ...rest) : Promise.resolve(refuse(p));
	// The rest as it is, bound to the environment itself (its own fields stay its own).
	const env: any = { id: inner.id };
	Object.defineProperty(env, "cwd", { get: () => inner.cwd, set: (v: string) => (inner.cwd = v) });
	for (const name of ["absolutePath", "joinPath", "createTempDir", "createTempFile", "cleanup"] as const) env[name] = (inner as any)[name].bind(inner);
	for (const name of [
		"readTextFile", "openTextLineReader", "readTextLines", "readBinaryFile", "openBinaryReader", "writeFile", "appendFile",
		"truncateFile", "flushFile", "fileInfo", "listDir", "listFiles", "openDirReader", "canonicalPath", "exists", "createDir", "remove",
	] as const)
		if (typeof (inner as any)[name] === "function") env[name] = guard(name);
	env.renameFile = (a: string, b: string, ctx: Context) => (!inside(a) ? Promise.resolve(refuse(a)) : !inside(b) ? Promise.resolve(refuse(b)) : inner.renameFile(a, b, ctx));
	env.watch = (targets: readonly { path: string }[], onChange: any, ctx: Context) => {
		const out = targets.find((t) => !inside(t.path));
		return out ? Promise.resolve(refuse(out.path)) : inner.watch(targets as any, onChange, ctx);
	};
	env.exec = async (command: string | readonly string[], options: ShellExecOptions | undefined, ctx: Context): Promise<Result<ShellExecResult, ExecutionError>> => {
		const cwd = options?.cwd ? posix.resolve(inner.cwd, options.cwd) : inner.cwd;
		if (!inside(cwd)) return err(new ExecutionError("spawn_error", `${cwd} is outside the project's folder ${base}`));
		const argv = typeof command === "string" ? ["bash", "-c", command] : [...command];
		if (argv.length === 0) return inner.exec([], options, ctx);
		const wrapped = local && process.platform === "darwin" ? ["/usr/bin/sandbox-exec", "-p", seatbelt(base), ...argv] : ["sh", "-c", BWRAP, "sh", base, cwd, ...argv];
		return inner.exec(wrapped, { ...options, cwd }, ctx);
	};
	return env as ExecutionEnv;
}
