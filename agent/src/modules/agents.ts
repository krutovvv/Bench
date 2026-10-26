// The servers' agents this person works with (`builtin/servers.ts`): Bench's gear lists them, adds
// and removes them; a server's Host project takes its name from here.

import { HttpError, type Module } from "../http.ts";
import { servers } from "../builtin/servers.ts";

export default (): Module => ({
	routes: {
		"GET /api/agents": () => ({ agents: servers.list() }),
		"POST /api/agents": (req) => {
			const host = String(req.body?.host ?? "").trim();
			if (!/^[A-Za-z0-9._-]+$/.test(host)) throw new HttpError(400, "host: an ~/.ssh/config alias");
			return { agent: servers.save({ host, name: String(req.body?.name ?? "") }) };
		},
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/agents\/([^/]+)$/,
			(req, m) => {
				const host = decodeURIComponent(m[1]!);
				if (!servers.list().some((s) => s.host === host)) throw new HttpError(404, "no such agent");
				return { agent: servers.save({ host, name: String(req.body?.name ?? host) }) };
			},
		],
		[
			"DELETE",
			/^\/api\/agents\/([^/]+)$/,
			(_req, m) => {
				servers.remove(decodeURIComponent(m[1]!));
				return { ok: true };
			},
		],
	],
});
