// Threads: list, create, open, send, the queue, stop, pause, fork, the answers to what a run
// asks, inserts, delete — the `thread.*` methods of crates/protocol.

import { gitRoot, type Ctx, type Module } from "../http.ts";
import { projects } from "../builtin/projects.ts";
import { speedOf } from "../usage.ts";

export default ({ rt }: Ctx): Module => ({
	rpc: {
		// Every thread, whatever project it belongs to: the sidebar groups them by project.
		"threads.list": () =>
			rt.store
				.list({ limit: 1000 })
				.filter((s) => s.source !== "subagent")
				.map((s) => {
					const repo = gitRoot(s.cwd);
					return {
						id: s.id,
						title: s.title || "Untitled",
						messages: s.message_count,
						running: rt.isBusy(s.id),
						paused: rt.holds.isPaused(s.id),
						source: s.source,
						updated: s.updated,
						cwd: s.cwd,
						...(s.project ? { project: s.project } : {}),
						...(repo ? { repo } : {}),
						...(s.display_name ? { with: s.display_name } : {}),
					};
				}),
		"thread.create": async ({ project, cwd }) => {
			// A project's thread works in the project's folder unless it asked for another, with the
			// project's environment (general + its own).
			const dir = cwd || (project ? projects.dir(project) : undefined);
			const live = await rt.create(project ? `project:${project}` : "default", dir, "local", project ? { project } : {});
			rt.listChanged();
			return { thread: live.row.id };
		},
		// The thread as a journal (src/thread.ts) and its live run: items after `since`, the queue,
		// the inserts, what it waits for, its numbers. Changes follow as `thread` events.
		"thread.open": ({ thread, since }) => rt.journal.open(thread, since),
		"thread.rename": ({ thread, title }) => {
			rt.titles.rename(thread, title);
			return {};
		},
		"thread.delete": async ({ thread }) => {
			await rt.delete(thread);
			rt.listChanged();
			return {};
		},
		// A new thread from this one's entry `at` (an answer's), Pi's fork: Bench opens it.
		"thread.fork": async ({ thread, at }) => {
			const row = await rt.fork(thread, at);
			rt.listChanged();
			return { thread: row.id };
		},
		// `request`: Bench's id for the message, the one its row on Enter is matched by.
		"thread.send": async ({ thread, text, context, request, queue, effort }) => {
			if (effort !== null) rt.setEffort(thread, effort);
			await rt.submit(thread, text, [], context, queue ? "queue" : "steer", request ?? undefined);
			return {};
		},
		"thread.stop": async ({ thread }) => {
			await rt.interrupt(thread);
			return {};
		},
		// The person's hold: stand the run at its next step boundary, then let it go on.
		"thread.pause": async ({ thread, paused }) => {
			await (paused ? rt.holds.pause(thread) : rt.holds.resume(thread));
			return {};
		},
		// The messages waiting while the agent works (Pi's inbox): take one back, send it now, fix its words.
		"thread.queue.withdraw": async ({ thread, sub }) => {
			await rt.queue.withdraw(thread, sub);
			return {};
		},
		"thread.queue.send": async ({ thread, sub }) => {
			await rt.queue.sendNow(thread, sub);
			return {};
		},
		"thread.queue.edit": async ({ thread, sub, text }) => {
			await rt.queue.edit(thread, sub, text);
			return {};
		},
		// The person's inserts (src/inserts.ts): put one where the thread is now, or change one.
		// Bench sees the result in the thread's `inserts` event.
		"thread.insert": async ({ thread, id, key, state }) => {
			await rt.inserts.put(thread, id, key, state as any);
			return {};
		},
		"thread.insert.patch": async ({ thread, id, patch }) => {
			await rt.inserts.patch(thread, id, patch);
			return {};
		},
		"thread.answer": ({ thread, ask, answer }) => {
			rt.asks.respond(thread, ask, answer);
			return {};
		},
		"thread.effort": ({ thread, effort }) => {
			rt.setEffort(thread, effort);
			return {};
		},
		// How fast the model wrote in the answer that ended, as Bench measured it from the stream.
		"thread.speed": ({ thread, speed }) => {
			rt.store.update(thread, { speed: speedOf(speed) });
			return {};
		},
		// A background process ends by its own stop only (`builtin/process.ts`); Bench sees it end
		// in the thread's `process` event.
		"process.stop": async ({ process }) => {
			await rt.processes.stop(process);
			return {};
		},
	},
	routes: {
		// The threads whose titles or messages have the words: (thread, the snippet around them).
		"GET /api/threads/search": (req) => ({ results: rt.store.search(req.query.get("q") ?? "", Number(req.query.get("limit") ?? 30)).map((r) => ({ thread: r.session_id, snippet: r.snippet })) }),
	},
});
