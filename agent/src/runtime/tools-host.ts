// The runtime as its tools' host (`host.ts`), and where its tools work: the environment of a
// conversation, on this machine or its project's over SSH, kept in its folder when asked; its
// project's settings always here.

import { randomUUID } from "node:crypto";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import type { ExecutionEnv } from "@earendil-works/pi-durable/env";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { projects } from "../builtin/projects.ts";
import { confined } from "../machine/confined.ts";
import { withSettings } from "../machine/settings.ts";
import { host, type Host, type ThreadFacts } from "../host.ts";
import { warmUp } from "../lsp/lsp.ts";
import { runToCompletion } from "./run.ts";
import type { Runtime } from "./runtime.ts";
import { machineEnv } from "../machine/ssh.ts";
import type { ThreadRow } from "../store.ts";

/** The runtime as its tools' host. */
export function hostOf(rt: Runtime): Host {
	const info = (row: ThreadRow): ThreadFacts => ({ id: row.id, agent: row.agent, cwd: row.cwd, source: row.source, chat_id: row.chat_id, project: row.project ?? "", inserts: rt.live.get(row.id)?.inserts ?? [] });
	return {
		thread: (conversationId) => {
			const key = String(conversationId);
			const live = rt.byConversation.get(key);
			if (live) return info(live.row);
			const row = rt.store.byConversation(key);
			return row ? info(row) : undefined;
		},
		hold: (threadId, signal) => rt.holds.wait(threadId, signal),
		processes: (conversationId) => rt.processes.list(conversationId),
		stopProcess: (id) => rt.processes.stop(id),
		ask: (threadId, callId, question, choices, _multiSelect, signal) => rt.asks.question(threadId, callId, question, choices, signal),
		approve: (threadId, callId, command, description) => rt.asks.approve(threadId, callId, command, description),
		report: (threadId, e) => rt.report(threadId, e),
		search: (query, limit, agent) =>
			rt.store.search(query, limit, agent).map((h) => ({ thread_id: h.thread_id, title: rt.store.get(h.thread_id)?.title ?? "", snippet: h.snippet, when: new Date(h.ts * 1000).toISOString() })),
		run: async (agent, prompt, opts) => {
			const r = await runToCompletion(rt, agent, prompt, opts);
			return { threadId: r.threadId, text: r.text };
		},
		deliver: async (target, text) => {
			const [kind, to] = [target.slice(0, target.indexOf(":")), target.slice(target.indexOf(":") + 1)];
			const send = rt.deliverers.get(kind);
			if (!send) throw new Error(`no way to deliver to ${kind}`);
			await send(to, text);
		},
		// Not awaited: the caller is a hook of the very run being stopped.
		stop: (threadId) => void rt.interrupt(threadId).catch(() => undefined),
		write: (conversationId, entry) => {
			const live = rt.byConversation.get(String(conversationId));
			if (live) void live.conversation.submit({ type: "write", entry, requestId: randomUUID() } as any, BACKGROUND_CONTEXT).catch(() => undefined);
		},
		count: (conversationId, what) => {
			const key = String(conversationId);
			const live = rt.byConversation.get(key);
			if (!live) return;
			if (what === "call") live.usage.judge = (live.usage.judge ?? 0) + 1;
			else live.usage.nudges = (live.usage.nudges ?? 0) + 1;
			rt.stream.emitUsage(live);
		},
	};
}

/** The execution environment of a conversation: this machine's, or its project's machine over SSH;
 * kept in its folder when the project says so; its project's settings on this machine. */
export async function envFor(conversationId: string, cwd: string | undefined): Promise<ExecutionEnv> {
	const s = host().thread(conversationId);
	const p = s?.project ? projects.get(s.project) : undefined;
	// The thread is at work: its project's language servers start loading the project now, so
	// they know it when the agent asks (`lsp` tool, diagnostics after an edit).
	if (s) warmUp(s.agent, s.cwd, p?.machine ?? undefined);
	const env: ExecutionEnv = p?.machine ? await machineEnv(p.machine, cwd || p.folder || "") : new NodeExecutionEnv({ cwd: cwd || process.env.HOME || "/" });
	// Kept in its folder: its tools and commands stay there (`machine/confined.ts`).
	const kept = p?.confined && p.folder ? confined(env, env.cwd, !p.machine) : env;
	// Its settings are on this machine, whatever its machine and folder (`machine/settings.ts`).
	if (!p) return kept;
	const dir = projects.env(p.id);
	return withSettings(kept, new NodeExecutionEnv({ cwd: dir }), dir);
}
