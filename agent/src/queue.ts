// The thread's waiting messages: the arithmetic of their order.
//
// Pi's inbox is an array, and a boundary places what it finds **by position**: `applyBoundary`
// takes the first item of each kind it is allowed at that boundary (with the default
// `one-at-a-time` modes, exactly one steer at a tool round, one follow-up when the run answers).
// Position is therefore "what the agent answers next" — the ids only matter in that new ones
// arrive at the end. "Send now" is the one place that has to disagree with arrival order, so it
// is the one place that moves an item.

/**
 * Put the item with `id` first, in place, and answer where it ended up: 0 when it was moved (or
 * already was first), -1 when the queue does not hold it — it was taken back or already placed,
 * and there is nothing to move.
 */
export function moveToFront<T extends { id: unknown }>(items: T[], id: string): number {
	const at = items.findIndex((i) => String(i.id) === id);
	if (at < 0) return -1;
	if (at > 0) items.unshift(...items.splice(at, 1));
	return 0;
}