// The machine's files as Bench reads and writes them: /files, a skill's tree, attachments. /api/fs/* take an optional `machine` (an ~/.ssh/config alias): a project this agent
// works on a server (`machine/ssh.ts`) has its files there. Either way they go through Pi's
// `ExecutionEnv` — this machine's, or that one's over SSH.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";
import { getOrThrow } from "@earendil-works/pi-durable/env";
import { dataUrl, expand, HttpError, isDir, required, type Module } from "../http.ts";
import { onMachine, shOut as runSh } from "../machine/sh.ts";

/** Pi's file errors as HTTP answers. */
const fsCall = async <T>(f: () => Promise<T>): Promise<T> => {
	try {
		return await f();
	} catch (e: any) {
		const status = e?.code === "not_found" ? 404 : e?.code === "permission_denied" ? 403 : 400;
		throw new HttpError(status, String(e?.message ?? e));
	}
};


/** Folders the Explorer's search leaves out, as the agent's grep and glob do (`builtin/search.ts`). */
const SKIP = [".git", "node_modules", "target", "dist", "build", ".next", "vendor", ".venv", "__pycache__"];
const skipRg = SKIP.map((d) => `-g '!${d}'`).join(" ");
const skipGrep = SKIP.map((d) => `--exclude-dir=${d}`).join(" ");
const skipFind = SKIP.map((d) => `-name ${d} -prune -o`).join(" ");

/** A POSIX script run in `dir` with its arguments as $1, $2…, on `machine` (over SSH) or here;
 * what it printed, at most `cap` lines. */
async function shIn(machine: string | null, dir: string, script: string, args: string[], cap: number): Promise<string[]> {
	return (await shOut(machine, dir, script, args)).split("\n").filter(Boolean).slice(0, cap);
}

/** What a POSIX script run in `dir` printed (`machine/sh.ts`), its failure a 400. */
async function shOut(machine: string | null, dir: string, script: string, args: string[]): Promise<string> {
	try {
		return await runSh(machine, dir, script, args);
	} catch (e) {
		throw new HttpError(400, String((e as Error)?.message ?? e));
	}
}

/** A file's text at HEAD of the repository it is in; null: not in one, or not committed. */
export async function textAtHead(machine: string | null, file: string): Promise<string | null> {
	const script = `if git cat-file -e "HEAD:./$1" 2>/dev/null; then printf T; git show "HEAD:./$1"; else printf N; fi`;
	const out = await shOut(machine, path.posix.dirname(file), script, [path.posix.basename(file)]);
	return out.startsWith("T") ? out.slice(1) : null;
}

export default (): Module => ({
	routes: {
		"POST /api/files/mkdir": (req) => {
			fs.mkdirSync(expand(req.body.path), { recursive: true });
			return { ok: true };
		},
		"POST /api/files/upload": async (req) => {
			// On a project's machine (this agent working on a server): through its environment.
			const m = await onMachine(req.body?.machine, String(req.body?.path ?? ""));
			if (m) {
				await fsCall(async () => getOrThrow(await m.env.writeFile(m.path, dataUrl(req.body.data_url), ctx)));
				return { ok: true, path: m.path };
			}
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
		// The Explorer's ⌘P: every file under a folder (paths relative to it), ripgrep when it is
		// there, find otherwise.
		"GET /api/fs/files": async (req) => {
			const script = `if command -v rg >/dev/null 2>&1; then rg --files --hidden --color never ${skipRg}; else find . ${skipFind} -type f -print | sed 's|^\\./||'; fi 2>/dev/null`;
			const files = await shIn(req.query.get("machine"), required(req.query.get("path"), "path"), script, [], 20000);
			return { files };
		},
		// The Explorer's ⌘⇧F: lines holding a text (not a pattern), as path:line:text.
		"GET /api/fs/grep": async (req) => {
			const q = required(req.query.get("q"), "q");
			const script = `if command -v rg >/dev/null 2>&1; then rg -n -F -i --no-heading --color never --max-columns 300 ${skipRg} -e "$1" .; else grep -rnIF -i ${skipGrep} -e "$1" .; fi 2>/dev/null`;
			const lines = await shIn(req.query.get("machine"), required(req.query.get("path"), "path"), script, [q], 500);
			const hits = lines.flatMap((l) => {
				const m = /^(?:\.\/)?(.+?):(\d+):(.*)$/.exec(l);
				return m ? [{ path: m[1], line: Number(m[2]), text: m[3].slice(0, 300) }] : [];
			});
			return { hits };
		},
		// The Explorer's editor marks the lines changed since the last commit (`git show HEAD:`).
		"GET /api/git/head": async (req) => ({ text: await textAtHead(req.query.get("machine"), required(req.query.get("path"), "path")) }),
		// A file as a tree has it (a turn's start or end, `runtime/changes.ts`): its text, or null
		// when the tree has no such file (made, or removed, in that turn).
		"GET /api/git/show": async (req) => {
			const root = required(req.query.get("root"), "root");
			const rel = path.posix.relative(root, required(req.query.get("path"), "path"));
			const script = `if git cat-file -e "$1:$2" 2>/dev/null; then printf T; git show "$1:$2"; else printf N; fi`;
			const out = await shOut(req.query.get("machine"), root, script, [required(req.query.get("tree"), "tree"), rel]);
			return { text: out.startsWith("T") ? out.slice(1) : null };
		},
		// The Explorer dims what git ignores: the ignored paths of the repository around a folder,
		// from its root (--directory folds an ignored folder into one line: node_modules/). Not a
		// repository: an empty list. Ignored-but-tracked files (-c -i) are dimmed too, as in Zed.
		"GET /api/git/ignored": async (req) => {
			const script = `root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0; cd "$root"; printf 'ROOT\\t%s\\n' "$root"; git ls-files -o -c -i --exclude-standard --directory 2>/dev/null`;
			const lines = await shIn(req.query.get("machine"), required(req.query.get("path"), "path"), script, [], 5000);
			const root = lines.find((l) => l.startsWith("ROOT\t"))?.slice(5) ?? "";
			return { root, ignored: lines.filter((l) => !l.startsWith("ROOT\t")) };
		},
		"GET /api/fs/default-cwd": async (req) => ({ cwd: (await onMachine(req.query.get("machine"), "~"))?.path ?? os.homedir() }),
		"GET /api/chat/workspaces": () => ({ home: os.homedir() }),
	},
});
