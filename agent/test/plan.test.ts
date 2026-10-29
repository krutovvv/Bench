// The plan reminder: a plan with open steps left unchanged for several steps is said to the model.
// It reads the turn's plan (`turn/turn.ts`), the one `todo` wrote.

import assert from "node:assert/strict";
import test from "node:test";
import { PLAN_STALE_STEPS, planNudge, seeResult } from "../src/turn/turn.ts";

const todo = (todos: { content: string; status: string }[]) => ({ name: "todo", arguments: { todos: todos.map((t, i) => ({ id: String(i), ...t })) } });
const step = { name: "bash", arguments: { command: "true" } };
const ok = { output: [{ type: "text", text: "ok" }] };
/** A call finished, then the reminder the model reads with its result. */
const after = (c: string, call: { name: string; arguments?: any }) => {
	seeResult(c, call, ok);
	return call.name === "todo" ? undefined : planNudge(c);
};

test("a plan left unchanged is recalled every few steps, with its current step", () => {
	assert.equal(after("c1", todo([{ content: "Write the game", status: "in_progress" }, { content: "Test it", status: "pending" }])), undefined);
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(after("c1", step), undefined);
	assert.match(after("c1", step)!, /"Write the game"/);
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(after("c1", step), undefined);
	assert.ok(after("c1", step));
});

test("an updated plan starts the count again; a finished one, or none, is never recalled", () => {
	after("c2", todo([{ content: "A", status: "in_progress" }]));
	for (let i = 1; i < PLAN_STALE_STEPS; i++) after("c2", step);
	after("c2", todo([{ content: "A", status: "completed" }, { content: "B", status: "in_progress" }]));
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(after("c2", step), undefined);
	assert.match(after("c2", step)!, /"B"/);
	after("c2", todo([{ content: "A", status: "completed" }, { content: "B", status: "completed" }]));
	for (let i = 0; i < PLAN_STALE_STEPS * 2; i++) assert.equal(after("c2", step), undefined);
	for (let i = 0; i < PLAN_STALE_STEPS * 2; i++) assert.equal(after("c3", step), undefined);
});
