// Scheduled jobs. Each enabled job is a durable Pi task that sleeps until its next run
// (`sleepUntil`), so it survives restarts and a run missed while the server was down happens when
// it starts again. A run is a fresh session of the job's agent (source "cron"); its answer goes
// to `deliver` ("telegram:<chat>"), or stays in that session for Bench.
//
// Schedules: cron expressions ("0 8 * * 1", croner), intervals ("30m", "every 2h"), one-shot
// ISO times ("2026-11-01T09:00").

import { Type } from "@earendil-works/pi-ai";
import { defineTask, defineTool, type Conversation } from "@earendil-works/pi-durable";
import { Cron } from "croner";
import { randomBytes } from "node:crypto";
import { db, renameOld, timezone } from "../config.ts";
import { host, threadOf } from "../host.ts";

export interface Job {
	id: string;
	agent: string;
	name: string;
	prompt: string;
	schedule: string;
	deliver: string;
	enabled: boolean;
	next_run: number | null;
	last_run: number | null;
	last_status: string | null;
	last_output: string | null;
	last_thread: string | null;
	/** The Pi task that sleeps until its next run. */
	task_id: string | null;
	created: number;
}

function table() {
	renameOld("ALTER TABLE cron_jobs RENAME COLUMN profile TO agent");
	renameOld("ALTER TABLE cron_jobs RENAME COLUMN last_session TO last_thread");
	db().exec(`CREATE TABLE IF NOT EXISTS cron_jobs (
		id TEXT PRIMARY KEY, agent TEXT NOT NULL, name TEXT NOT NULL, prompt TEXT NOT NULL, schedule TEXT NOT NULL,
		deliver TEXT DEFAULT '', enabled INTEGER DEFAULT 1, next_run REAL, last_run REAL, last_status TEXT,
		last_output TEXT, last_thread TEXT, task_id TEXT, created REAL
	)`);
}

const row = (r: any): Job => ({ ...r, enabled: Boolean(r.enabled) });

export const jobs = {
	list(agent?: string): Job[] {
		table();
		return (db().prepare(`SELECT * FROM cron_jobs ${agent ? "WHERE agent = ?" : ""} ORDER BY created`).all(...(agent ? [agent] : [])) as any[]).map(row);
	},
	get(id: string): Job | undefined {
		table();
		const r = db().prepare("SELECT * FROM cron_jobs WHERE id = ?").get(id);
		return r ? row(r) : undefined;
	},
	/** By id, or a unique case-insensitive name. */
	resolve(ref: string, agent?: string): Job {
		const all = jobs.list(agent);
		const hit = all.find((j) => j.id === ref) ?? all.filter((j) => j.name.toLowerCase() === ref.toLowerCase());
		if (Array.isArray(hit)) {
			if (hit.length === 1) return hit[0];
			throw new Error(hit.length ? `"${ref}" matches ${hit.length} jobs; use the id` : `no job "${ref}"`);
		}
		return hit;
	},
	save(j: Job): void {
		table();
		db()
			.prepare(
				`INSERT INTO cron_jobs (id, agent, name, prompt, schedule, deliver, enabled, next_run, last_run, last_status, last_output, last_thread, task_id, created)
				 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET agent=excluded.agent, name=excluded.name, prompt=excluded.prompt,
				 schedule=excluded.schedule, deliver=excluded.deliver, enabled=excluded.enabled, next_run=excluded.next_run, last_run=excluded.last_run,
				 last_status=excluded.last_status, last_output=excluded.last_output, last_thread=excluded.last_thread, task_id=excluded.task_id`,
			)
			.run(j.id, j.agent, j.name, j.prompt, j.schedule, j.deliver, j.enabled ? 1 : 0, j.next_run, j.last_run, j.last_status, j.last_output, j.last_thread, j.task_id ?? null, j.created);
	},
	remove(id: string): void {
		table();
		db().prepare("DELETE FROM cron_jobs WHERE id = ?").run(id);
	},
};

const UNITS: Record<string, number> = { s: 1e3, m: 6e4, h: 36e5, d: 864e5, w: 6048e5 };

