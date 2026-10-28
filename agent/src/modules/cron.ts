// Scheduled jobs: change, pause, run now, delete. A project's new job is made in projects.ts.

import { arm, describe, jobs, nextRun, runJob, type Job } from "../builtin/cron.ts";
import { HttpError, agentOf, type Module } from "../http.ts";

/** A job as Bench's schedule reads it. */
export function cronView(j: Job) {
	return {
		id: j.id,
		name: j.name,
		prompt: j.prompt,
		schedule: { expr: j.schedule, display: describe(j.schedule) },
		schedule_display: describe(j.schedule),
		enabled: j.enabled,
		state: j.enabled ? "scheduled" : "paused",
		next_run_at: j.next_run ? new Date(j.next_run).toISOString() : null,
		last_run_at: j.last_run ? new Date(j.last_run).toISOString() : null,
		last_status: j.last_status,
		last_output: (j.last_output ?? "").slice(0, 400),
		last_session: j.last_session,
		deliver: j.deliver,
	};
}

export default (): Module => ({
	routes: {
		"GET /api/cron/jobs": (req) => jobs.list(agentOf(req)).map(cronView),
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/cron\/jobs\/([^/]+)$/,
			async (req, m) => {
				const job = jobs.resolve(decodeURIComponent(m[1]));
				const u = req.body?.updates ?? {};
				const schedule = typeof u.schedule === "object" ? u.schedule?.expr : u.schedule;
				const next: Job = { ...job, prompt: u.prompt ?? job.prompt, name: u.name ?? job.name, schedule: schedule ?? job.schedule };
				try {
					next.next_run = nextRun(next.schedule, Date.now(), null);
				} catch (e) {
					throw new HttpError(400, `the schedule: ${(e as Error).message}`);
				}
				jobs.save(next);
				await arm(next);
				return cronView(jobs.get(job.id)!);
			},
		],
		[
			"DELETE",
			/^\/api\/cron\/jobs\/([^/]+)$/,
			async (_req, m) => {
				const job = jobs.resolve(decodeURIComponent(m[1]));
				jobs.remove(job.id);
				// Its sleeping task stops (the job is gone).
				await arm(job);
				return { ok: true };
			},
		],
		[
			"POST",
			/^\/api\/cron\/jobs\/([^/]+)\/(trigger|pause|resume)$/,
			async (_req, m) => {
				const job = jobs.resolve(decodeURIComponent(m[1]));
				if (m[2] === "trigger") void runJob(job);
				else {
					const enabled = m[2] === "resume";
					const next = { ...job, enabled, next_run: enabled ? nextRun(job.schedule, Date.now(), null) : job.next_run };
					jobs.save(next);
					await arm(next);
				}
				return { ok: true };
			},
		],
	],
});
