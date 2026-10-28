// The runtime as its tools' host (`host.ts`), and where its tools work: the environment of a
// conversation, on this machine or its project's over SSH, kept in its folder when asked.

import type { ExecutionEnv } from "@earendil-works/pi-durable/env";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { projects } from "./builtin/projects.ts";
import { confined } from "./confined-env.ts";
import { host, type Host, type SessionInfo } from "./host.ts";
import { warmUp } from "./lsp.ts";
import { runToCompletion } from "./run.ts";
import type { Runtime } from "./runtime.ts";
import { machineEnv } from "./ssh-env.ts";
import type { SessionRow } from "./store.ts";

/** The runtime as its tools' host. */
export function hostOf(rt: Runtime): Host {
	const info = (row: SessionRow): SessionInfo => ({ id: row.id, profile: row.profile, cwd: row.cwd, source: row.source, chat_id: row.chat_id, project: row.project ?? "", inserts: rt.live.get(row.id)?.inserts ?? [] });
	/** Subagents' conversations act for their parent's session. */
	const adopted = new Map<string, string>();
	return {
		session: (conversationId) => {
			const key = String(conversationId);
			const live = rt.byConversation.get(key);
			if (live) return info(live.row);
			const parent = adopted.get(key);
			const row = parent ? rt.store.get(parent) : rt.store.byConversation(key);
			return row ? info(row) : undefined;
		},
		adopt: (conversationId, sessionId) => void adopted.set(String(conversationId), sessionId),
		// A subagent's call waits on its parent's hold too: the person holds the whole work.
		hold: (sessionId, signal) => rt.holds.wait(sessionId, signal),
		processes: (conversationId) => rt.processes.list(conversationId),
		stopProcess: (id) => rt.processes.stop(id),
		busyElsewhere: (sessionId) => [...rt.live.values()].filter((l) => l.busy && l.row.id !== sessionId).length,
		ask: (sessionId, callId, question, choices, _multiSelect, signal) => rt.asks.question(sessionId, callId, question, choices, signal),
		approve: (sessionId, callId, command, description) => rt.asks.approve(sessionId, callId, command, description),
		report: (sessionId, e) => rt.report(sessionId, e),
		search: (query, limit, profile) =>
			rt.store.search(query, limit, profile).map((h) => ({ session_id: h.session_id, title: rt.store.get(h.session_id)?.title ?? "", snippet: h.snippet, when: new Date(h.ts * 1000).toISOString() })),
		run: async (profile, prompt, opts) => {
			const r = await runToCompletion(rt, profile, prompt, opts);
			return { sessionId: r.sessionId, text: r.text };
		},
		deliver: async (target, text) => {
			const [kind, to] = [target.slice(0, target.indexOf(":")), target.slice(target.indexOf(":") + 1)];
			const send = rt.deliverers.get(kind);
			if (!send) throw new Error(`no way to deliver to ${kind}`);
			await send(to, text);
		},
	};
}

/** The execution environment of a conversation: this machine's, or its project's machine over SSH;
 * kept in its folder when the project says so. */
export async function envFor(conversationId: string, cwd: string | undefined): Promise<ExecutionEnv> {
	const s = host().session(conversationId);
	const p = s?.project ? projects.get(s.project) : undefined;
	// The thread is at work: its project's language servers start loading the project now, so
	// they know it when the agent asks (`lsp` tool, diagnostics after an edit).
	if (s) warmUp(s.profile, s.cwd, p?.machine ?? undefined);
	const env: ExecutionEnv = p?.machine ? await machineEnv(p.machine, cwd || p.folder || "") : new NodeExecutionEnv({ cwd: cwd || process.env.HOME || "/" });
	// Kept in its folder: its tools and commands stay there (`confined-env.ts`).
	return p?.confined && p.folder ? confined(env, env.cwd, !p.machine) : env;
}
