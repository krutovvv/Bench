// The read tool's pictures: a shot larger than the model's limits is resized, not refused.

import assert from "node:assert/strict";
import zlib from "node:zlib";
import { test } from "node:test";
import { images } from "../src/extensions/assistant.ts";

/** A grey PNG of `w`×`h`, the size of a Retina window's shot. */
function png(w: number, h: number): Uint8Array {
	const crc = (b: Buffer) => {
		let c = ~0;
		for (const x of b) {
			c ^= x;
			for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
		}
		return ~c >>> 0;
	};
	const chunk = (type: string, data: Buffer) => {
		const t = Buffer.concat([Buffer.from(type), data]);
		const out = Buffer.alloc(12 + data.length);
		out.writeUInt32BE(data.length, 0);
		t.copy(out, 4);
		out.writeUInt32BE(crc(t), 8 + data.length);
		return out;
	};
	const head = Buffer.alloc(13);
	head.writeUInt32BE(w, 0);
	head.writeUInt32BE(h, 4);
	head[8] = 8; // bit depth
	head[9] = 0; // greyscale
	const rows = Buffer.alloc((w + 1) * h, 0x80);
	for (let y = 0; y < h; y++) rows[y * (w + 1)] = 0;
	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk("IHDR", head),
		chunk("IDAT", zlib.deflateSync(rows)),
		chunk("IEND", Buffer.alloc(0)),
	]);
}

test("a 2480×1600 shot is resized to the model's limits", async () => {
	const out: any = await images.prepare(png(2480, 1600), "image/png", { maxWidth: 2000, maxHeight: 2000, maxBytes: 4_500_000 } as any);
	assert.ok(out, "prepared");
	assert.ok(out.resized, "resized");
	assert.ok(out.resized.to.width <= 2000 && out.resized.to.height <= 2000);
});
