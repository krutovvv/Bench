// What a test hears from a Runtime: its threads' events (the wire, `ThreadEvent`).

import type { ThreadEvent } from "../src/protocol.gen.ts";
import type { Publish } from "../src/runtime/runtime.ts";

/** A Runtime's `publish` that hands each thread event to `fn`, with the thread's id. */
export const listen =
	(fn: (e: ThreadEvent, thread: string) => void): Publish =>
	(ev) => {
		if (ev.type === "thread") fn(ev.event, ev.thread);
	};
