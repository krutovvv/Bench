// Server accesses in bench.sqlite: SSH, SFTP, FTP and FTPS, with their passwords (the person
// chose to keep them in plain text). Imported once from ~/.ssh/config and Transmit's favorites
// (their passwords live in the keychain and come separately, when the person approves macOS'
// prompts). Bench's Servers mode shows them: SSH with a terminal, the others as a file browser.

import { execFileSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { db } from "../config.ts";

export interface Host {
	id: string;
	name: string;
	kind: "ssh" | "sftp" | "ftp" | "ftps";
	host: string;
	port: number | null;
	user: string;
	password: string | null;
	/** A private key file (SSH/SFTP). */
	key_path: string | null;
	/** Where to open (remote folder). */
	path: string | null;
	/** Folder in the list, e.g. a client. */
	folder: string;
	notes: string;
	/** "ssh_config" | "transmit:<id>" | null (made in Bench). */
	source: string | null;
	created: number;
}

function table() {
	db().exec(`CREATE TABLE IF NOT EXISTS hosts (
		id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, host TEXT NOT NULL, port INTEGER, user TEXT DEFAULT '',
		password TEXT, key_path TEXT, path TEXT, folder TEXT DEFAULT '', notes TEXT DEFAULT '', source TEXT UNIQUE, created INTEGER
	)`);
}

const row = (r: any): Host => r;

export const hosts = {
	list(): Host[] {
		table();
		return (db().prepare("SELECT * FROM hosts ORDER BY folder, name").all() as any[]).map(row);
	},
	get(id: string): Host | undefined {
		table();
		return (db().prepare("SELECT * FROM hosts WHERE id = ?").get(id) as any) ?? undefined;
	},
	save(h: Partial<Host> & { name: string; kind: Host["kind"]; host: string }): Host {
		table();
		const full: Host = { id: h.id || randomBytes(6).toString("hex"), name: h.name, kind: h.kind, host: h.host, port: h.port ?? null, user: h.user ?? "", password: h.password ?? null, key_path: h.key_path ?? null, path: h.path ?? null, folder: h.folder ?? "", notes: h.notes ?? "", source: h.source ?? null, created: h.created ?? Math.floor(Date.now() / 1000) };
		db()
			.prepare(
				`INSERT INTO hosts (id, name, kind, host, port, user, password, key_path, path, folder, notes, source, created) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)
				 ON CONFLICT(id) DO UPDATE SET name=excluded.name, kind=excluded.kind, host=excluded.host, port=excluded.port, user=excluded.user,
				 password=excluded.password, key_path=excluded.key_path, path=excluded.path, folder=excluded.folder, notes=excluded.notes`,
			)
			.run(full.id, full.name, full.kind, full.host, full.port, full.user, full.password, full.key_path, full.path, full.folder, full.notes, full.source, full.created);
		return full;
	},
	remove(id: string): void {
		table();
		db().prepare("DELETE FROM hosts WHERE id = ?").run(id);
	},
	bySource(source: string): Host | undefined {
		table();
		return (db().prepare("SELECT * FROM hosts WHERE source = ?").get(source) as any) ?? undefined;
	},
};

/** `Host` blocks of ~/.ssh/config (no wildcards), as SSH hosts. */
export function importSshConfig(): number {
	const file = path.join(os.homedir(), ".ssh", "config");
	if (!fs.existsSync(file)) return 0;
	let n = 0;
	let cur: Record<string, string> | null = null;
	const flush = () => {
		if (!cur?.alias || /[*?!]/.test(cur.alias) || hosts.bySource(`ssh_config:${cur.alias}`)) return;
		hosts.save({ name: cur.alias, kind: "ssh", host: cur.hostname ?? cur.alias, port: cur.port ? Number(cur.port) : null, user: cur.user ?? "", key_path: cur.identityfile ?? null, source: `ssh_config:${cur.alias}` });
		n++;
	};
	for (const raw of fs.readFileSync(file, "utf8").split("\n")) {
		const line = raw.trim();
		const m = line.match(/^(\S+)\s+(.+)$/);
		if (!m || line.startsWith("#")) continue;
		const key = m[1].toLowerCase();
		if (key === "host") {
			flush();
			cur = { alias: m[2].split(/\s+/)[0] };
		} else if (cur) cur[key] = m[2].trim();
	}
	flush();
	return n;
}

/** Transmit's favorites (~/Library/Application Support/Transmit/Metadata/*.favoriteMetadata), without passwords. */
export function importTransmit(): number {
	const dir = path.join(os.homedir(), "Library", "Application Support", "Transmit", "Metadata");
	if (!fs.existsSync(dir)) return 0;
	let n = 0;
	for (const f of fs.readdirSync(dir).filter((f) => f.endsWith(".favoriteMetadata"))) {
		try {
			const meta = JSON.parse(execFileSync("plutil", ["-convert", "json", "-o", "-", path.join(dir, f)], { encoding: "utf8" }));
			const id = meta.com_panic_transmit_uniqueIdentifier ?? path.basename(f, ".favoriteMetadata");
			if (hosts.bySource(`transmit:${id}`)) continue;
			const proto = String(meta.com_panic_transmit_protocol ?? "SFTP").toLowerCase();
			const kind = (["sftp", "ftp", "ftps"].includes(proto) ? proto : proto === "ftp with implicit ssl" || proto === "ftp-tls" ? "ftps" : "sftp") as Host["kind"];
			const server = String(meta.com_panic_transmit_server ?? "");
			if (!server) continue;
			hosts.save({ name: String(meta.com_panic_transmit_nickname ?? server), kind, host: server, user: String(meta.com_panic_transmit_username ?? ""), folder: "Transmit", source: `transmit:${id}` });
			n++;
		} catch {}
	}
	return n;
}
