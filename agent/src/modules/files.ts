// The machine's files as Bench reads and writes them: /files, a skill's tree, attachments, git
// worktrees. /api/fs/* take an optional `machine` (an ~/.ssh/config alias): a project this agent
// works on a server (`ssh-env.ts`) has its files there. Either way they go through Pi's
// `ExecutionEnv` — this machine's, or that one's over SSH.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { BENCH_HOME } from "../config.ts";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";
import { getOrThrow } from "@earendil-works/pi-durable/env";
import { dataUrl, expand, HttpError, isDir, required, type Module } from "../http.ts";
import { machineEnv } from "../ssh-env.ts";

/** A path on `machine` (its `~` its login directory) and its environment there; None: this one's. */
export async function onMachine(machine: string | null | undefined, p: string) {
	if (!machine) return undefined;
	const env = await machineEnv(machine, "");
	const abs = p === "~" ? env.cwd : p.startsWith("~/") ? path.posix.join(env.cwd, p.slice(2)) : path.posix.resolve(env.cwd, p || ".");
	return { env, path: abs };
}

/** Pi's file errors as HTTP answers. */
const fsCall = async <T>(f: () => Promise<T>): Promise<T> => {
	try {
		return await f();
	} catch (e: any) {
		const status = e?.code === "not_found" ? 404 : e?.code === "permission_denied" ? 403 : 400;
		throw new HttpError(status, String(e?.message ?? e));
	}
};


