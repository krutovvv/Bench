// Edits by line anchors (builtin/hashline.ts): read shows LINE:HASH|text; edit changes lines by
// those anchors, refuses a stale one with the lines as they are now, and keeps Pi's oldText edit.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { BACKGROUND_CONTEXT as ctx } from "@earendil-works/chord/context";
import { NodeExecutionEnv } from "@earendil-works/pi-durable/env/node";
import { anchored, applyAnchored, createAnchoredEdit, createAnchoredRead, lineHash } from "../src/builtin/hashline.ts";

const at = (lines: string[], n: number) => `${n}:${lineHash(lines[n - 1])}`;

test("edits apply against the lines as read, from the bottom up, and say where the new lines are", () => {
	const lines = ["a", "b", "c", "d", "e"];
	const { lines: out, changed } = applyAnchored(lines, [
		{ start: at(lines, 2), end: at(lines, 3), content: "B\nC\nC2" },
		{ op: "insert_after", start: at(lines, 5), content: "f" },
		{ op: "insert_before", start: at(lines, 1), content: "top" },
		{ start: at(lines, 4), content: "" },
	]);
	assert.deepEqual(out, ["top", "a", "B", "C", "C2", "e", "f"]);
	assert.deepEqual(changed, [[0, 1], [2, 5], [5, 5], [6, 7]]);
	assert.deepEqual(applyAnchored(["x"], [{ op: "insert_after", start: "0", content: "first\n" }]).lines, ["first", "x"], "0 is the top; one trailing newline is not a line");
});

test("a stale anchor changes nothing and shows the lines as they are now", () => {
	const lines = ["one", "two", "three"];
	assert.throws(() => applyAnchored(lines, [{ start: "2:zz", content: "2" }]), (e: any) => {
		assert.match(e.message, /^Nothing was changed: line 2 is not what you read \(2:zz\)/);
		assert.ok(e.message.includes(anchored(["two"], 2)), "the line with its anchor now");
		return true;
	});
	assert.throws(() => applyAnchored(lines, [{ start: at(lines, 1), end: at(lines, 2), content: "x" }, { start: at(lines, 2), content: "y" }]), /touch the same lines/);
	assert.throws(() => applyAnchored(lines, [{ start: "9:aa", content: "x" }]), /past the end/);
	assert.throws(() => applyAnchored(lines, [{ start: at(lines, 1) }]), /has no content/, "a replace without content deletes nothing");
});

test("anchors copied into content come off", () => {
	const lines = ["a", "b"];
	const content = anchored(["B1", "B2"], 2);
	assert.deepEqual(applyAnchored(lines, [{ start: at(lines, 2), content }]).lines, ["a", "B1", "B2"]);
});

test("the tools: read anchors from its offset; edit by anchors keeps CRLF; oldText is Pi's edit", async () => {
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bench-hashline-"));
	const file = path.join(dir, "a.txt");
	fs.writeFileSync(file, "alpha\r\nbeta\r\ngamma\r\n");
	const api: any = { env: new NodeExecutionEnv({ cwd: dir }) };
	const read = createAnchoredRead({});
	const edit = createAnchoredEdit();
	const shown = (await read.execute({ path: "a.txt", offset: 2, limit: 1 }, api, ctx)).output[0].text;
	assert.equal(shown, `2:${lineHash("beta")}|beta`);

	const r: any = await edit.execute({ path: "a.txt", edits: [{ start: `2:${lineHash("beta")}`, content: "BETA" }] }, api, ctx);
	assert.equal(fs.readFileSync(file, "utf8"), "alpha\r\nBETA\r\ngamma\r\n");
	assert.match(r.output[0].text, new RegExp(`^Edited a\\.txt \\(1 change\\)\\. Now:\\n1:..\\|alpha\\n2:${lineHash("BETA")}\\|BETA\\n3:..\\|gamma`));
	assert.match(r.details.patch, /-beta\r?\n\+BETA/);

	// newText beside anchors, as models write it, is the content.
	await edit.execute({ path: "a.txt", edits: [{ start: `1:${lineHash("alpha")}`, newText: "alpha" }] }, api, ctx);
	assert.equal(fs.readFileSync(file, "utf8"), "alpha\r\nBETA\r\ngamma\r\n");
	await edit.execute({ path: "a.txt", edits: [{ oldText: "gamma", newText: "GAMMA" }] }, api, ctx);
	assert.equal(fs.readFileSync(file, "utf8"), "alpha\r\nBETA\r\nGAMMA\r\n");
	await assert.rejects(edit.execute({ path: "a.txt", edits: [{ start: "1:zz", content: "x" }] }, api, ctx), /Nothing was changed/);
	assert.equal(fs.readFileSync(file, "utf8"), "alpha\r\nBETA\r\nGAMMA\r\n", "a refused edit writes nothing");
	fs.rmSync(dir, { recursive: true, force: true });
});
