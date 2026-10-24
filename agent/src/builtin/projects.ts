// Projects: what the person works on, the sidebar's only unit. A project is a row here; a folder
// on disk (a codebase) is optional. Its threads work in that folder, or in a folder Bench keeps
// for it (~/.bench/projects/<id>), so every project has a place for its files either way.

import { randomBytes } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { BENCH_HOME, db } from "../config.ts";

export interface Project {
	id: string;
	name: string;
	/** A folder on disk the project works in (a codebase), or null for a virtual project. */
	folder: string | null;
	created: number;
	/** A server whose agent the project is (an ~/.ssh/config alias of the Mac Bench runs on). */
	host?: string | null;
	/** Its own Telegram bot: { token, allowed_users: [ids], username (once it ran) }. */
	telegram?: { token: string; allowed_users: string[]; username?: string } | null;
}

function table() {
	db().exec(`CREATE TABLE IF NOT EXISTS projects (
		id TEXT PRIMARY KEY, name TEXT NOT NULL, folder TEXT, created INTEGER
	)`);
	try {
		db().exec("ALTER TABLE projects ADD COLUMN telegram TEXT");
	} catch {}
	try {
		db().exec("ALTER TABLE projects ADD COLUMN host TEXT");
	} catch {}
}

const row = (r: any): Project => ({ ...r, telegram: r.telegram ? JSON.parse(r.telegram) : null });

export const projects = {
	list(): Project[] {
		table();
		return (db().prepare("SELECT * FROM projects ORDER BY created").all() as any[]).map(row);
	},
	get(id: string): Project | undefined {
		table();
		const r = db().prepare("SELECT * FROM projects WHERE id = ?").get(id);
		return r ? row(r) : undefined;
	},
	save(p: Partial<Project> & { name: string }): Project {
		table();
		const full: Project = { id: p.id || randomBytes(6).toString("hex"), name: p.name, folder: p.folder || null, created: p.created ?? Math.floor(Date.now() / 1000), telegram: p.telegram ?? null, host: p.host || null };
		db()
			.prepare("INSERT INTO projects (id, name, folder, created, telegram, host) VALUES (?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, folder=excluded.folder, telegram=excluded.telegram, host=excluded.host")
			.run(full.id, full.name, full.folder, full.created, full.telegram ? JSON.stringify(full.telegram) : null, full.host ?? null);
		return full;
	},
	/** The project goes; its threads stay, without a project. */
	remove(id: string): void {
		table();
		db().prepare("DELETE FROM projects WHERE id = ?").run(id);
		db().prepare("UPDATE sessions SET project = '' WHERE project = ?").run(id);
	},
	/** The bot's @name, learned when it started (for the block to show). */
	setBotName(id: string, username: string): void {
		const p = projects.get(id);
		if (p?.telegram && p.telegram.username !== username) projects.save({ ...p, telegram: { ...p.telegram, username } });
	},
	/** Where its threads work: its own folder, else files/ in its environment. */
	dir(id: string): string {
		const p = projects.get(id);
		if (p?.folder && fs.existsSync(p.folder)) return p.folder;
		const own = path.join(projects.env(id), "files");
		fs.mkdirSync(own, { recursive: true });
		return own;
	},
	/** Its environment, an agent directory in Pi's terms: SYSTEM.md (its soul), mcp.json, skills/, prompts/. */
	env(id: string): string {
		const dir = path.join(BENCH_HOME, "projects", id);
		for (const sub of ["skills", "prompts"]) fs.mkdirSync(path.join(dir, sub), { recursive: true });
		return dir;
	},
};
