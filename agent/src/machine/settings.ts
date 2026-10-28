// A project's settings (SYSTEM.md, mcp.json, skills/, prompts/: `projects.env`) are on this
// agent's machine, wherever the project's work is. Its conversations' file tools reach them there
// even when the work is on a server over SSH or kept in its folder (`confined.ts`): the prompt
// tells the agent to change them with read, write and edit when asked. Every other path, and
// every command, goes to the conversation's own environment as it is.

import path from "node:path";
import type { Context } from "@earendil-works/chord";
import type { ExecutionEnv } from "@earendil-works/pi-durable/env";

const posix = path.posix;

export function withSettings(inner: ExecutionEnv, local: ExecutionEnv, dir: string): ExecutionEnv {
	const base = posix.resolve(dir);
	const mine = (p: string) => {
		const abs = posix.resolve(inner.cwd, p);
		return abs === base || abs.startsWith(`${base}/`);
	};
	/** A method whose first argument is a path. */
	const route =
		(name: keyof ExecutionEnv) =>
		(p: string, ...rest: unknown[]) =>
			((mine(p) ? local : inner) as any)[name](p, ...rest);
	// The rest as it is, bound to the environment itself (its own fields stay its own).
	const env: any = { id: inner.id };
	Object.defineProperty(env, "cwd", { get: () => inner.cwd, set: (v: string) => (inner.cwd = v) });
	for (const name of ["absolutePath", "joinPath", "createTempDir", "createTempFile", "cleanup", "exec"] as const) env[name] = (inner as any)[name].bind(inner);
	for (const name of [
		"readTextFile", "openTextLineReader", "readTextLines", "readBinaryFile", "openBinaryReader", "writeFile", "appendFile",
		"truncateFile", "flushFile", "fileInfo", "listDir", "listFiles", "openDirReader", "canonicalPath", "exists", "createDir", "remove",
	] as const)
		if (typeof (inner as any)[name] === "function") env[name] = route(name);
	env.renameFile = (a: string, b: string, ctx: Context) => (mine(a) && mine(b) ? local : inner).renameFile(a, b, ctx);
	env.watch = (targets: readonly { path: string }[], onChange: any, ctx: Context) => inner.watch(targets as any, onChange, ctx);
	return env as ExecutionEnv;
}
