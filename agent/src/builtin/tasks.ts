// The person's tasks (Bench's Tasks mode, after Things), kept in bench.sqlite: title, notes, tags,
// the day it is planned for (Today from then on), "this evening", "someday", the red flag,
// completion, time spent and a running timer. Bench edits them through REST; the agent through
// the `tasks` tool. They came from Apple Reminders (Bench imports them once).

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { randomBytes } from "node:crypto";
import { db } from "../config.ts";
import { sessionOf } from "../host.ts";

export interface Task {
	id: string;
	/** The project it belongs to; "" = none. */
	project: string;
	title: string;
	notes: string;
	tags: string[];
	/** YYYY-MM-DD it is planned for. */
	day: string | null;
	evening: boolean;
	someday: boolean;
	important: boolean;
	/** YYYY-MM-DD it was done. */
	completed: string | null;
	/** Seconds spent, a running timer not counted. */
	spent: number;
	/** A running timer's start (Unix seconds). */
	timer: number | null;
	/** Unix seconds. */
	created: number;
	/** Where it came from, e.g. the reminder's identifier, so an import never doubles it. */
	source_id: string | null;
}

function table() {
	db().exec(`CREATE TABLE IF NOT EXISTS tasks (
		id TEXT PRIMARY KEY, title TEXT NOT NULL, notes TEXT DEFAULT '', tags TEXT DEFAULT '[]', day TEXT,
		evening INTEGER DEFAULT 0, someday INTEGER DEFAULT 0, important INTEGER DEFAULT 0, completed TEXT,
		spent INTEGER DEFAULT 0, timer INTEGER, created INTEGER NOT NULL, source_id TEXT UNIQUE
	)`);
	try {
		db().exec("ALTER TABLE tasks ADD COLUMN project TEXT DEFAULT ''");
	} catch {}
}

const row = (r: any): Task => ({ ...r, tags: JSON.parse(r.tags || "[]"), evening: !!r.evening, someday: !!r.someday, important: !!r.important });
const today = () => new Date().toLocaleDateString("sv-SE");

export const tasks = {
	/** Open tasks, and the ones done since `doneSince` (YYYY-MM-DD). */
	list(doneSince?: string): Task[] {
		table();
		const done = doneSince ?? "9999";
		return (db().prepare("SELECT * FROM tasks WHERE completed IS NULL OR completed >= ? ORDER BY created").all(done) as any[]).map(row);
	},
	get(id: string): Task | undefined {
		table();
		const r = db().prepare("SELECT * FROM tasks WHERE id = ?").get(id);
		return r ? row(r) : undefined;
	},
	save(t: Partial<Task> & { title: string }): Task {
		table();
		const full: Task = {
			id: t.id || randomBytes(6).toString("hex"),
			project: t.project ?? "",
			title: t.title,
			notes: t.notes ?? "",
			tags: t.tags ?? [],
			day: t.day ?? null,
			evening: t.evening ?? false,
			someday: t.someday ?? false,
			important: t.important ?? false,
			completed: t.completed ?? null,
			spent: t.spent ?? 0,
			timer: t.timer ?? null,
			created: t.created ?? Math.floor(Date.now() / 1000),
			source_id: t.source_id ?? null,
		};
		db()
			.prepare(
				`INSERT INTO tasks (id, title, notes, tags, day, evening, someday, important, completed, spent, timer, created, source_id, project)
				 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title, notes=excluded.notes, tags=excluded.tags,
				 day=excluded.day, evening=excluded.evening, someday=excluded.someday, important=excluded.important, completed=excluded.completed,
				 spent=excluded.spent, timer=excluded.timer, project=excluded.project`,
			)
			.run(full.id, full.title, full.notes, JSON.stringify(full.tags), full.day, +full.evening, +full.someday, +full.important, full.completed, full.spent, full.timer, full.created, full.source_id, full.project);
		return full;
	},
	remove(id: string): void {
		table();
		db().prepare("DELETE FROM tasks WHERE id = ?").run(id);
	},
	/** Imports tasks that carry a source id; ones already imported are skipped. Returns how many were new. */
	import(items: (Partial<Task> & { title: string; source_id: string })[]): number {
		table();
		let n = 0;
		for (const t of items) {
			if (db().prepare("SELECT 1 FROM tasks WHERE source_id = ?").get(t.source_id)) continue;
			tasks.save({ ...t, id: undefined });
			n++;
		}
		return n;
	},
};

/** Tasks one list returns at most (the count and the time total cover them all). */
const MAX_LIST = 50;
const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });
const view = (t: Task) => ({ id: t.id, title: t.title, ...(t.notes ? { notes: t.notes } : {}), ...(t.tags.length ? { tags: t.tags } : {}), day: t.day, ...(t.evening ? { evening: true } : {}), ...(t.someday ? { someday: true } : {}), ...(t.important ? { important: true } : {}), completed: t.completed, ...(t.spent ? { spent_minutes: Math.round(t.spent / 60) } : {}) });

