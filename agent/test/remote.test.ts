// Remote files without a network: paths, listings, text detection, the connection pool, the
// prompt and the tool's host lookup, on a temporary BENCH_HOME.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-remote-test-"));
process.env.BENCH_HOME = home;
after(() => fs.rmSync(home, { recursive: true, force: true }));

const { resolvePath, sortEntries, asText, encodeContent, parseListDate, Pool, remoteError, hostFor } = await import("../src/builtin/remote.ts");
const { hosts } = await import("../src/builtin/hosts.ts");
const { subject } = await import("../src/prompt.ts");

const host = (over: any = {}) => ({ id: "h1", name: "site", kind: "ftp", host: "example.test", port: null, user: "u", password: "s3cret", key_path: null, path: null, folder: "", notes: "", source: null, created: 0, ...over });

test("paths: the host's folder, the login dir, ~, absolute and relative", () => {
	assert.equal(resolvePath("", null, "/home/u"), "/home/u");
	assert.equal(resolvePath("", "/var/www", "/home/u"), "/var/www");
	assert.equal(resolvePath("", "www", "/home/u"), "/home/u/www");
	assert.equal(resolvePath("css/a.css", "/var/www/", "/home/u"), "/var/www/css/a.css");
	assert.equal(resolvePath("../logs", "/var/www", "/home/u"), "/var/logs");
	assert.equal(resolvePath("/etc//nginx/", "/var/www", "/home/u"), "/etc/nginx");
	assert.equal(resolvePath("~", "/var/www", "/home/u"), "/home/u");
	assert.equal(resolvePath("~/x", "/var/www", "/home/u"), "/home/u/x");
	assert.equal(resolvePath("/", null, "/"), "/");
	assert.equal(resolvePath("..", "/", "/"), "/");
});

test("listings: folders first, then by name, naturally", () => {
	const e = (name: string, type: any) => ({ name, type, size: 0, modified: null });
	const sorted = sortEntries([e("b.txt", "file"), e("img", "dir"), e("A.txt", "file"), e("file10", "file"), e("file2", "file"), e("css", "dir"), e("link", "link")]);
	assert.deepEqual(sorted.map((x) => x.name), ["css", "img", "A.txt", "b.txt", "file2", "file10", "link"]);
});

test("content: UTF-8 text vs binary, a cut multibyte character at the end of a truncated read", () => {
	assert.equal(asText(Buffer.from("привет, world")), "привет, world");
	assert.equal(asText(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00])), null);
	assert.equal(asText(Buffer.from([0xff, 0xfe, 0x41])), null);
	const cut = Buffer.from("да").subarray(0, 3); // "д" and half of "а"
	assert.equal(asText(cut), null);
	assert.equal(asText(cut, true), "д");
	assert.equal(asText(Buffer.alloc(0)), "");
	assert.deepEqual(encodeContent(Buffer.from("hi")), { text: "hi" });
	assert.deepEqual(encodeContent(Buffer.from([0, 1, 2])), { base64: "AAEC" });
});

test("LIST dates: Unix with time or year, DOS", () => {
	const now = new Date("2026-10-24T10:00:00Z");
	assert.equal(parseListDate("Oct 20 12:30", now), "2026-10-20T12:30:00.000Z");
	assert.equal(parseListDate("Dec 31 23:00", now), "2025-12-31T23:00:00.000Z");
	assert.equal(parseListDate("Mar  5  2024", now), "2024-03-05T00:00:00.000Z");
	assert.equal(parseListDate("10-24-26 01:15PM", now), "2026-10-24T13:15:00.000Z");
	assert.equal(parseListDate("10-24-26 12:05AM", now), "2026-10-24T00:05:00.000Z");
	assert.equal(parseListDate("", now), null);
	assert.equal(parseListDate("whenever", now), null);
});

function fakes() {
	const opened: any[] = [];
	const log: string[] = [];
	const connect = async (h: any) => {
		const c: any = {
			home: "/home/u",
			isClosed: false,
			closedCount: 0,
			closed: () => c.isClosed,
			close: () => {
				c.isClosed = true;
				c.closedCount++;
			},
			async list(dir: string) {
				log.push(`start ${dir}`);
				await new Promise((r) => setTimeout(r, 15));
				log.push(`end ${dir}`);
				return [];
			},
		};
		opened.push({ host: h, conn: c });
		return c;
	};
	return { opened, log, connect };
}

test("pool: one connection per host, reused, operations one at a time", async () => {
	const f = fakes();
	const pool = new Pool(f.connect as any, 10_000);
	const h = host();
	await Promise.all([pool.use(h, (c) => c.list("/a")), pool.use(h, (c) => c.list("/b")), pool.use(h, (c) => c.list("/c"))]);
	assert.equal(f.opened.length, 1);
	assert.deepEqual(f.log, ["start /a", "end /a", "start /b", "end /b", "start /c", "end /c"]);
	await pool.use(h, async () => 1);
	assert.equal(f.opened.length, 1);
	// Another host, or this one edited, has its own.
	await pool.use(host({ id: "h2" }), async () => 1);
	await pool.use(host({ password: "new" }), async () => 1);
	assert.equal(f.opened.length, 3);
	pool.closeAll();
	assert.ok(f.opened.every((o) => o.conn.isClosed));
});

test("pool: an idle connection closes, the next operation opens a new one", async () => {
	const f = fakes();
	const pool = new Pool(f.connect as any, 30);
	const h = host();
	await pool.use(h, async () => 1);
	await new Promise((r) => setTimeout(r, 15));
	await pool.use(h, async () => 2); // within the idle time: reused, timer restarts
	assert.equal(f.opened.length, 1);
	await new Promise((r) => setTimeout(r, 60));
	assert.equal(f.opened[0].conn.closedCount, 1);
	assert.equal(pool.slots.size, 0);
	await pool.use(h, async () => 3);
	assert.equal(f.opened.length, 2);
	pool.closeAll();
});

test("pool: a failed operation keeps the queue going; a dropped connection is replaced", async () => {
	const f = fakes();
	const pool = new Pool(f.connect as any, 10_000);
	const h = host();
	await assert.rejects(pool.use(h, async () => {
		throw new Error("550 No such file");
	}), /550/);
	assert.equal(await pool.use(h, async () => "next"), "next");
	assert.equal(f.opened.length, 1);
	await pool.use(h, async (c: any) => c.close()); // e.g. a truncated FTP download
	await pool.use(h, async () => 1);
	assert.equal(f.opened.length, 2);
	pool.closeAll();
});

test("errors never carry the password", () => {
	assert.equal(remoteError(new Error("530 Login incorrect for u:s3cret"), host() as any), "530 Login incorrect for u:•••");
	assert.equal(remoteError(new Error("timeout"), host({ password: null }) as any), "timeout");
});

test("the thread's host and its prompt", () => {
	const h = hosts.save({ name: "Shop", kind: "sftp", host: "shop.test", user: "deploy" });
	hosts.save({ name: "box", kind: "ssh", host: "box.test" });
	assert.equal(hostFor(undefined, `files:${h.id}`).id, h.id);
	assert.equal(hostFor("shop", undefined).id, h.id);
	assert.equal(hostFor(h.id, "tasks:today").id, h.id);
	assert.equal(hostFor(undefined, "ssh:box").name, "box");
	assert.throws(() => hostFor(undefined, "tasks:today"), /pass host/);
	assert.throws(() => hostFor("nope", undefined), /no host "nope"/);
	const text = subject({ id: "s", profile: "default", cwd: home, source: "local", subject: `files:${h.id}` });
	assert.match(text!, /files of Shop \(sftp\).*remote_files/s);
});