/** The next run after `after` (ms), or null when a one-shot is past. Throws on a schedule it cannot read. */
export function nextRun(schedule: string, after = Date.now(), lastRun: number | null = null): number | null {
	const s = schedule.trim();
	const every = s.match(/^(?:every\s+)?(\d+)\s*(s|m|h|d|w)\w*$/i);
	if (every) {
		const step = Number(every[1]) * UNITS[every[2].toLowerCase()];
		return Math.max(after, (lastRun ?? after - step) + step);
	}
	if (/^\d{4}-\d{2}-\d{2}/.test(s)) {
		const at = new Date(s).getTime();
		if (Number.isNaN(at)) throw new Error(`cannot read the time "${s}"`);
		return lastRun ? null : at;
	}
	const next = new Cron(s, { timezone: timezone(), paused: true }).nextRun(new Date(after));
	return next ? next.getTime() : null;
}

export function describe(schedule: string): string {
	const every = schedule.trim().match(/^(?:every\s+)?(\d+)\s*(s|m|h|d|w)\w*$/i);
	return every ? `every ${every[1]}${every[2].toLowerCase()}` : schedule.trim();
}

// ---- the durable task --------------------------------------------------------

type Phase = { phase: "wait" } | { phase: "fire"; at: number };

export const CronTask = defineTask<{ jobId: string }, Phase, null>({
	name: "bench.cron",
	version: 1,
	initial: () => ({ phase: "wait" }),
	phases: {
		wait: async (task, runtime, context) => {
			const job = jobs.get(task.input.jobId);
			const at = job?.enabled ? job.next_run : null;
			if (!job || at === null || at === undefined) {
				await runtime.commit(() => ({ status: "terminal", outcome: { status: "completed", result: null } }), context);
				return;
			}
			await runtime.sleepUntil(at, { phase: "fire", at }, context);
		},
		fire: async (task, runtime, context) => {
			const job = jobs.get(task.input.jobId);
			if (job?.enabled) {
				// A run is decided once: after a restart mid-run it is not started a second time.
				const at = task.state.checkpoint.at;
				const done = await runtime.memo<boolean>(`fired:${at}`, context);
				if (!done) {
					await runtime.memo(`fired:${at}`, true, context);
					await runJob(job);
				} else if (job.next_run === at) {
					// The agent restarted while this run worked: it is said so, and the job goes on
					// at its next time instead of waking for this one again.
					settle(job, at, "interrupted by restart");
				}
			}
			const fresh = jobs.get(task.input.jobId);
			const next = fresh?.enabled ? fresh.next_run : null;
			if (!fresh || next === null || next === undefined) {
				await runtime.commit(() => ({ status: "terminal", outcome: { status: "completed", result: null } }), context);
				return;
			}
			await runtime.sleepUntil(next, { phase: "fire", at: next }, context);
		},
	},
	abort: (_task, runtime, context) => runtime.commit(() => ({ status: "terminal", outcome: { status: "aborted" } }), context),
});

export async function runJob(job: Job): Promise<void> {
	const started = Date.now();
	try {
		const r = await host().run(job.agent, job.prompt, { source: "cron", title: `Cron: ${job.name}` });
		job.last_status = "ok";
		job.last_output = r.text;
		job.last_thread = r.threadId;
		// A project's job answers through that project's bot ("telegram:<project>/<chat>").
		const project = job.agent.startsWith("project:") ? job.agent.slice(8) : "";
		const target = project && job.deliver.startsWith("telegram:") && !job.deliver.includes("/") ? `telegram:${project}/${job.deliver.slice(9)}` : job.deliver;
		if (target && r.text.trim()) await host().deliver(target, r.text).catch((e) => (job.last_status = `delivery failed: ${(e as Error).message}`));
	} catch (e) {
		job.last_status = `error: ${(e as Error).message}`;
	}
	settle(job, started, job.last_status ?? "ok");
}

/** A run of the job is over (or lost): when and how it went, and its next time. */
function settle(job: Job, started: number, status: string): void {
	job.last_run = started;
	job.last_status = status;
	try {
		job.next_run = nextRun(job.schedule, Date.now(), started);
	} catch {
		job.next_run = null;
	}
	if (job.next_run === null) job.enabled = false;
	jobs.save({ ...job, ...pick(jobs.get(job.id), ["enabled", "schedule", "prompt", "name", "deliver"]), last_run: job.last_run, last_status: job.last_status, last_output: job.last_output, last_thread: job.last_thread, next_run: job.next_run });
}

const pick = (o: any, keys: string[]) => (o ? Object.fromEntries(keys.map((k) => [k, o[k]])) : {});

// ---- scheduling (the server's side) -----------------------------------------------

