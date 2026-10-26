// Accesses (SSH, SFTP, FTP, FTPS) and a host's files: /ftp and the project's Accesses.

import { hosts, importSshConfig, importTransmit, type Host } from "../builtin/hosts.ts";
import * as remote from "../builtin/remote.ts";
import { HttpError, required, type Module } from "../http.ts";

/** Runs a file operation on the host of the route; the server's errors come back as 502, without the password. */
async function onHost<T>(m: RegExpMatchArray, fn: (h: Host) => Promise<T>): Promise<T> {
	const h = hosts.get(decodeURIComponent(m[1]));
	if (!h) throw new HttpError(404, "no such host");
	try {
		return await fn(h);
	} catch (e) {
		if (e instanceof HttpError) throw e;
		throw new HttpError(502, remote.remoteError(e, h));
	}
}

export default (): Module => ({
	routes: {
		"GET /api/hosts": () => ({ hosts: hosts.list() }),
		// By `source` when given ("ssh_config:<alias>"): the row already imported is updated, not doubled.
		"POST /api/hosts": (req) => {
			const prev = req.body?.source ? hosts.bySource(String(req.body.source)) : undefined;
			return { host: hosts.save(prev ? { ...prev, ...req.body, id: prev.id } : req.body) };
		},
		"POST /api/hosts/import": () => ({ ssh_config: importSshConfig(), transmit: importTransmit() }),
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/hosts\/([^/]+)$/,
			(req, m) => {
				const prev = hosts.get(decodeURIComponent(m[1]));
				if (!prev) throw new HttpError(404, "no such host");
				return { host: hosts.save({ ...prev, ...req.body, id: prev.id }) };
			},
		],
		// A file host deleted in /ftp or in its project: the row goes, its password with it.
		[
			"DELETE",
			/^\/api\/hosts\/([^/]+)$/,
			(_req, m) => {
				const id = decodeURIComponent(m[1]);
				if (!hosts.get(id)) throw new HttpError(404, "no such host");
				hosts.remove(id);
				return { ok: true };
			},
		],
		["GET", /^\/api\/hosts\/([^/]+)\/ls$/, (req, m) => onHost(m, (h) => remote.ls(h, req.query.get("path")))],
		[
			"GET",
			/^\/api\/hosts\/([^/]+)\/file$/,
			(req, m) =>
				onHost(m, async (h) => {
					const max = Math.min(Number(req.query.get("max") || 5_000_000), 50_000_000);
					const r = await remote.read(h, required(req.query.get("path"), "path"), max);
					return { path: r.path, size: r.size, ...remote.encodeContent(r.data, r.truncated), truncated: r.truncated };
				}),
		],
		[
			"PUT",
			/^\/api\/hosts\/([^/]+)\/file$/,
			(req, m) =>
				onHost(m, async (h) => {
					const b = req.body ?? {};
					if (typeof b.text !== "string" && typeof b.base64 !== "string") throw new HttpError(400, "text or base64 is required");
					await remote.write(h, required(b.path, "path"), typeof b.text === "string" ? Buffer.from(b.text, "utf8") : Buffer.from(b.base64, "base64"));
					return { ok: true };
				}),
		],
		["DELETE", /^\/api\/hosts\/([^/]+)\/file$/, (req, m) => onHost(m, async (h) => (await remote.remove(h, required(req.query.get("path"), "path")), { ok: true }))],
		["POST", /^\/api\/hosts\/([^/]+)\/mkdir$/, (req, m) => onHost(m, async (h) => (await remote.mkdir(h, required(req.body?.path, "path")), { ok: true }))],
		["POST", /^\/api\/hosts\/([^/]+)\/rename$/, (req, m) => onHost(m, async (h) => (await remote.rename(h, required(req.body?.from, "from"), required(req.body?.to, "to")), { ok: true }))],
	],
});
