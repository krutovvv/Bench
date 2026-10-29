// A turn reviewed (`rt.reviews`), asked by the person on the turn's changes (Bench's "Review"
// under "Changed files", `thread.review`). One call to the thread's model, thinking left as it is:
// what the person asked, the plan the agent kept, what it said it did, and the turn's diff from
// git (`changes.ts`); back come findings on lines of the files. They go into the journal as a
// display-only entry (`bench.review`) that settles the turn's changes card; the person sends them
// to the agent as review comments (`builtin/review.ts`) — nothing reaches the agent unasked.
//
// The reviewer's order of looking is JetBrains ThinkRail's (an independent reviewer of agent
// code): intent first, then scope, then what is missing — where generated code fails.

import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { randomUUID } from "node:crypto";
import { projects } from "../builtin/projects.ts";
import { shOut } from "../machine/sh.ts";
import { REVIEW_KIND } from "./journal.ts";
import type { Runtime } from "./runtime.ts";

const ctx = BACKGROUND_CONTEXT;

/** The diff a model is shown at most (characters); a longer one is cut, and says so. */
const DIFF_CAP = 60_000;

export const REVIEW_PROMPT = [
	"You are an independent code reviewer of ONE turn of a coding agent's work. You never edit anything.",
	"Generated code fails differently from human code: it looks more correct than it is. Judge the change against what the person asked, never against whether it looks plausible.",
	"",
	"Look, in this order:",
	"1. Intent: does the change do what was asked — all of it, and only it? A correct solution to a slightly different problem is the typical failure.",
	"2. Scope: anything beyond the ask (drive-by refactors, renames, new dependencies) the agent did not mention; leftover debug output, commented-out code, a pasted token or URL.",
	"3. Weakened checks: tests deleted, skipped or loosened, `|| true`, broadened ignores, type escapes.",
	"4. What is missing: for state the change sets, where is it cleared on every path (success, error, abort)? For a new reader of state, what happens when it is absent?",
	"5. Edges: errors, empty input, concurrency, cleanup.",
	"Style is not your job unless it hides a bug. Do not repeat what is fine.",
	"",
	"One concrete problem = one finding, on the changed file and lines (the new version's line numbers from the diff's hunks). Each body is one or two sentences: what is wrong and what to do instead.",
	'Reply with JSON only: {"findings": [{"path": "src/a.ts", "lines": "12-14", "body": "..."}]}. Nothing worth saying: {"findings": []}.',
].join("\n");

export interface Finding {
	path: string;
	lines?: string;
	body: string;
}

/** The model's reply read as findings; paths made absolute under the repository's root. */
export function parseFindings(root: string, out: string): Finding[] | null {
	const text = out.replace(/<think>[\s\S]*?<\/think>/g, "");
	const json = text.slice(text.indexOf("{"), text.lastIndexOf("}") + 1);
	try {
		const list = JSON.parse(json)?.findings;
		if (!Array.isArray(list)) return null;
		const base = root.replace(/\/$/, "");
		return list
			.filter((f: any) => f && typeof f.body === "string" && f.body.trim())
			.map((f: any) => {
				const p = String(f.path ?? "").trim().replace(/^\.\//, "");
				const lines = String(f.lines ?? "").trim().match(/^\d+(-\d+)?$/)?.[0];
				return { path: p.startsWith("/") || !p ? p : `${base}/${p}`, ...(lines ? { lines } : {}), body: String(f.body).trim() };
			});
	} catch {
		return null;
	}
}

export class Reviews {
	private rt: Runtime;
	/** Reviews under way, by thread and the changes they look at. */
	private running = new Set<string>();

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** Review the turn whose changes are the journal's item `of`; its findings land in the journal. */
	async review(threadId: string, of: string): Promise<void> {
		const key = `${threadId}:${of}`;
		if (this.running.has(key)) return;
		this.running.add(key);
		try {
			const live = await this.rt.thread(threadId);
			if (!live) throw new Error(`thread not found: ${threadId}`);
			const write = (data: any) =>
				live.conversation.submit({ type: "write", entry: { kind: REVIEW_KIND, data: { of, ...data } }, requestId: randomUUID() } as any, ctx);
			try {
				const findings = await this.findings(threadId, of, live.row);
				await write({ findings });
			} catch (err: any) {
				await write({ findings: [], error: String(err?.message ?? err).slice(0, 300) });
			}
		} finally {
			this.running.delete(key);
		}
	}

	private async findings(threadId: string, of: string, row: any): Promise<Finding[]> {
		const { items } = await this.rt.journal.open(threadId);
		const at = items.findIndex((i) => i.key === of && i.kind === "changes");
		const changes: any = items[at];
		if (!changes) throw new Error("the turn's changes are not in the journal");
		// The turn: from the person's last message before the changes to them.
		let from = at;
		while (from > 0 && items[from].kind !== "user") from--;
		const turn = items.slice(from, at);
		const asked = turn.filter((i: any) => i.kind === "user").map((i: any) => i.text).join("\n\n");
		const plans = turn.filter((i: any) => i.kind === "plan");
		const plan = plans.length ? (plans[plans.length - 1] as any).todos.map((t: any) => `- [${t.status}] ${t.content}`).join("\n") : "";
		const said = turn.filter((i: any) => i.kind === "text" && !i.narration).map((i: any) => i.text).join("\n\n");

		const p = row.project ? projects.get(row.project) : undefined;
		let diff = await shOut(p?.machine ?? null, changes.root, `git diff --no-color --no-renames "$1" "$2"`, [changes.base, changes.head]);
		if (diff.length > DIFF_CAP) diff = `${diff.slice(0, DIFF_CAP)}\n… (the diff goes on; cut here)`;
		const message = [
			`What the person asked:\n${asked.slice(0, 4000) || "(nothing said)"}`,
			plan && `The plan the agent kept:\n${plan}`,
			`What the agent said it did:\n${said.slice(0, 4000) || "(nothing said)"}`,
			`The turn's diff (repository ${changes.root}):\n${diff}`,
		]
			.filter(Boolean)
			.join("\n\n");

		const models = this.rt.models;
		const model = models.model(models.ref(row.agent));
		const res: any = await (models.pi as any).complete(model, { systemPrompt: REVIEW_PROMPT, messages: [{ role: "user", content: message, timestamp: Date.now() }] }, { maxTokens: 4000 });
		if (res?.stopReason === "error") throw new Error(res.errorMessage ?? "the model stopped with an error");
		const out = (res?.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("");
		const found = parseFindings(changes.root, out);
		if (!found) throw new Error("the model's reply was not the findings asked for");
		return found;
	}
}
