// The /tasks list: one list for every project (a project is a tag), kept in bench.sqlite.

import { tasks } from "../builtin/tasks.ts";
import { HttpError, type Module } from "../http.ts";

export default (): Module => ({
	routes: {
		// Open tasks and the ones done since ?done_since=YYYY-MM-DD.
		"GET /api/tasks": (req) => ({ tasks: tasks.list(req.query.get("done_since") ?? undefined) }),
		"POST /api/tasks": (req) => ({ task: tasks.save(req.body) }),
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/tasks\/([^/]+)$/,
			(req, m) => {
				const id = decodeURIComponent(m[1]);
				const prev = tasks.get(id);
				if (!prev) throw new HttpError(404, "no such task");
				return { task: tasks.save({ ...prev, ...req.body, id }) };
			},
		],
		[
			"DELETE",
			/^\/api\/tasks\/([^/]+)$/,
			(_req, m) => {
				tasks.remove(decodeURIComponent(m[1]));
				return { ok: true };
			},
		],
	],
});
