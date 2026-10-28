// One prompt run to its answer in a fresh thread (scheduled jobs, the tools' `run`).

import type { RunEvent, RunListener, Runtime } from "./runtime.ts";

export async function runToCompletion(
	rt: Runtime,
	settings: string,
	prompt: string,
	opts: { source: string; cwd?: string; title?: string; signal?: AbortSignal; onEvent?: (e: RunEvent) => void },
): Promise<{ sessionId: string; text: string; status: string }> {
	const live = await rt.create(settings, opts.cwd, opts.source, { title: opts.title ?? "" });
	const done = new Promise<{ text: string; status: string }>((resolve) => {
		const listener: RunListener = (sid, e) => {
			if (sid !== live.row.id) return;
			opts.onEvent?.(e);
			if (e.kind === "complete") {
				rt.runs.delete(listener);
				resolve({ text: e.text, status: opts.signal?.aborted ? "interrupted" : "completed" });
			}
		};
		rt.runs.add(listener);
	});
	opts.signal?.addEventListener("abort", () => void rt.interrupt(live.row.id));
	await rt.submit(live.row.id, prompt);
	const r = await done;
	return { sessionId: live.row.id, ...r };
}
