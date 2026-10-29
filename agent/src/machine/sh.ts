// A POSIX script run on a project's machine — over SSH (`ssh.ts`) through Pi's `ExecutionEnv`, or
// here — for what reads a folder there in one go: git's trees of a turn (`runtime/changes.ts`), a
// review's diff, the specs of a repository, the Explorer's searches. Failures are plain errors: the
// routes that call it say what they mean in HTTP (`modules/files.ts`).

import { execFile } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";
import { machineEnv } from "./ssh.ts";

/** A path on `machine` (its `~` its login directory) and its environment there; undefined: this one's. */
export async function onMachine(machine: string | null | undefined, p: string) {
	if (!machine) return undefined;
	const env = await machineEnv(machine, "");
	const abs = p === "~" ? env.cwd : p.startsWith("~/") ? path.posix.join(env.cwd, p.slice(2)) : path.posix.resolve(env.cwd, p || ".");
	return { env, path: abs };
}

/** What a POSIX script run in `dir` with its arguments as $1, $2… printed, whole, on `machine` (over SSH) or here. */
export async function shOut(machine: string | null, dir: string, script: string, args: string[]): Promise<string> {
	const full = `cd "$1" || exit 1; shift; ${script}`;
	const m = await onMachine(machine, dir);
	let out = "";
	if (m) {
		const r = await m.env.exec(["sh", "-c", full, "sh", m.path, ...args], { onOutput: (t: string) => (out += t) }, ctx);
		if (!r.ok) throw new Error(r.error.message);
	} else {
		const here = dir.startsWith("~") ? path.join(os.homedir(), dir.slice(1)) : dir;
		out = await new Promise<string>((resolve) =>
			execFile("sh", ["-c", full, "sh", here, ...args], { maxBuffer: 64 << 20 }, (_e, stdout) => resolve(String(stdout ?? ""))),
		);
	}
	return out;
}
