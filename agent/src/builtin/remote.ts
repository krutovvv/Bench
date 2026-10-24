// Files on a server of the `hosts` table: FTP and FTPS through basic-ftp, SFTP (and SSH hosts,
// over their SFTP subsystem) through ssh2-sftp-client. One connection per host stays open for a
// minute after its last use and is reused; operations on a host run one at a time (an FTP
// connection does one command at a time). Bench's file browser uses it over REST, the agent with
// `remote_files` in a thread about a host's files (subject "files:<host id>").

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import fs from "node:fs";
import os from "node:os";
import { posix } from "node:path";
import { Readable, Writable } from "node:stream";
import { sessionOf } from "../host.ts";
import { hosts, type Host } from "./hosts.ts";

export interface Entry {
	name: string;
	type: "dir" | "file" | "link";
	size: number;
	/** ISO time, or null when the server does not say. */
	modified: string | null;
}

/** An open connection, whatever the protocol. Paths are absolute. */
export interface Conn {
	/** The login directory. */
	home: string;
	closed(): boolean;
	list(dir: string): Promise<Entry[]>;
	read(file: string, max: number): Promise<{ size: number; data: Buffer; truncated: boolean }>;
	write(file: string, data: Buffer): Promise<void>;
	remove(p: string): Promise<void>;
	mkdir(dir: string): Promise<void>;
	rename(from: string, to: string): Promise<void>;
	close(): Promise<void> | void;
}

// ---- paths, listings, content ----------------------------------------------

/** `p` on the server: "" is `base` (the host's folder, or the login dir `home`); "~" and "~/…" are under `home`; others relative to `base`. */
export function resolvePath(p: string | null | undefined, base: string | null | undefined, home: string): string {
	const abs = (x: string, from: string) => (x === "~" ? home : x.startsWith("~/") ? posix.join(home, x.slice(2)) : x.startsWith("/") ? posix.normalize(x) : posix.join(from, x));
	const b = base?.trim() ? abs(base.trim(), home) : home;
	const out = p?.trim() ? abs(p.trim(), b) : b;
	return out.length > 1 ? out.replace(/\/+$/, "") || "/" : out || "/";
}