interface Scheduler {
	/** The hidden conversation the jobs' tasks belong to. */
	conversation(): Promise<Conversation>;
	abortTask(id: string): Promise<void>;
}
let scheduler: Scheduler | undefined;

export function setScheduler(s: Scheduler): void {
	scheduler = s;
}

/** (Re)starts the job's sleeping task: after it was created, resumed or rescheduled; a paused or removed job's task is stopped. */
export async function arm(job: Job): Promise<void> {
	if (!scheduler) return;
	if (job.task_id) await scheduler.abortTask(job.task_id).catch(() => {});
	const current = jobs.get(job.id);
	if (!current || !job.enabled || job.next_run === null) {
		if (current) jobs.save({ ...current, task_id: null });
		return;
	}
	const conv = await scheduler.conversation();
	const ctx = (await import("@earendil-works/chord/context")).BACKGROUND_CONTEXT;
	const taskId = await conv.commit((tx: any) => tx.createTask(CronTask, { jobId: job.id }, { ownership: { kind: "conversation" }, background: true }), ctx);
	jobs.save({ ...current, task_id: String(taskId) });
}

export function newJob(o: { agent: string; name: string; prompt: string; schedule: string; deliver?: string }): Job {
	const next = nextRun(o.schedule);
	return { id: randomBytes(6).toString("hex"), agent: o.agent, name: o.name, prompt: o.prompt, schedule: o.schedule, deliver: o.deliver ?? "", enabled: next !== null, next_run: next, last_run: null, last_status: null, last_output: null, last_thread: null, task_id: null, created: Date.now() };
}

// ---- the tool --------------------------------------------------------------

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });
const view = (j: Job) => ({ id: j.id, name: j.name, schedule: describe(j.schedule), enabled: j.enabled, next_run: j.next_run ? new Date(j.next_run).toISOString() : null, last_run: j.last_run ? new Date(j.last_run).toISOString() : null, last_status: j.last_status, deliver: j.deliver || "this app" });

export const cronTool = defineTool({
	name: "cron",
	description:
		"Schedule a prompt to run on its own: cron expression ('0 9 * * 1-5'), interval ('30m', 'every 2h') or a one-time ISO time. " +
		"Each run is a fresh session with no memory of this one, so the prompt must be self-contained. " +
		"Its answer is sent to where the job was created (this Telegram chat, or the app). Actions: create, list, update, pause, resume, remove, run.",
	parameters: Type.Object({
		action: Type.Union([Type.Literal("create"), Type.Literal("list"), Type.Literal("update"), Type.Literal("pause"), Type.Literal("resume"), Type.Literal("remove"), Type.Literal("run")]),
		job: Type.Optional(Type.String({ description: "Job id or name (all but create, list)" })),
		name: Type.Optional(Type.String()),
		prompt: Type.Optional(Type.String()),
		schedule: Type.Optional(Type.String()),
	}),
	execute: async (args, api) => {
		const s = threadOf(api.conversationId);
		if (args.action === "list") return text({ jobs: jobs.list(s.agent).map(view) });
		if (args.action === "create") {
			if (!args.prompt || !args.schedule) throw new Error("prompt and schedule are required");
			const chat = s.source === "telegram" ? s.chat_id : undefined;
			const job = newJob({ agent: s.agent, name: args.name || args.prompt.slice(0, 40), prompt: args.prompt, schedule: args.schedule, deliver: chat ? `telegram:${String(chat).split(":")[0]}` : "" });
			jobs.save(job);
			await arm(job);
			return text({ success: true, job: view(job) });
		}
		const job = jobs.resolve(String(args.job ?? ""), s.agent);
		if (args.action === "remove") {
			await arm({ ...job, enabled: false });
			jobs.remove(job.id);
			return text({ success: true, removed: job.id });
		}
		if (args.action === "pause") {
			jobs.save({ ...job, enabled: false });
			await arm({ ...job, enabled: false });
			return text({ success: true, job: view({ ...job, enabled: false }) });
		}
		if (args.action === "run") {
			void runJob(job);
			return text({ success: true, note: "Started; its answer goes where the job delivers." });
		}
		const updated = { ...job, name: args.name ?? job.name, prompt: args.prompt ?? job.prompt, schedule: args.schedule ?? job.schedule, enabled: args.action === "resume" ? true : job.enabled };
		updated.next_run = nextRun(updated.schedule, Date.now(), null);
		jobs.save(updated);
		await arm(updated);
		return text({ success: true, job: view(updated) });
	},
});
