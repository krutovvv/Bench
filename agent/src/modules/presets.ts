// The person's presets (`builtin/presets.ts`), read and edited from Bench's gear.

import { HttpError, type Module } from "../http.ts";
import { presets, savePresets, type Presets } from "../builtin/presets.ts";

const KINDS = ["mcp", "lsp", "compaction", "reasoning"];

export default (): Module => ({
	routes: {
		"GET /api/presets": () => presets(),
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/presets\/([a-z]+)$/,
			(req, m) => {
				const kind = m[1]!;
				if (!KINDS.includes(kind)) throw new HttpError(404, `no presets of ${kind}`);
				let value: unknown = null;
				if (!req.body?.reset) {
					try {
						value = JSON.parse(String(req.body?.text ?? ""));
					} catch (e) {
						throw new HttpError(400, `not JSON: ${(e as Error).message}`);
					}
				}
				try {
					savePresets(kind as keyof Presets, value);
				} catch (e) {
					throw new HttpError(400, (e as Error).message);
				}
				return { ok: true, presets: presets() };
			},
		],
	],
});