/** Folders first, then by name (natural order, case-insensitive). */
export function sortEntries(entries: Entry[]): Entry[] {
	return [...entries].sort((a, b) => Number(b.type === "dir") - Number(a.type === "dir") || a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
}

/** The UTF-8 text of `data`, or null when it is binary (a NUL byte, or not valid UTF-8). A cut at the end of a truncated read is forgiven. */
export function asText(data: Buffer, truncated = false): string | null {
	if (data.includes(0)) return null;
	const decoder = new TextDecoder("utf-8", { fatal: true });
	for (let cut = 0; cut <= (truncated ? 3 : 0) && cut <= data.length; cut++)
		try {
			return decoder.decode(data.subarray(0, data.length - cut));
		} catch {}
	return null;
}

/** `{ text }` for UTF-8 text, `{ base64 }` otherwise. */
export function encodeContent(data: Buffer, truncated = false): { text: string } | { base64: string } {
	const text = asText(data, truncated);
	return text !== null ? { text } : { base64: data.toString("base64") };
}

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/** A LIST date ("Oct 24 12:30", "Oct 24 2025", "10-24-26 12:30PM") as ISO, or null. */
export function parseListDate(raw: string | undefined, now = new Date()): string | null {
	if (!raw) return null;
	let m = raw.match(/^([A-Za-z]{3})\s+(\d{1,2})\s+(?:(\d{1,2}):(\d{2})|(\d{4}))$/);
	if (m) {
		const month = MONTHS.indexOf(m[1].toLowerCase());
		if (month < 0) return null;
		let year = m[5] ? Number(m[5]) : now.getUTCFullYear();
		const at = (y: number) => Date.UTC(y, month, Number(m![2]), m![3] ? Number(m![3]) : 0, m![4] ? Number(m![4]) : 0);
		if (!m[5] && at(year) > now.getTime() + 86_400_000) year--;
		return new Date(at(year)).toISOString();
	}
	m = raw.match(/^(\d{2})-(\d{2})-(\d{2,4})\s+(\d{1,2}):(\d{2})\s*([AP]M)?$/i);
	if (m) {
		const year = m[3].length === 2 ? 2000 + Number(m[3]) : Number(m[3]);
		const ampm = m[6]?.toUpperCase();
		const hour = ampm ? (Number(m[4]) % 12) + (ampm === "PM" ? 12 : 0) : Number(m[4]);
		return new Date(Date.UTC(year, Number(m[1]) - 1, Number(m[2]), hour, Number(m[5]))).toISOString();
	}
	const t = Date.parse(raw);
	return Number.isNaN(t) ? null : new Date(t).toISOString();
}

// ---- the pool ----------------------------------------------------------------

interface Slot {
	conn?: Conn;
	queue: Promise<unknown>;
	pending: number;
	timer?: ReturnType<typeof setTimeout>;
}

/** Everything that changes a connection: a host edited in Bench gets a new one. */
const fingerprint = (h: Host) => JSON.stringify([h.id, h.kind, h.host, h.port, h.user, h.password, h.key_path]);

export class Pool {
	connect: (h: Host) => Promise<Conn>;
	idleMs: number;
	slots = new Map<string, Slot>();

	constructor(connect: (h: Host) => Promise<Conn>, idleMs = 60_000) {
		this.connect = connect;
		this.idleMs = idleMs;
	}

	/** Runs `fn` on the host's connection, after the operations already queued for it. */
	use<T>(h: Host, fn: (c: Conn) => Promise<T>): Promise<T> {
		const key = fingerprint(h);
		let slot = this.slots.get(key);
		if (!slot) this.slots.set(key, (slot = { queue: Promise.resolve(), pending: 0 }));
		const s = slot;
		s.pending++;
		if (s.timer) clearTimeout(s.timer);
		const run = s.queue.then(async () => {
			try {
				if (s.conn?.closed()) s.conn = undefined;
				s.conn ??= await this.connect(h);
				return await fn(s.conn);
			} finally {
				if (s.conn?.closed()) s.conn = undefined;
				if (--s.pending === 0) {
					s.timer = setTimeout(() => this.drop(key), this.idleMs);
					s.timer.unref?.();
				}
			}
		});
		s.queue = run.catch(() => {});
		return run;
	}

	drop(key: string): void {
		const s = this.slots.get(key);
		if (!s || s.pending) return;
		this.slots.delete(key);
		try {
			void Promise.resolve(s.conn?.close()).catch(() => {});
		} catch {}
	}

	/** Closes every idle connection now. */
	closeAll(): void {
		for (const key of [...this.slots.keys()]) this.drop(key);
	}
}

// ---- protocols ---------------------------------------------------------------

const expand = (p: string) => (p.startsWith("~/") ? posix.join(os.homedir(), p.slice(2)) : p);

async function connectFtp(h: Host): Promise<Conn> {
	const { Client } = await import("basic-ftp");
	const c = new Client(30_000);
	const implicit = h.kind === "ftps" && h.port === 990;
	await c.access({ host: h.host, port: h.port ?? (implicit ? 990 : 21), user: h.user || "anonymous", password: h.password ?? (h.user ? "" : "anonymous@"), secure: h.kind === "ftps" ? (implicit ? "implicit" : true) : false });
	const home = await c.pwd();
	return {
		home,
		closed: () => c.closed,
		async list(dir) {
			const items = await c.list(dir);
			return items
				.filter((f) => f.name !== "." && f.name !== "..")
				.map((f) => ({ name: f.name, type: f.isDirectory ? "dir" : f.isSymbolicLink ? "link" : "file", size: f.size, modified: f.modifiedAt ? f.modifiedAt.toISOString() : parseListDate(f.rawModifiedAt) }));
		},
		async read(file, max) {
			let size: number | null = null;
			try {
				size = await c.size(file);
			} catch {}
			const chunks: Buffer[] = [];
			let got = 0;
			let truncated = false;
			const sink = new Writable({
				write(chunk: Buffer, _enc, done) {
					if (got < max) chunks.push(chunk.subarray(0, max - got));
					got += chunk.length;
					// The only way to stop an FTP download is to drop the connection; the pool opens a new one.
					if (got > max && !truncated) {
						truncated = true;
						c.close();
					}
					done();
				},
			});
			try {
				await c.downloadTo(sink, file);
			} catch (e) {
				if (!truncated) throw e;
			}
			const data = Buffer.concat(chunks);
			return { size: size ?? (truncated ? got : data.length), data, truncated: truncated || (size !== null && size > data.length) };
		},
		async write(file, data) {
			await c.uploadFrom(Readable.from([data]), file);
		},
		async remove(p) {
			try {
				await c.remove(p);
			} catch (e) {
				// Not a file: a folder, removed only when empty.
				try {
					await c.removeEmptyDir(p);
				} catch {
					throw e;
				}
			}
		},
		async mkdir(dir) {
			await c.ensureDir(dir);
			await c.cd(home);
		},
		async rename(from, to) {
			await c.rename(from, to);
		},
		close: () => c.close(),
	};
}

async function connectSftp(h: Host): Promise<Conn> {
	const { default: SftpClient } = await import("ssh2-sftp-client");
	let dead = false;
	const markDead = () => {
		dead = true;
	};
	const c = new SftpClient("bench", { error: markDead, end: markDead, close: markDead });
	let privateKey: Buffer | undefined;
	if (h.key_path)
		try {
			privateKey = fs.readFileSync(expand(h.key_path));
		} catch {}
	await c.connect({
		host: h.host,
		port: h.port ?? 22,
		username: h.user || os.userInfo().username,
		...(h.password ? { password: h.password } : {}),
		...(privateKey ? { privateKey } : {}),
		...(process.env.SSH_AUTH_SOCK ? { agent: process.env.SSH_AUTH_SOCK } : {}),
		readyTimeout: 20_000,
	});
	const home = await c.cwd();
	return {
		home,
		closed: () => dead,
		async list(dir) {
			const items: any[] = await c.list(dir);
			return items
				.filter((f) => f.name !== "." && f.name !== "..")
				.map((f) => ({ name: f.name, type: f.type === "d" ? "dir" : f.type === "l" ? "link" : "file", size: Number(f.size ?? 0), modified: f.modifyTime ? new Date(f.modifyTime).toISOString() : null }));
		},
		async read(file, max) {
			const st: any = await c.stat(file);
			if (st.isDirectory) throw new Error(`${file} is a folder`);
			const size = Number(st.size ?? 0);
			const chunks: Buffer[] = [];
			if (size > 0 && max > 0)
				await new Promise<void>((resolve, reject) => {
					const rs = c.createReadStream(file, { start: 0, end: Math.min(size, max) - 1 });
					rs.on("data", (b: Buffer) => chunks.push(b));
					rs.on("end", () => resolve());
					rs.on("close", () => resolve());
					rs.on("error", reject);
				});
			const data = Buffer.concat(chunks).subarray(0, max);
			return { size, data, truncated: size > data.length };
		},
		async write(file, data) {
			await c.put(data, file);
		},
		async remove(p) {
			const st: any = await c.stat(p);
			if (st.isDirectory) await c.rmdir(p, false);
			else await c.delete(p);
		},
		async mkdir(dir) {
			await c.mkdir(dir, true);
		},
		async rename(from, to) {
			await c.rename(from, to);
		},
		close: async () => {
			dead = true;
			await c.end();
		},
	};
}

export const pool = new Pool((h) => (h.kind === "ftp" || h.kind === "ftps" ? connectFtp(h) : connectSftp(h)));

/** An error's message for the person, without the host's password. */
export function remoteError(e: unknown, h: Host): string {
	let msg = String((e as Error)?.message ?? e);
	if (h.password) msg = msg.split(h.password).join("•••");
	return msg;
}

// ---- operations ----------------------------------------------------------------

const base = (h: Host, c: Conn, p: string | null | undefined) => resolvePath(p, h.path, c.home);

export function ls(h: Host, p: string | null | undefined): Promise<{ path: string; entries: Entry[] }> {
	return pool.use(h, async (c) => {
		const dir = base(h, c, p);
		return { path: dir, entries: sortEntries(await c.list(dir)) };
	});
}

export function read(h: Host, p: string, maxBytes = 5_000_000): Promise<{ path: string; size: number; data: Buffer; truncated: boolean }> {
	return pool.use(h, async (c) => {
		const file = base(h, c, p);
		return { path: file, ...(await c.read(file, Math.max(0, maxBytes))) };
	});
}

export function write(h: Host, p: string, data: Buffer): Promise<{ path: string }> {
	return pool.use(h, async (c) => {
		const file = base(h, c, p);
		await c.write(file, data);
		return { path: file };
	});
}

export function remove(h: Host, p: string): Promise<{ path: string }> {
	return pool.use(h, async (c) => {
		const target = base(h, c, p);
		await c.remove(target);
		return { path: target };
	});
}

export function mkdir(h: Host, p: string): Promise<{ path: string }> {
	return pool.use(h, async (c) => {
		const dir = base(h, c, p);
		await c.mkdir(dir);
		return { path: dir };
	});
}

export function rename(h: Host, from: string, to: string): Promise<{ from: string; to: string }> {
	return pool.use(h, async (c) => {
		const [a, b] = [base(h, c, from), base(h, c, to)];
		await c.rename(a, b);
		return { from: a, to: b };
	});
}

// ---- the tool ------------------------------------------------------------------

/** A host by id or name (case-insensitive), or the one the thread is about. */
export function hostFor(name: string | undefined, subject: string | undefined): Host {
	if (name) {
		const all = hosts.list();
		const h = hosts.get(name) ?? all.find((x) => x.name.toLowerCase() === name.toLowerCase());
		if (!h) throw new Error(`no host "${name}"; known: ${all.map((x) => x.name).join(", ") || "none"}`);
		return h;
	}
	if (subject?.startsWith("files:")) {
		const h = hosts.get(subject.slice(6));
		if (!h) throw new Error("the host of this thread no longer exists");
		return h;
	}
	if (subject?.startsWith("ssh:") && subject !== "ssh:local") {
		const alias = subject.slice(4);
		const h = hosts.list().find((x) => x.kind === "ssh" && x.name === alias);
		if (h) return h;
	}
	throw new Error("this thread is not about a server's files; pass host");
}

const human = (n: number) => (n < 1024 ? `${n} B` : n < 1024 ** 2 ? `${(n / 1024).toFixed(1)} KB` : n < 1024 ** 3 ? `${(n / 1024 ** 2).toFixed(1)} MB` : `${(n / 1024 ** 3).toFixed(1)} GB`);
const out = (text: string) => ({ output: [{ type: "text" as const, text }] });

export const remoteFiles = defineTool({
	name: "remote_files",
	description:
		"Files on the server this thread is about (FTP, FTPS or SFTP; the file browser above the conversation). " +
		"ls a folder, read a file (text; max bytes, default 200000), write text to a file (replaces it), delete a file or an empty folder, mkdir (with parents), rename / move (path → to). " +
		"Paths are absolute or relative to the host's folder. Say what you will change before write, delete or rename.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("ls"), Type.Literal("read"), Type.Literal("write"), Type.Literal("delete"), Type.Literal("mkdir"), Type.Literal("rename")]),
		path: Type.Optional(Type.String({ description: "Default: the host's folder" })),
		to: Type.Optional(Type.String({ description: "rename: the new path" })),
		text: Type.Optional(Type.String({ description: "write: the file's whole content" })),
		max: Type.Optional(Type.Number({ description: "read: bytes (default 200000)" })),
		host: Type.Optional(Type.String({ description: "Another host by name or id, only when the person names one" })),
	}),
	execute: async (args, api) => {
		const h = hostFor(args.host, sessionOf(api.conversationId).subject);
		try {
			if (args.action === "ls") {
				const r = await ls(h, args.path);
				const lines = r.entries.map((e) => (e.type === "dir" ? `${e.name}/` : `${e.name}${e.type === "link" ? " @" : ""}  ${human(e.size)}${e.modified ? `  ${e.modified.slice(0, 16).replace("T", " ")}` : ""}`));
				return out(`${r.path} on ${h.name} (${r.entries.length})\n${lines.join("\n")}`);
			}
			if (!args.path?.trim()) throw new Error("path is required");
			if (args.action === "read") {
				const r = await read(h, args.path, args.max ?? 200_000);
				const text = asText(r.data, r.truncated);
				if (text === null) return out(`${r.path} on ${h.name}: a binary file, ${human(r.size)}`);
				return out(`${text}\n[${r.path} on ${h.name}, ${human(r.size)}${r.truncated ? `, first ${human(r.data.length)}` : ""}]`);
			}
			if (args.action === "write") {
				if (args.text === undefined) throw new Error("text is required");
				const r = await write(h, args.path, Buffer.from(args.text, "utf8"));
				return out(`wrote ${r.path} on ${h.name} (${human(Buffer.byteLength(args.text))})`);
			}
			if (args.action === "delete") return out(`deleted ${(await remove(h, args.path)).path} on ${h.name}`);
			if (args.action === "mkdir") return out(`made ${(await mkdir(h, args.path)).path} on ${h.name}`);
			if (!args.to?.trim()) throw new Error("to is required");
			const r = await rename(h, args.path, args.to);
			return out(`moved ${r.from} → ${r.to} on ${h.name}`);
		} catch (e) {
			throw new Error(remoteError(e, h));
		}
	},
});
