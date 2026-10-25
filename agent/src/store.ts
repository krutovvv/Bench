// bench-agent's session index: what Bench lists and searches (titles, sources, folders, the
// plan, a searchable copy of the text). The transcripts themselves are Pi Durable's, in pi.sqlite.

import type { DatabaseSync } from "node:sqlite";
import { db } from "./config.ts";
import { speedOf, totalsOf, type SpeedStat, type Totals } from "./usage.ts";

export interface Todo {
	id: string;
	content: string;
	status: "pending" | "in_progress" | "completed" | "cancelled";
	parent?: string;
}

export interface SessionRow {
	id: string;
	conversation: string;
	profile: string;
	title: string;
	source: string;
	cwd: string;
	model: string;
	provider: string;
	effort: string;
	created: number;
	updated: number;
	message_count: number;
	todos: Todo[];
	/** When the plan in `todos` started (unix seconds): where Bench puts its block in the
	 *  thread. 0 = no plan of note. */
	todos_at: number;
	chat_id?: string;
	display_name?: string;
	/** What the thread is about, shown as a live block at its top: "tasks:today", "ssh:<host id>", "folder:<path>"; "" = a plain chat. */
	subject?: string;
	/** The project the thread belongs to (`projects.id`); "" = none. */
	project?: string;
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
		this.db.exec(`
			CREATE TABLE IF NOT EXISTS sessions (
				id TEXT PRIMARY KEY, conversation TEXT NOT NULL, profile TEXT NOT NULL, title TEXT DEFAULT '',
				source TEXT DEFAULT 'local', cwd TEXT DEFAULT '', model TEXT DEFAULT '', provider TEXT DEFAULT '',
				effort TEXT DEFAULT '', created REAL, updated REAL, message_count INTEGER DEFAULT 0,
				todos TEXT DEFAULT '[]', todos_at REAL DEFAULT 0, usage TEXT DEFAULT '',
				speed TEXT DEFAULT '',
				chat_id TEXT, display_name TEXT,
				deleted INTEGER DEFAULT 0
			);
			CREATE TABLE IF NOT EXISTS messages (
				session_id TEXT NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, ts REAL NOT NULL,
				tool_name TEXT, tool_call_id TEXT, args TEXT
			);
			CREATE INDEX IF NOT EXISTS messages_session ON messages(session_id, ts);
		`);
		// usage: the thread's token counters, settled at every turn end (src/usage.ts);
		// speed: how fast the model wrote, measured by Bench and sent over at the turn's end.
		for (const col of ["subject", "project", "usage", "speed"])
			try {
				this.db.exec(`ALTER TABLE sessions ADD COLUMN ${col} TEXT DEFAULT ''`);
			} catch {}
		try {
			this.db.exec("ALTER TABLE sessions ADD COLUMN todos_at REAL DEFAULT 0");
		} catch {}
		// Pi's conversation ids are integers; early rows kept them as "125.0".
		this.db.exec("UPDATE sessions SET conversation = CAST(CAST(conversation AS REAL) AS INTEGER) WHERE conversation LIKE '%.%'");
	}

	private row(r: any): SessionRow {
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
		return { ...r, todos: JSON.parse(r.todos || "[]"), todos_at: r.todos_at ?? 0, usage, speed };
	}

	create(
		s: Omit<SessionRow, "created" | "updated" | "message_count" | "todos" | "todos_at" | "title" | "usage" | "speed"> & { title?: string },
	): SessionRow {
		const now = Date.now() / 1000;
		this.db
			.prepare("INSERT INTO sessions (id, conversation, profile, title, source, cwd, model, provider, effort, created, updated, chat_id, display_name, subject, project) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
			.run(s.id, String(s.conversation), s.profile, s.title ?? "", s.source, s.cwd, s.model, s.provider, s.effort, now, now, s.chat_id ?? null, s.display_name ?? null, s.subject ?? "", s.project ?? "");
		return this.get(s.id)!;
	}

	get(id: string): SessionRow | undefined {
		const r = this.db.prepare("SELECT * FROM sessions WHERE id = ? AND deleted = 0").get(id);
		return r ? this.row(r) : undefined;
	}

	byConversation(conversation: string | number): SessionRow | undefined {
		const r = this.db.prepare("SELECT * FROM sessions WHERE conversation = ? AND deleted = 0").get(String(conversation));
		return r ? this.row(r) : undefined;
	}

	list(opts: { profile?: string; source?: string; limit?: number } = {}): SessionRow[] {
		const where = ["deleted = 0"];
		const args: any[] = [];
		if (opts.profile) (where.push("profile = ?"), args.push(opts.profile));
		if (opts.source) (where.push("source = ?"), args.push(opts.source));
		return this.db
			.prepare(`SELECT * FROM sessions WHERE ${where.join(" AND ")} ORDER BY updated DESC LIMIT ?`)
			.all(...args, opts.limit ?? 100)
			.map((r) => this.row(r));
	}

	update(id: string, fields: Partial<Omit<SessionRow, "id">>): void {
		const sets: string[] = [];
		const args: any[] = [];
		for (const [k, v] of Object.entries(fields)) {
			sets.push(`${k} = ?`);
			args.push(k === "conversation" ? String(v) : typeof v === "object" && v !== null ? JSON.stringify(v) : v);
		}
		if (!sets.length) return;
		this.db.prepare(`UPDATE sessions SET ${sets.join(", ")} WHERE id = ?`).run(...args, id);
	}

	touch(id: string): void {
		this.db.prepare("UPDATE sessions SET updated = ? WHERE id = ?").run(Date.now() / 1000, id);
	}

	delete(id: string): void {
		this.db.prepare("UPDATE sessions SET deleted = 1 WHERE id = ?").run(id);
		this.db.prepare("DELETE FROM messages WHERE session_id = ?").run(id);
	}

	addMessage(sessionId: string, m: { role: string; text: string; tool_name?: string; tool_call_id?: string; args?: unknown }): void {
		this.db
			.prepare("INSERT INTO messages (session_id, role, text, ts, tool_name, tool_call_id, args) VALUES (?,?,?,?,?,?,?)")
			.run(sessionId, m.role, m.text, Date.now() / 1000, m.tool_name ?? null, m.tool_call_id ?? null, m.args === undefined ? null : JSON.stringify(m.args));
		if (m.role === "user" || m.role === "assistant")
			this.db.prepare("UPDATE sessions SET message_count = message_count + 1, updated = ? WHERE id = ?").run(Date.now() / 1000, sessionId);
	}

	messages(sessionId: string): { role: string; text: string; timestamp: number; tool_name?: string; tool_call_id?: string; args?: unknown }[] {
		return this.db
			.prepare("SELECT role, text, ts, tool_name, tool_call_id, args FROM messages WHERE session_id = ? ORDER BY rowid")
			.all(sessionId)
			.map((r: any) => ({ role: r.role, text: r.text, timestamp: r.ts, tool_name: r.tool_name ?? undefined, tool_call_id: r.tool_call_id ?? undefined, args: r.args ? JSON.parse(r.args) : undefined }));
	}

	/** Drops the index's copy from the given time on (a regenerated turn). */
	truncateMessages(sessionId: string, fromTs: number): void {
		this.db.prepare("DELETE FROM messages WHERE session_id = ? AND ts >= ?").run(sessionId, fromTs);
	}

	/** Full-text-ish search: every word must appear (case-insensitive), newest sessions first. */
	search(query: string, limit = 30, profile?: string): { session_id: string; snippet: string; role: string; ts: number }[] {
		const words = query.toLowerCase().split(/\s+/).filter(Boolean);
		if (!words.length) return [];
		const cond = words.map(() => "lower(m.text) LIKE ?").join(" AND ");
		const rows = this.db
			.prepare(
				`SELECT m.session_id, m.text, m.role, m.ts FROM messages m JOIN sessions s ON s.id = m.session_id
				 WHERE s.deleted = 0 ${profile ? "AND s.profile = ?" : ""} AND m.role IN ('user','assistant') AND ${cond}
				 ORDER BY m.ts DESC LIMIT ?`,
			)
			.all(...(profile ? [profile] : []), ...words.map((w) => `%${w}%`), limit * 4) as any[];
		const seen = new Set<string>();
		const out = [];
		for (const r of rows) {
			if (seen.has(r.session_id)) continue;
			seen.add(r.session_id);
			const at = r.text.toLowerCase().indexOf(words[0]);
			const start = Math.max(0, at - 60);
			const snippet = (start ? "…" : "") + r.text.slice(start, at) + ">>>" + r.text.slice(at, at + words[0].length) + "<<<" + r.text.slice(at + words[0].length, at + 120);
			out.push({ session_id: r.session_id, snippet, role: r.role, ts: r.ts });
			if (out.length >= limit) break;
		}
		return out;
	}
}
