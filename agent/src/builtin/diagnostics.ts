// The `diagnostics` tool: what the project's language servers say (its lsp.json), asked for. After
// edit and write the same come with the result by themselves (`extensions/assistant.ts`); beside
// `navigate.ts`, the other tool that asks them.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { host } from "../host.ts";
import { diagnosticsFor, knownDiagnostics } from "../lsp/lsp.ts";
import { placeOf } from "./navigate.ts";

/** What the project's language servers say: about one file, or everything they have seen. */
export const diagnosticsTool = defineTool({
	name: "diagnostics",
	description:
		"Errors and warnings from the project's language servers (its lsp.json): for one file, checked now, or for every file they have seen. After edit and write they come with the result by themselves.",
	parameters: Type.Object({ path: Type.Optional(Type.String({ description: "A file; empty: every file seen" })) }),
	async execute(args: any, api: any) {
		const info = host().thread(api.conversationId);
		if (!info) throw new Error("no session");
		if (!args.path) return { output: [{ type: "text" as const, text: knownDiagnostics(info.agent) }] };
		const at = await placeOf(info, String(args.path));
		if (!at) throw new Error(`cannot read ${args.path}`);
		const d = await diagnosticsFor(info.agent, info.cwd, at.machine, at.file, at.text);
		return { output: [{ type: "text" as const, text: d ?? "No language server of this project knows this kind of file (its lsp.json)." }] };
	},
});
