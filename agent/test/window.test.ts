// What is sent to the model fits its window: a result larger than its share keeps its head and
// tail with a line; a request that would overflow has its largest old blocks cut, in that request
// only, until it fits with room for the answer; the provider's count corrects the estimate.

import assert from "node:assert/strict";
import test from "node:test";
import { CHARS_PER_TOKEN, cutMiddle, fitRequest, fitResult, seeUsage } from "../src/builtin/window.ts";

const W = 131072;
const text = (t: string) => ({ output: [{ type: "text", text: t }] });

test("a result within its share is left alone; a larger one keeps head and tail and says how to ask for less", () => {
	assert.equal(fitResult(text("x".repeat(1000)), W), undefined);
	const big = "HEAD" + "m".repeat(52_000 * 3) + "TAIL";
	const r = fitResult({ ...text(big), details: { a: 1 } }, W);
	const out = r.output[0].text as string;
	assert.ok(out.startsWith("HEAD") && out.endsWith("TAIL"));
	assert.match(out, /characters cut out here: the result is larger than this model takes in one step.*Ask for less/);
	assert.ok(out.length <= W * 0.12 * CHARS_PER_TOKEN + 400, "within its share");
	assert.deepEqual(r.details, { a: 1 }, "the rest of the result kept");
});

test("cutMiddle keeps two thirds from the head and a third from the tail", () => {
	const s = cutMiddle("a".repeat(600) + "b".repeat(400), 300, "why");
	assert.ok(s.startsWith("a".repeat(200)) && s.endsWith("b".repeat(100)));
});

const user = (t: string) => ({ role: "user", content: [{ type: "text", text: t }] });
const call = (args: any) => ({ role: "assistant", content: [{ type: "toolCall", id: "1", name: "write", arguments: args }] });
const result = (t: string) => ({ role: "toolResult", toolCallId: "1", content: [{ type: "text", text: t }] });

test("a request that fits is sent as it is", () => {
	assert.equal(fitRequest("r1", [user("hi"), result("x".repeat(10_000)), user("go on")], W), undefined);
});

test("one that would overflow has its largest old blocks cut, the last message never", () => {
	const page = (c: string) => c.repeat(52_000 * 3);
	const msgs = [user("find prices"), call({ path: "a.html", content: "<html>" + "z".repeat(77_000) }), result(page("1")), result(page("2")), result("small"), user("y".repeat(30_000))];
	const out = fitRequest("r2", msgs, W)!;
	assert.ok(out, "cut");
	const sizes = out.map((m: any) => JSON.stringify(m.content).length);
	const total = Math.ceil(sizes.reduce((a: number, b: number) => a + b, 0) / CHARS_PER_TOKEN);
	assert.ok(total < W * 0.88, `fits with room for the answer (${total})`);
	assert.match(out[2].content[0].text, /kept whole in the thread, cut here/);
	assert.equal(out[4].content[0].text, "small");
	assert.equal(out[5].content[0].text.length, 30_000, "the last message whole");
	assert.equal(msgs[2].content[0].text.length, 156_000, "the journal's messages untouched");
});

test("what the provider counted beyond the estimate makes the next request cut sooner", () => {
	const msgs = [user("a"), result("x".repeat(250_000)), user("b")];
	assert.equal(fitRequest("r3", msgs, W), undefined, "~83K tokens fit");
	// The system prompt and tools' descriptions took 45K more than the messages.
	seeUsage("r3", { input: 83_400 + 45_000 });
	assert.ok(fitRequest("r3", msgs, W), "now it would not fit");
});
