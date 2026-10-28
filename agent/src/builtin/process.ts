// Background processes (docs/thread-objects.md): a command the agent leaves running — a dev
// server, a watcher, a long build — while the run goes on or ends. Pi's `exec` waits for its
// command, so a process is ours in Pi's terms: a background task of the conversation
// (`bench.process`) whose one phase runs the command in the conversation's environment, keeps the
// tail of its output in its checkpoint and commits its exit as the outcome. Bench reads the tasks'
// records from Pi's commits, so the card in the thread is Pi's state, never polled.
//
// - `background`: "Stop" of a run and the run's end leave it alone; only its own stop ends it.
// - `abandonOnRestart`: when the agent restarts, the command dies with it and is not started
//   again; Pi marks the task with reason `restart` and its `abort` says so.

import { Type } from "@earendil-works/pi-ai";
import { BACKGROUND_CONTEXT, withAbortSignal } from "@earendil-works/chord/context";
import { defineTask, defineTool, type ConversationId } from "@earendil-works/pi-durable";
import { host } from "../host.ts";
import type { Process } from "../protocol.gen.ts";
import type { Runtime } from "../runtime.ts";

export const PROCESS_KIND = "bench.process";

/** The tail of the output a process keeps: what the card and the agent's `log` show. */
const TAIL_LINES = 200;
const TAIL_BYTES = 16 * 1024;
/** How often a running process commits its new output. */
const COMMIT_MS = 1000;

interface Input {
	command: string;
	/** The call that started it: its place in the thread. */
	call: string;
}
type Checkpoint = { phase: "run"; tail: string };
interface Result {
	exit: number | null;
	tail: string;
}

/** `text` with `more` appended, cut to the kept tail. */
export function keepTail(text: string, more: string): string {
	let t = text + more;
	if (t.length > TAIL_BYTES) t = t.slice(t.length - TAIL_BYTES);
	const lines = t.split("\n");
	return lines.length > TAIL_LINES ? lines.slice(lines.length - TAIL_LINES).join("\n") : t;
}

export const ProcessTask = defineTask<Input, Checkpoint, Result>({
	name: PROCESS_KIND,
	version: 1,
	initial: () => ({ phase: "run", tail: "" }),
	phases: {
		run: async (task, rt, context) => {
			const env = await rt.env(context);
			if (!env) throw new Error("this thread has no machine to run on");
			let tail = task.state.checkpoint.tail;
			let committed = tail;
			let disarm: (() => void) | undefined;
			// New output is committed at most once a second: the checkpoint is the card's tail.
			const flush = () => {
				disarm = undefined;
				if (tail === committed || rt.signal.aborted) return;
				committed = tail;
				const at = tail;
				rt.commit(() => ({ status: "running", checkpoint: { phase: "run", tail: at } }), context).catch(() => {});
			};
			const r = await env.exec(
				task.input.command,
				{
					cwd: env.cwd,
					inheritEnv: true,
					onOutput: (text) => {
						tail = keepTail(tail, text);
						disarm ??= rt.setTimer(flush, COMMIT_MS);
					},
				},
				// The invocation's own signal: a stop, and the agent closing, kill the command with
				// it, so it never outlives the agent that ran it.
				withAbortSignal(rt.signal, context),
			);
			disarm?.();
			// Stopped or the agent closing: the abort (now, or after the restart) says how it ended.
			if (rt.signal.aborted) return;
			if (!r.ok) {
				await rt.commit(() => ({ status: "terminal", outcome: { status: "failed", error: { message: String(r.error.message ?? r.error.code) }, result: { exit: null, tail } } }), context);
				return;
			}
			await rt.commit(() => ({ status: "terminal", outcome: { status: "completed", result: { exit: r.value.exitCode, tail } } }), context);
		},
	},
	abort: async (task, rt, context) => {
		const tail = task.state.checkpoint?.tail ?? "";
		const reason = task.abortReason === "restart" ? "restart" : "stopped";
		await rt.commit(() => ({ status: "terminal", outcome: { status: "aborted", reason, result: { exit: null, tail } } }), context);
	},
});