/** A folder with everything in it: paths relative to it, folders first at each level. */
function tree(root: string): { path: string; dir: boolean }[] {
	const out: { path: string; dir: boolean }[] = [];
	const walk = (rel: string, depth: number) => {
		if (depth > 8 || out.length >= 3000) return;
		const entries = fs.readdirSync(path.join(root, rel), { withFileTypes: true }).filter((e) => ![".git", "node_modules", "target", ".DS_Store"].includes(e.name));
		entries.sort((a, b) => Number(b.isDirectory()) - Number(a.isDirectory()) || a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
		for (const e of entries) {
			const p = rel ? `${rel}/${e.name}` : e.name;
			out.push({ path: p, dir: e.isDirectory() });
			if (e.isDirectory()) walk(p, depth + 1);
		}
	};
	walk("", 0);
	return out;
}

export default (): Module => ({
	routes: {
		"DELETE /api/files": (req) => {
			fs.rmSync(expand(req.body.path), { recursive: Boolean(req.body.recursive), force: true });
			return { ok: true };
		},
		"POST /api/files/mkdir": (req) => {
			fs.mkdirSync(expand(req.body.path), { recursive: true });
			return { ok: true };
		},
		"POST /api/files/upload": (req) => {
			const p = expand(req.body.path);
			if (fs.existsSync(p) && req.body.overwrite === false) throw new HttpError(409, "exists");
			fs.mkdirSync(path.dirname(p), { recursive: true });
			fs.writeFileSync(p, dataUrl(req.body.data_url));
			return { ok: true, path: p };
		},
		"GET /api/fs/read-text": async (req) => {
			const m = await onMachine(req.query.get("machine"), req.query.get("path") ?? "");
			const p = m?.path ?? expand(req.query.get("path") ?? "");
			const buf = m ? Buffer.from(await fsCall(async () => getOrThrow(await m.env.readBinaryFile(p, ctx)))) : fs.readFileSync(p);
			const binary = buf.subarray(0, 8000).includes(0);
			const cap = 2_000_000;
			return { text: binary ? "" : buf.subarray(0, cap).toString("utf8"), truncated: buf.length > cap, binary };
		},
		"POST /api/fs/write-text": async (req) => {
			const m = await onMachine(req.body.machine, req.body.path);
			if (m) {
				await fsCall(async () => getOrThrow(await m.env.writeFile(m.path, String(req.body.content ?? ""), ctx)));
				return { ok: true };
			}
			const p = expand(req.body.path);
			fs.mkdirSync(path.dirname(p), { recursive: true });
			fs.writeFileSync(p, String(req.body.content ?? ""));
			return { ok: true };
		},
		// /files: a new folder, and removing an entry (a folder with what is in it; Bench asks first).
		"POST /api/fs/mkdir": async (req) => {
			const m = await onMachine(req.body?.machine, required(req.body?.path, "path"));
			if (m) {
				await fsCall(async () => getOrThrow(await m.env.createDir(m.path, { recursive: true }, ctx)));
				return { ok: true };
			}
			fs.mkdirSync(expand(required(req.body?.path, "path")), { recursive: true });
			return { ok: true };
		},
		"POST /api/fs/delete": async (req) => {
			const m = await onMachine(req.body?.machine, required(req.body?.path, "path"));
			const p = m?.path ?? expand(required(req.body?.path, "path"));
			if (p === "/" || p === (m ? m.env.cwd : os.homedir())) throw new HttpError(400, "not removing the root or the home folder");
			if (m) {
				await fsCall(async () => getOrThrow(await m.env.remove(p, { recursive: true }, ctx)));
				return { ok: true };
			}
			fs.rmSync(p, { recursive: true });
			return { ok: true };
		},
		"POST /api/fs/rename": async (req) => {
			const m = await onMachine(req.body?.machine, required(req.body?.from, "from"));
			if (m) {
				const to = (await onMachine(req.body.machine, required(req.body?.to, "to")))!.path;
				if (getOrThrow(await m.env.exists(to, ctx))) throw new HttpError(409, `${path.basename(to)} exists`);
				await fsCall(async () => getOrThrow(await m.env.renameFile(m.path, to, ctx)));
				return { ok: true };
			}
			const from = expand(required(req.body?.from, "from"));
			const to = expand(required(req.body?.to, "to"));
			if (fs.existsSync(to)) throw new HttpError(409, `${path.basename(to)} exists`);
			fs.renameSync(from, to);
			return { ok: true };
		},
		"GET /api/fs/tree": (req) => ({ entries: tree(expand(required(req.query.get("path"), "path"))) }),
		"GET /api/fs/list": async (req) => {
			const m = await onMachine(req.query.get("machine"), req.query.get("path") ?? "~");
			if (m) {
				const r = await m.env.listDir(m.path, ctx);
				if (!r.ok) return { entries: [], error: r.error.message };
				// A link to a folder opens as one, as here.
				const entries = await Promise.all(
					r.value.map(async (e) => ({ name: e.name, path: e.path, isDirectory: e.kind === "directory" || (e.kind === "symlink" && (await m.env.listDir(e.path, ctx)).ok) })),
				);
				return { entries };
			}
			const p = expand(req.query.get("path") ?? os.homedir());
			try {
				return { entries: fs.readdirSync(p, { withFileTypes: true }).map((e) => ({ name: e.name, path: path.join(p, e.name), isDirectory: e.isDirectory() || (e.isSymbolicLink() && isDir(path.join(p, e.name))) })) };
			} catch (e) {
				return { entries: [], error: String((e as Error).message) };
			}
		},
		"GET /api/fs/default-cwd": async (req) => ({ cwd: (await onMachine(req.query.get("machine"), "~"))?.path ?? os.homedir() }),
		// A branch in its own folder, <repo>/.worktrees/<name>.
		"POST /api/git/worktree/add": (req) => {
			const repo = expand(String(req.body?.path ?? ""));
			const name = String(req.body?.name ?? "").trim().toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 60) || `idea-${Date.now()}`;
			const folder = path.join(repo, ".worktrees", name);
			const branch = `bench/${name}`;
			execFileSync("git", ["-C", repo, "worktree", "add", "-b", branch, folder], { stdio: "pipe" });
			return { path: folder, branch };
		},
		"POST /api/git/worktree/remove": (req) => {
			const repo = expand(String(req.body?.path ?? ""));
			execFileSync("git", ["-C", repo, "worktree", "remove", ...(req.body?.force ? ["--force"] : []), expand(String(req.body?.worktreePath ?? ""))], { stdio: "pipe" });
			return { ok: true };
		},
		"GET /api/chat/workspaces": () => ({ home: os.homedir() }),
		"POST /api/chat/image-upload": (req) => {
			const dir = path.join(BENCH_HOME, "attachments");
			fs.mkdirSync(dir, { recursive: true });
			const p = path.join(dir, `${Date.now()}-${path.basename(String(req.body.filename ?? "image.png"))}`);
			fs.writeFileSync(p, dataUrl(req.body.data_url));
			return { path: p };
		},
	},
});
