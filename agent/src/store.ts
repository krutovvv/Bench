// bench-agent's session index: what Bench lists and searches (titles, sources, folders, and the
// words of the person's messages and the agent's answers, indexed from Pi's commits by
// `Journal.watch`). The transcripts themselves are Pi Durable's, in pi.sqlite.

import type { DatabaseSync } from "node:sqlite";
import { db, renameOld } from "./config.ts";
import { speedOf, totalsOf, type SpeedStat, type Totals } from "./runtime/usage.ts";

export interface ThreadRow {
	id: string;
	conversation: string;
	agent: string;
	title: string;
	source: string;
	cwd: string;
	/** The person's reasoning effort for it; "" its settings' own. (The model is one for every
	 * thread: the general agent's, `resolveModel`.) */
	effort: string;
	created: number;
	updated: number;
	message_count: number;
	chat_id?: string;
	display_name?: string;
	/** The project the thread belongs to (`projects.id`); "" = none. */
	project?: string;
	/** The thread it was forked from ("Fork from here"); "" = none. The sidebar shows it under it. */
	parent?: string;
	/** The thread's token counters as of its last finished turn; zeroes for a thread that has
	 *  not run yet. Kept so Bench shows them again when the thread is reopened. */
	usage: Totals;
	/** How fast the model wrote in the thread's last answer, as Bench measured it (src/usage.ts). */
	speed: SpeedStat;
}

export class Store {
	db: DatabaseSync;

	constructor() {
		this.db = db();
		// The first layout said "sessions" and "profile" for what are threads and agent settings.
		renameOld("ALTER TABLE sessions RENAME TO threads");
		renameOld("ALTER TABLE threads RENAME COLUMN profile TO agent");
		renameOld("ALTER TABLE search RENAME COLUMN session_id TO thread_id");
		// The /tasks list left Bench (2026-10-28): its table goes with it.
		this.db.exec("DROP TABLE IF EXISTS tasks");
		this.db.exec(`
			CREATE TABLE IF NOT EXISTS threads (
				id TEXT PRIMARY KEY, conversation TEXT NOT NULL, agent TEXT NOT NULL, title TEXT DEFAULT '',
				source TEXT DEFAULT 'local', cwd TEXT DEFAULT '', model TEXT DEFAULT '', provider TEXT DEFAULT '',
				effort TEXT DEFAULT '', created REAL, updated REAL, message_count INTEGER DEFAULT 0,
				usage TEXT DEFAULT '',
				speed TEXT DEFAULT '',
				chat_id TEXT, display_name TEXT,
				deleted INTEGER DEFAULT 0
			);
			CREATE TABLE IF NOT EXISTS search (
				thread_id TEXT NOT NULL, entry INTEGER NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, ts REAL NOT NULL,
				PRIMARY KEY (thread_id, entry, role)
			);
		`);
		// The first index was a copy written beside the journal: its words stay searchable, under
		// entries of their own (negative, before any of Pi's), and the copy goes.
		const old = this.db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'messages'").get();
		if (old) {
			this.db.exec(`INSERT OR IGNORE INTO search (thread_id, entry, role, text, ts)
				SELECT session_id, -rowid, role, text, ts FROM messages WHERE role IN ('user', 'assistant');
				DROP TABLE messages;`);
		}
		// parent: the thread a fork was made from (`Runtime.fork`).
		// usage: the thread's token counters, settled at every turn end (src/usage.ts);
		// speed: how fast the model wrote, measured by Bench and sent over at the turn's end.
		for (const col of ["project", "usage", "speed", "parent"])
			try {
				this.db.exec(`ALTER TABLE threads ADD COLUMN ${col} TEXT DEFAULT ''`);
			} catch {}
		// Pi's conversation ids are integers; early rows kept them as "125.0".
		this.db.exec("UPDATE threads SET conversation = CAST(CAST(conversation AS REAL) AS INTEGER) WHERE conversation LIKE '%.%'");
	}

	private row(r: any): ThreadRow {
		let usage: Totals;
		try {
			usage = totalsOf(JSON.parse(r.usage || "{}"));
		} catch {
			usage = totalsOf(null);
		}
		let speed: SpeedStat;
		try {
			speed = speedOf(JSON.parse(r.speed || "{}"));
		} catch {
			speed = speedOf(null);
		}
		// The plan is the journal's (the `todo` calls), the model the general agent's: old rows'
		// columns stay unread.
		const { todos: _t, todos_at: _a, plan_id: _p, model: _m, provider: _v, ...rest } = r;
		return { ...rest, usage, speed };
	}

	create(
		s: Omit<ThreadRow, "created" | "updated" | "message_count" | "title" | "usage" | "speed"> & { title?: string },
	): ThreadRow {
		const now = Date.now() / 1000;
		this.db
			.prepare("INSERT INTO threads (id, conversation, agent, title, source, cwd, effort, created, updated, chat_id, display_name, project, parent) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)")
			.run(s.id, String(s.conversation), s.agent, s.title ?? "", s.source, s.cwd, s.effort, now, now, s.chat_id ?? null, s.display_name ?? null, s.project ?? "", s.parent ?? "");
		return this.get(s.id)!;
	}