export const tasksTool = defineTool({
	name: "tasks",
	description:
		"The person's task list (Bench's Tasks; the only place tasks live). list: status open (default), completed or all; completed ones by done_since / done_until (YYYY-MM-DD, inclusive); filter by tag; the result has count and spent_minutes_total. " +
		"add / update: title, notes, tags, day (YYYY-MM-DD, puts it in Today from then; \"\" clears it), evening, someday, important, spent_minutes, completed (YYYY-MM-DD: done on that day). complete (today, or `completed`) / reopen / remove by id; complete and remove take several ids in `ids`.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("list"), Type.Literal("add"), Type.Literal("update"), Type.Literal("complete"), Type.Literal("reopen"), Type.Literal("remove")]),
		id: Type.Optional(Type.String()),
		title: Type.Optional(Type.String()),
		notes: Type.Optional(Type.String()),
		tags: Type.Optional(Type.Array(Type.String())),
		day: Type.Optional(Type.String()),
		evening: Type.Optional(Type.Boolean()),
		someday: Type.Optional(Type.Boolean()),
		important: Type.Optional(Type.Boolean()),
		spent_minutes: Type.Optional(Type.Number({ minimum: 0 })),
		completed: Type.Optional(Type.String({ description: "YYYY-MM-DD it was done" })),
		ids: Type.Optional(Type.Array(Type.String(), { description: "complete / remove: several at once" })),
		tag: Type.Optional(Type.String({ description: "list: only tasks with this tag" })),
		status: Type.Optional(Type.Union([Type.Literal("open"), Type.Literal("completed"), Type.Literal("all")])),
		done_since: Type.Optional(Type.String({ description: "list: tasks done on or after this day (with status completed / all)" })),
		done_until: Type.Optional(Type.String({ description: "list: tasks done on or before this day" })),
	}),
	execute: async (args, api) => {
		// In a project's thread: that project's tasks only, and new ones go into it.
		let project = "";
		try {
			project = sessionOf(api.conversationId).project ?? "";
		} catch {}
		const mine = (t: Task) => !project || t.project === project;
		if (args.action === "list") {
			const status = args.status ?? (args.done_since ? "all" : "open");
			const all = tasks
				.list(status === "open" ? undefined : (args.done_since ?? "0000"))
				.filter(mine)
				.filter((t) => (status === "open" ? !t.completed : status === "completed" ? !!t.completed : true))
				.filter((t) => !t.completed || !args.done_until || t.completed <= args.done_until)
				.filter((t) => !args.tag || t.tags.some((g) => g.toLowerCase() === args.tag!.toLowerCase()));
			// The total answers "how much time on X in October" in one call; a list too long for
			// the context says how to narrow it instead of being cut off.
			const spent = Math.round(all.reduce((n, t) => n + t.spent, 0) / 60);
			const shown = all.slice(-MAX_LIST);
			return text({
				tasks: shown.map(view),
				count: all.length,
				spent_minutes_total: spent,
				...(shown.length < all.length ? { note: `showing the latest ${shown.length} of ${all.length}; narrow with done_since / done_until / tag (the counts and the total cover all ${all.length})` } : {}),
			});
		}
		if (args.action === "add") {
			if (!args.title?.trim()) throw new Error("title is required");
			return text({ success: true, task: view(tasks.save({ project, title: args.title.trim(), notes: args.notes, tags: args.tags, day: args.day || null, evening: args.evening, someday: args.someday, important: args.important, completed: args.completed || null, spent: args.spent_minutes !== undefined ? Math.round(args.spent_minutes * 60) : 0 })) });
		}
		if ((args.action === "complete" || args.action === "remove") && args.ids?.length) {
			const done: string[] = [];
			for (const id of args.ids) {
				const t = tasks.get(id);
				if (!t) continue;
				if (args.action === "remove") tasks.remove(id);
				else tasks.save({ ...t, completed: args.completed || today() });
				done.push(id);
			}
			return text({ success: true, [args.action === "remove" ? "removed" : "completed"]: done });
		}
		const t = args.id ? tasks.get(args.id) : undefined;
		if (!t) throw new Error(`no task ${args.id ?? ""}`);
		if (args.action === "remove") {
			tasks.remove(t.id);
			return text({ success: true, removed: t.id });
		}
		const next: Task = { ...t };
		if (args.action === "complete") next.completed = args.completed || today();
		else if (args.action === "reopen") next.completed = null;
		else
		{
			Object.assign(next, Object.fromEntries(Object.entries({ title: args.title, notes: args.notes, tags: args.tags, evening: args.evening, someday: args.someday, important: args.important }).filter(([, v]) => v !== undefined)));
			if (args.day !== undefined) next.day = args.day || null;
			if (args.completed !== undefined) next.completed = args.completed || null;
			if (args.spent_minutes !== undefined) next.spent = Math.round(args.spent_minutes * 60);
		}
		return text({ success: true, task: view(tasks.save(next)) });
	},
});
