// Threads: create, open, send, the queue, stop, the answers to the agent's questions, delete.

import { resolveModel } from "../config.ts";
import { gitRoot, HttpError, type Ctx, type Module } from "../http.ts";
import { projects } from "../builtin/projects.ts";
import { speedOf, usagePayload } from "../usage.ts";

export default ({ rt, broadcast }: Ctx): Module => ({
	rpc: {
		"session.list": ({ limit }) => ({
			sessions: rt.store
				// Every thread, whatever agent or project it runs with: the sidebar groups them by project.
				.list({ limit: limit ?? 200 })
				.filter((s) => s.source !== "subagent")
				.map((s) => ({ id: s.id, title: s.title, message_count: s.message_count, model: s.model, running: rt.isBusy(s.id), paused: rt.isPaused(s.id), source: s.source, cwd: s.cwd, subject: s.subject ?? "", project: s.project ?? "", started_at: s.created, last_active: s.updated })),
		}),
		"session.create": async ({ profile, cwd, subject, project }) => {
			// A thread in a project works in the project's folder unless it asked for another.
			const dir = cwd || (project ? projects.dir(String(project)) : undefined);
			// A project's thread runs with the project's environment (general + its own).
			const agent = project ? `project:${project}` : profile || "default";
			const live = await rt.create(agent, dir, "local", { ...(subject ? { subject: String(subject) } : {}), ...(project ? { project: String(project) } : {}) });
			broadcast(live.row.id, "sessions.changed", {});
			return { session_id: live.row.id, stored_session_id: live.row.id, revision: 0, execution_generation: 0 };
		},
		// The person renames a thread (its "⋯" → Rename).
		"session.title": ({ session_id, title }) => {
			rt.rename(String(session_id), String(title ?? ""));
			broadcast(session_id, "sessions.changed", {});
			return { ok: true };
		},
		"session.subject": async ({ session_id, subject }) => {
			rt.setSubject(String(session_id), String(subject ?? ""));
			broadcast(session_id, "sessions.changed", {});
			return { ok: true };
		},
		// The person's inserts (src/inserts.ts): put one where the thread is now, or change one.
		"session.insert": async ({ session_id, id, key, state }) => {
			const inserts = await rt.insert(String(session_id), String(id ?? ""), String(key ?? ""), state && typeof state === "object" ? state : {});
			broadcast(session_id, "session.inserts", { inserts });
			return { inserts };
		},
		"session.insert.patch": async ({ session_id, id, patch }) => {
			const inserts = await rt.patchInsert(String(session_id), String(id ?? ""), patch && typeof patch === "object" ? patch : {});
			broadcast(session_id, "session.inserts", { inserts });
			return { inserts };
		},
		// How fast the model wrote in the answer that ended, as Bench measured it from the stream.
		"session.speed": ({ session_id, speed }) => {
			rt.store.update(String(session_id), { speed: speedOf(speed) });
			return { ok: true };
		},
		// What a thread has besides its journal: questions waiting, settings, numbers, inserts.
		"session.resume": async ({ session_id }) => {
			const live = await rt.session(session_id);
			if (!live) throw new Error(`session not found: ${session_id}`);
			const row = rt.store.get(session_id)!;
			return {
				session_id,
				stored_session_id: session_id,
				revision: 0,
				execution_generation: live.generation,
				running: live.busy,
				inserts: await rt.inserts(session_id),
				prompts: rt.pendingPrompts(session_id),
				info: { reasoning_effort: row.effort, model: row.model, cwd: row.cwd, revision: 0, execution_generation: live.generation },
				usage: usagePayload(row.model, live.usage, resolveModel().contextLength),
				speed: row.speed ?? speedOf(null),
			};
		},
		// The thread as a journal (src/thread.ts): items after `since`, the queue, the inserts' state.
		// Live changes follow as thread.append / thread.stream / thread.queue / thread.inserts / thread.run.
		"thread.open": async ({ session_id, since }) => rt.thread(String(session_id), Number(since) || 0),
		"session.close": async ({ session_id }) => {
			if (!rt.isBusy(session_id)) await rt.close(session_id);
			return { ok: true };
		},
		"session.delete": async ({ session_id }) => {
			await rt.delete(session_id);
			broadcast(session_id, "sessions.changed", {});
			return { ok: true };
		},
		"prompt.submit": async ({ session_id, text, attachments, context, mode }) => {
			await rt.submit(session_id, String(text ?? ""), attachments ?? [], String(context ?? ""), mode === "queue" ? "queue" : "steer");
			return { ok: true };
		},
		// The messages waiting while the agent works (Pi's inbox): take one back, send it now, fix its words.
		"session.queue": async ({ session_id }) => ({ items: await rt.queue(String(session_id)) }),
		"session.queue.withdraw": async ({ session_id, id }) => ({ result: await rt.withdraw(String(session_id), String(id)) }),
		"session.queue.send": async ({ session_id, id }) => {
			await rt.sendNow(String(session_id), String(id));
			return { ok: true };
		},
		"session.queue.edit": async ({ session_id, id, text }) => ({ result: await rt.edit(String(session_id), String(id), String(text ?? "")) }),
		"session.interrupt": async ({ session_id }) => {
			await rt.interrupt(session_id);
			return { ok: true };
		},
		// The person's hold: stand the run at its next step boundary, then let it go on.
		"session.pause": async ({ session_id }) => {
			await rt.pause(String(session_id));
			return { ok: true };
		},
		"session.unpause": async ({ session_id }) => {
			await rt.resume(String(session_id));
			return { ok: true };
		},
		"session.undo": async ({ session_id }) => {
			await rt.undo(session_id);
			return { ok: true };
		},
		"clarify.respond": ({ session_id, prompt_id, request_id, answer }) => {
			rt.respond(session_id, prompt_id ?? request_id, String(answer ?? ""));
			return { ok: true };
		},
		"approval.respond": ({ session_id, prompt_id, request_id, choice }) => {
			rt.respond(session_id, prompt_id ?? request_id, String(choice ?? "deny"));
			return { ok: true };
		},
		// Pi's bash has no background processes.
		"process.list": () => ({ processes: [] }),
		"process.kill": async () => ({ ok: true }),
	},
	routes: {
		"GET /api/sessions": (req) => {
			const source = req.query.get("source") ?? undefined;
			const rows = rt.store.list({ source, limit: Number(req.query.get("limit") ?? 100) }).filter((s) => s.source !== "subagent");
			return { sessions: rows.map((s) => ({ id: s.id, title: s.title, cwd: s.cwd, source: s.source, git_repo_root: gitRoot(s.cwd), git_branch: null, chat_id: s.chat_id, user_id: s.chat_id, display_name: s.display_name })) };
		},
		"GET /api/sessions/search": (req) => ({ results: rt.store.search(req.query.get("q") ?? "", Number(req.query.get("limit") ?? 30)).map((r) => ({ session_id: r.session_id, snippet: r.snippet })) }),
	},
	dynamic: [
		[
			"GET",
			/^\/api\/sessions\/([^/]+)\/mutation-snapshot$/,
			() => {
				throw new HttpError(404, "no mutation snapshots");
			},
		],
		[
			"DELETE",
			/^\/api\/sessions\/([^/]+)$/,
			async (_req, m) => {
				await rt.delete(decodeURIComponent(m[1]));
				broadcast(m[1], "sessions.changed", {});
				return { ok: true };
			},
		],
	],
});