	get(id: string): ThreadRow | undefined {
		const r = this.db.prepare("SELECT * FROM threads WHERE id = ? AND deleted = 0").get(id);
		return r ? this.row(r) : undefined;
	}

	byConversation(conversation: string | number): ThreadRow | undefined {
		const r = this.db.prepare("SELECT * FROM threads WHERE conversation = ? AND deleted = 0").get(String(conversation));
		return r ? this.row(r) : undefined;
	}

	list(opts: { agent?: string; source?: string; limit?: number } = {}): ThreadRow[] {
		const where = ["deleted = 0"];
		const args: any[] = [];
		if (opts.agent) (where.push("agent = ?"), args.push(opts.agent));
		if (opts.source) (where.push("source = ?"), args.push(opts.source));
		return this.db
			.prepare(`SELECT * FROM threads WHERE ${where.join(" AND ")} ORDER BY updated DESC LIMIT ?`)
			.all(...args, opts.limit ?? 100)
			.map((r) => this.row(r));
	}

	update(id: string, fields: Partial<Omit<ThreadRow, "id">>): void {
		const sets: string[] = [];
		const args: any[] = [];
		for (const [k, v] of Object.entries(fields)) {
			sets.push(`${k} = ?`);
			args.push(k === "conversation" ? String(v) : typeof v === "object" && v !== null ? JSON.stringify(v) : v);
		}
		if (!sets.length) return;
		this.db.prepare(`UPDATE threads SET ${sets.join(", ")} WHERE id = ?`).run(...args, id);
	}

	touch(id: string): void {
		this.db.prepare("UPDATE threads SET updated = ? WHERE id = ?").run(Date.now() / 1000, id);
	}

	delete(id: string): void {
		this.db.prepare("UPDATE threads SET deleted = 1 WHERE id = ?").run(id);
		this.db.prepare("DELETE FROM search WHERE thread_id = ?").run(id);
	}

	/** The words of one journal entry, for search: the person's message or the agent's answer.
	 * The thread's count of messages follows. */
	index(threadId: string, entry: number, role: "user" | "assistant", text: string, ts: number): void {
		this.db.prepare("INSERT OR REPLACE INTO search (thread_id, entry, role, text, ts) VALUES (?,?,?,?,?)").run(threadId, entry, role, text, ts);
		this.db
			.prepare("UPDATE threads SET message_count = (SELECT COUNT(*) FROM search WHERE thread_id = ?), updated = ? WHERE id = ?")
			.run(threadId, Date.now() / 1000, threadId);
	}

	/** A fork's index: the words of the thread it came from, through entry `at`. */
	copyIndex(from: string, to: string, at: number): void {
		this.db.prepare("INSERT OR IGNORE INTO search (thread_id, entry, role, text, ts) SELECT ?, entry, role, text, ts FROM search WHERE thread_id = ? AND entry <= ?").run(to, from, at);
		this.db.prepare("UPDATE threads SET message_count = (SELECT COUNT(*) FROM search WHERE thread_id = ?) WHERE id = ?").run(to, to);
	}

	/** The index after entry `at` goes (an exchange taken back). */
	cutIndex(threadId: string, at: number): void {
		this.db.prepare("DELETE FROM search WHERE thread_id = ? AND entry > ?").run(threadId, at);
		this.db.prepare("UPDATE threads SET message_count = (SELECT COUNT(*) FROM search WHERE thread_id = ?) WHERE id = ?").run(threadId, threadId);
	}

	/** Full-text-ish search: every word must appear (case-insensitive), newest sessions first. */
	search(query: string, limit = 30, agent?: string): { thread_id: string; snippet: string; role: string; ts: number }[] {
		const words = query.toLowerCase().split(/\s+/).filter(Boolean);
		if (!words.length) return [];
		const cond = words.map(() => "lower(m.text) LIKE ?").join(" AND ");
		const rows = this.db
			.prepare(
				`SELECT m.thread_id, m.text, m.role, m.ts FROM search m JOIN threads s ON s.id = m.thread_id
				 WHERE s.deleted = 0 ${agent ? "AND s.agent = ?" : ""} AND ${cond}
				 ORDER BY m.ts DESC LIMIT ?`,
			)
			.all(...(agent ? [agent] : []), ...words.map((w) => `%${w}%`), limit * 4) as any[];
		const seen = new Set<string>();
		const out = [];
		for (const r of rows) {
			if (seen.has(r.thread_id)) continue;
			seen.add(r.thread_id);
			const at = r.text.toLowerCase().indexOf(words[0]);
			const start = Math.max(0, at - 60);
			const snippet = (start ? "…" : "") + r.text.slice(start, at) + ">>>" + r.text.slice(at, at + words[0].length) + "<<<" + r.text.slice(at + words[0].length, at + 120);
			out.push({ thread_id: r.thread_id, snippet, role: r.role, ts: r.ts });
			if (out.length >= limit) break;
		}
		return out;
	}
}
