// The servers whose agents this person works with, besides this machine's (`src/app/SPEC.md`).
// A list of its own (the setting `agents`: host — an ~/.ssh/config alias — and the name its Host
// project shows), kept by the agent on the person's machine; Bench connects to each. It used to be
// project rows with a `host`: `migrateServerRows` moves those here once.

import { db, getSetting, setSetting } from "../config.ts";

export interface ServerAgent {
	host: string;
	name: string;
}

export const servers = {
	list(): ServerAgent[] {
		const v = getSetting("agents");
		return Array.isArray(v) ? v.filter((a: any) => a?.host).map((a: any) => ({ host: String(a.host), name: String(a.name || a.host) })) : [];
	},
	/** Adds it, or renames it if it is there. */
	save(a: ServerAgent): ServerAgent {
		const list = servers.list().filter((x) => x.host !== a.host);
		const at = servers.list().findIndex((x) => x.host === a.host);
		const entry = { host: a.host, name: a.name.trim() || a.host };
		if (at < 0) list.push(entry);
		else list.splice(at, 0, entry);
		setSetting("agents", list);
		return entry;
	},
	remove(host: string): void {
		setSetting("agents", servers.list().filter((x) => x.host !== host));
	},
};

/** Project rows that stood for a server's agent (`host` set) become the list's entries, in order. */
export function migrateServerRows(): string[] {
	try {
		const rows = db().prepare("SELECT id, name, host FROM projects WHERE host IS NOT NULL AND host != '' ORDER BY created").all() as any[];
		for (const r of rows) {
			if (!servers.list().some((s) => s.host === r.host)) servers.save({ host: r.host, name: r.name });
			db().prepare("DELETE FROM projects WHERE id = ?").run(r.id);
		}
		return rows.map((r) => `${r.name} (${r.host})`);
	} catch {
		return [];
	}
}
