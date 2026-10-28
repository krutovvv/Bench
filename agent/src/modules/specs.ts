// The project's specs for the Explorer's "Specs" (src/specs.ts): every spec of the repository
// around a folder, on its machine, with its parent and references — the tree is drawn from them.

import { required, type Module } from "../http.ts";
import { specsOf } from "../specs.ts";

export default (): Module => ({
	routes: {
		"GET /api/specs": async (req) => specsOf(req.query.get("machine"), required(req.query.get("path"), "path")),
	},
});
