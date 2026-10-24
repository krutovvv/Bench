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
}

function table() {
	db().exec(`CREATE TABLE IF NOT EXISTS projects (
		id TEXT PRIMARY KEY, name TEXT NOT NULL, folder TEXT, created INTEGER
	)`);
}

export const projects = {
	list(): Project[] {
		table();
		return db().prepare("SELECT * FROM projects ORDER BY created").all() as any[];
	},
	get(id: string): Project | undefined {
		table();
		return (db().prepare("SELECT * FROM projects WHERE id = ?").get(id) as any) ?? undefined;
	},
	save(p: Partial<Project> & { name: string }): Project {
		table();
		const full: Project = { id: p.id || randomBytes(6).toString("hex"), name: p.name, folder: p.folder || null, created: p.created ?? Math.floor(Date.now() / 1000) };
		db()
			.prepare("INSERT INTO projects (id, name, folder, created) VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, folder=excluded.folder")
			.run(full.id, full.name, full.folder, full.created);
		return full;
	},
	/** The project goes; its threads stay, without a project. */
	remove(id: string): void {
		table();
		db().prepare("DELETE FROM projects WHERE id = ?").run(id);
		db().prepare("UPDATE sessions SET project = '' WHERE project = ?").run(id);
	},
	/** Where its threads work: its own folder, else the one Bench keeps for it. */
	dir(id: string): string {
		const p = projects.get(id);
		if (p?.folder && fs.existsSync(p.folder)) return p.folder;
		const own = path.join(BENCH_HOME, "projects", id);
		fs.mkdirSync(own, { recursive: true });
		return own;
	},
};
