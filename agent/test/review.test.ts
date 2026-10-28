// A turn's review (runtime/review.ts): the model's reply read as findings on files under the
// repository, its reasoning and any words around the JSON left out; a reply that is not the
// findings asked for is said to be so, never taken for "nothing found".

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";

const home = fs.mkdtempSync(path.join(os.tmpdir(), "bench-review-"));
process.env.BENCH_HOME = home;
const { parseFindings } = await import("../src/runtime/review.ts");
after(() => fs.rmSync(home, { recursive: true, force: true }));

test("findings: paths under the root, lines kept only when they are lines, empty bodies dropped", () => {
	const out = `<think>hm</think>Here: {"findings": [
		{"path": "./src/a.ts", "lines": "12-14", "body": " Drop phone from submit(). "},
		{"path": "/abs/b.ts", "lines": "near the top", "body": "x"},
		{"path": "c.ts", "body": ""}
	]}`;
	assert.deepEqual(parseFindings("/r/", out), [
		{ path: "/r/src/a.ts", lines: "12-14", body: "Drop phone from submit()." },
		{ path: "/abs/b.ts", body: "x" },
	]);
	assert.deepEqual(parseFindings("/r", '{"findings": []}'), []);
	assert.equal(parseFindings("/r", "Looks good to me."), null);
});
