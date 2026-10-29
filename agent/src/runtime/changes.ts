// What a turn changed (`rt.changes`): when a run starts, the working tree of the repository the thread
// works in is written as a git tree; when it ends, again, and the files that differ go into the
// journal as a display-only entry (`bench.changes`, the model never sees it). It catches whatever
// changed the files — an edit, a formatter, a script run with bash — and the two trees let Bench
// show each file's diff of that turn (`GET /api/git/show`).
//
// The trees are written through a temporary index, a copy of the repository's own: the person's
// index, branch and stash are never touched. The blobs land in the object store as `git add` would
// put them; git's gc drops them when nothing refers to them.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { randomUUID } from "node:crypto";
import { projects } from "../builtin/projects.ts";
import { shOut } from "../modules/files.ts";
import { specsOf } from "../specs.ts";
import { CHANGES_KIND } from "./journal.ts";
import type { Live } from "./runtime.ts";

const ctx = BACKGROUND_CONTEXT;

export { CHANGES_KIND };

/** The repository's root and the tree its working files make now; empty: not a repository. */
const SNAPSHOT = `root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0
idx=$(mktemp) || exit 0
cp "$(git rev-parse --git-path index)" "$idx" 2>/dev/null
if GIT_INDEX_FILE="$idx" git add -A . >/dev/null 2>&1; then printf '%s\\t%s' "$root" "$(GIT_INDEX_FILE="$idx" git write-tree)"; fi
rm -f "$idx"`;

/** `added<TAB>removed<TAB>path` for each file that differs between two trees ("-" for a binary). */
const NUMSTAT = `cd "$1" && git diff --numstat --no-renames "$2" "$3"`;

export interface FileChange {
	/** Absolute, as the agent reaches it. */
	path: string;
	add: number;
	del: number;
	binary?: boolean;
	/** A spec's title, when the file is one (`specs.ts`). */
	spec?: string;
}

/** `git diff --numstat` read as the files of a turn, under the repository's root. */
export function parseNumstat(root: string, out: string): FileChange[] {
	return out
		.split("\n")
		.map((l) => l.split("\t"))
		.filter((p) => p.length >= 3 && p[2])
		.map(([a, d, p]) => ({
			path: `${root.replace(/\/$/, "")}/${p}`,
			add: Number(a) || 0,
			del: Number(d) || 0,
			...(a === "-" ? { binary: true } : {}),
		}));
}

export class Changes {
	/** Each running thread's tree as its run began. */
	private base = new Map<string, Promise<{ root: string; tree: string } | null>>();

	/** Where the thread works: its machine (none: this one) and folder. */
	private place(live: Live): { machine: string | null; dir: string } | null {
		const p = live.row.project ? projects.get(live.row.project) : undefined;
		const dir = live.row.cwd || p?.folder || "";
		return dir ? { machine: p?.machine ?? null, dir } : null;
	}

	private async snapshot(live: Live): Promise<{ root: string; tree: string } | null> {
		const at = this.place(live);
		if (!at) return null;
		const out = (await shOut(at.machine, at.dir, SNAPSHOT, []).catch(() => "")).trim();
		const [root, tree] = out.split("\t");
		return root && tree ? { root, tree } : null;
	}

	/** A run begins: the tree as it is now. */
	begin(live: Live): void {
		this.base.set(live.row.id, this.snapshot(live));
	}

	/** A run ended: what differs from its start, into the journal. Never in the run's way. */
	async settle(live: Live): Promise<void> {
		const started = this.base.get(live.row.id);
		this.base.delete(live.row.id);
		const base = await started;
		if (!base) return;
		const head = await this.snapshot(live);
		if (!head || head.tree === base.tree || head.root !== base.root) return;
		const at = this.place(live)!;
		const out = await shOut(at.machine, at.dir, NUMSTAT, [base.root, base.tree, head.tree]).catch(() => "");
		const files = parseNumstat(base.root, out);
		if (!files.length) return;
		// The specs the turn changed go first, named by their titles: a decision moved.
		const { specs } = await specsOf(at.machine, base.root);
		for (const f of files) {
			const s = specs.find((s) => s.path === f.path);
			if (s) f.spec = s.title;
		}
		files.sort((a, b) => Number(!a.spec) - Number(!b.spec));
		await live.conversation.submit(
			{ type: "write", entry: { kind: CHANGES_KIND, data: { root: base.root, base: base.tree, head: head.tree, files } }, requestId: randomUUID() } as any,
			ctx,
		);
	}
}
