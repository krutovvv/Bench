// The plan reminder: a plan with open steps left unchanged for several steps is said to the model.

import assert from "node:assert/strict";
import test from "node:test";
import { PLAN_STALE_STEPS, planNudge } from "../src/builtin/ask.ts";

const todo = (todos: { content: string; status: string }[]) => ({ name: "todo", arguments: { todos } });
const step = { name: "bash", arguments: { command: "true" } };

test("a plan left unchanged is recalled every few steps, with its current step", () => {
	assert.equal(planNudge("c1", todo([{ content: "Write the game", status: "in_progress" }, { content: "Test it", status: "pending" }])), undefined);
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(planNudge("c1", step), undefined);
	assert.match(planNudge("c1", step)!, /"Write the game"/);
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(planNudge("c1", step), undefined);
	assert.ok(planNudge("c1", step));
});

test("an updated plan starts the count again; a finished one, or none, is never recalled", () => {
	planNudge("c2", todo([{ content: "A", status: "in_progress" }]));
	for (let i = 1; i < PLAN_STALE_STEPS; i++) planNudge("c2", step);
	planNudge("c2", todo([{ content: "A", status: "completed" }, { content: "B", status: "in_progress" }]));
	for (let i = 1; i < PLAN_STALE_STEPS; i++) assert.equal(planNudge("c2", step), undefined);
	assert.match(planNudge("c2", step)!, /"B"/);
	planNudge("c2", todo([{ content: "A", status: "completed" }, { content: "B", status: "completed" }]));
	for (let i = 0; i < PLAN_STALE_STEPS * 2; i++) assert.equal(planNudge("c2", step), undefined);
	for (let i = 0; i < PLAN_STALE_STEPS * 2; i++) assert.equal(planNudge("c3", step), undefined);
});
