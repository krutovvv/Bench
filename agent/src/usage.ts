// What a thread used: tokens in and out, the size of its context, how fast the model wrote.
// The agent owns these numbers (only it sees the API's usage frames), so it also keeps them:
// Bench's status bar reads the same figures after a restart that it showed while the work ran.

/** The counters of one thread. `input` is the last call's window (≈ the context in use);
 *  `output` and `calls` add up over the thread; `tps` is the last turn's blended rate. */
export interface Totals {
	input: number;
	output: number;
	calls: number;
	cached?: number;
	tps?: number;
	started?: number;
	/** How often Pi compacted the thread (its journal's summaries, `compactionsOf`). */
	compactions?: number;
}

export const EMPTY_TOTALS: Totals = { input: 0, output: 0, calls: 0 };

/** Read stored counters back: anything unreadable is "nothing used yet". */
export function totalsOf(v: unknown): Totals {
	if (!v || typeof v !== "object") return { ...EMPTY_TOTALS };
	const n = (k: string) => {
		const x = (v as any)[k];
		return typeof x === "number" && isFinite(x) && x >= 0 ? x : 0;
	};
	const cached = (v as any).cached;
	const tps = (v as any).tps;
	const compactions = (v as any).compactions;
	return {
		input: n("input"),
		output: n("output"),
		calls: n("calls"),
		...(typeof cached === "number" ? { cached } : {}),
		...(typeof tps === "number" ? { tps } : {}),
		...(typeof compactions === "number" && isFinite(compactions) && compactions >= 0 ? { compactions } : {}),
		...((v as any).started ? { started: (v as any).started } : {}),
	};
}

/** The frame Bench shows: `max` is the model's context window. */
export function usagePayload(model: string, u: Totals, max: number): Record<string, unknown> {
	return {
		model,
		context_used: u.input,
		context_max: max,
		compressions: u.compactions ?? 0,
		cache_hit_pct: u.input ? Math.round(((u.cached ?? 0) / u.input) * 100) : 0,
		input: u.input,
		output: u.output,
		calls: u.calls,
		avg_tps: u.tps ?? 0,
	};
}
/** How fast the model wrote in this thread. Bench measures this (only it sees when each token
 *  of the stream arrived), so it sends it over at the end of an answer and the agent keeps it
 *  with the thread: reopening the thread or the app brings the chart back. */
export interface SpeedStat {
	/** Tokens per second, one sample for each second of the thread's last answer. */
	samples: number[];
	/** The last measured prefill and generation speeds, tokens per second. */
	prefill: number | null;
	generation: number | null;
	/** Characters per token, learned on this thread's model. */
	ratio: number | null;
}

/** How many of the per-second samples to keep (the chart shows the last minute or so). */
export const SPEED_SAMPLES = 180;

/** Read stored speed back: anything unreadable is "nothing measured yet". Bad entries are
 *  dropped rather than trusted — a broken sample would bend the chart and the averages. */
export function speedOf(v: unknown): SpeedStat {
	const o = v && typeof v === "object" ? (v as any) : {};
	const num = (x: unknown): number | null => (typeof x === "number" && isFinite(x) && x > 0 ? x : null);
	const samples = ((Array.isArray(o.samples) ? o.samples : []) as unknown[])
		.map(num)
		.filter((x): x is number => x !== null)
		.slice(-SPEED_SAMPLES);
	return {
		samples,
		prefill: num(o.prefill),
		generation: num(o.generation),
		ratio: typeof o.ratio === "number" && isFinite(o.ratio) && o.ratio > 0 ? o.ratio : null,
	};
}