/** A process as Bench and the agent see it, from its Pi task record (`Process` in crates/protocol). */
export function processOf(rec: any): import("../protocol.gen.ts").Process | undefined {
	if (rec?.kind !== PROCESS_KIND) return undefined;
	const live = rec.state?.status !== "terminal" && rec.state?.status !== "completing";
	const outcome = live ? undefined : rec.state.outcome;
	const state = live ? "running" : outcome?.status === "completed" ? "exited" : outcome?.status === "aborted" ? (outcome.reason === "restart" ? "restarted" : "stopped") : "failed";
	return {
		id: String(rec.id),
		call_id: String(rec.input?.call ?? ""),
		command: String(rec.input?.command ?? ""),
		state,
		...(typeof outcome?.result?.exit === "number" ? { exit_code: outcome.result.exit } : {}),
		...(typeof rec.startedAt === "number" ? { started: rec.startedAt } : {}),
		...(typeof rec.endedAt === "number" ? { ended: rec.endedAt } : {}),
		tail: String((live ? rec.state?.checkpoint?.tail : outcome?.result?.tail) ?? ""),
		...(outcome?.error?.message ? { error: String(outcome.error.message) } : {}),
	};
}

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const processTool = defineTool({
	name: "process",
	description:
		"Background processes: commands that keep running while you go on — a dev server, a watcher, a long build. " +
		"`start` runs `command` in the background and returns its id at once; `list` shows this thread's processes; " +
		"`log` shows the last 200 lines of one's output; `stop` ends it. They outlive your run and the person's Stop, " +
		"but not a restart of the agent: then they are stopped and not started again. For a command you wait for, use bash.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("start"), Type.Literal("list"), Type.Literal("log"), Type.Literal("stop")]),
		command: Type.Optional(Type.String({ description: "The shell command (start)" })),
		id: Type.Optional(Type.String({ description: "The process id (log, stop)" })),
	}),
	execute: async (args, api, context) => {
		const all = () => host().processes(api.conversationId);
		if (args.action === "start") {
			if (!args.command?.trim()) throw new Error("command is required");
			const id = await api.createTask(ProcessTask as any, { command: args.command, call: api.callId }, { ownership: { kind: "conversation" }, background: true, abandonOnRestart: true }, context);
			return text({ id: String(id), running: true });
		}
		if (args.action === "list") return text({ processes: (await all()).map(({ tail: _, ...p }) => p) });
		const p = (await all()).find((x) => x.id === String(args.id ?? ""));
		if (!p) throw new Error(`no process "${args.id ?? ""}" in this thread`);
		if (args.action === "stop") {
			if (p.state === "running") await host().stopProcess(p.id);
			return text({ id: p.id, stopped: true });
		}
		const { tail, ...rest } = p;
		return { output: [{ type: "text" as const, text: `${JSON.stringify(rest)}\n${tail || "(no output yet)"}` }] };
	},
});

/** The runtime's background processes, read from Pi's task records. */
export class Processes {
	private rt: Runtime;

	constructor(rt: Runtime) {
		this.rt = rt;
	}

	/** A conversation's background processes, oldest first. */
	async list(conversationId: string | number): Promise<Process[]> {
		const out: Process[] = [];
		let cursor: any;
		do {
			const page = await this.rt.storage.scanTasks({ conversationId: Number(conversationId) as ConversationId, kind: PROCESS_KIND }, 200, cursor, BACKGROUND_CONTEXT);
			for (const rec of page.items) {
				const p = processOf(rec);
				if (p) out.push(p);
			}
			cursor = page.next;
		} while (cursor);
		return out;
	}

	/** Stop a background process: Pi aborts its task, and its `abort` commits how it ended. */
	async stop(id: string): Promise<void> {
		await this.rt.harness.abortTask(Number(id) as any, BACKGROUND_CONTEXT);
	}
}
