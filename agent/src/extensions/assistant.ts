// The assistant: Bench's default extension. Pi's own coding tools (read, write, edit, bash) and
// tool search, plus the person-facing tools (clarify, todo, memory, skills, cron, delegate,
// session_search) and the inserts' tools (tasks, ssh_run). Its prompt is a
// few short sections (src/prompt.ts). An agent can select other extensions next to it or instead
// of it (its settings.json "extensions").

import { Type } from "@earendil-works/pi-ai";
import { defineExtension, defineTool, hook, section, ToolTask } from "@earendil-works/pi-durable";
import { createNodePhotonImages } from "@earendil-works/pi-durable/images/node";
import { createCodingTools } from "@earendil-works/pi-durable/tools";
import { approvalsMode } from "../config.ts";
import { clarify, sessionSearch, todo } from "../builtin/ask.ts";
import { CronTask, cronTool } from "../builtin/cron.ts";
import { ProcessTask, processTool } from "../builtin/process.ts";
import { memoryTool } from "../builtin/memory.ts";
import { skillManage, skillView } from "../builtin/skills.ts";
import { delegate } from "../builtin/subagent.ts";
import { compactWithoutReasoning } from "../builtin/compaction.ts";
import { sshRun } from "../builtin/ssh.ts";
import { tasksTool } from "../builtin/tasks.ts";
import { host } from "../host.ts";
import { environment, inserted, notes, projectContext, skills, soul, subject } from "../prompt.ts";
import { Inserts } from "../inserts.ts";

const of = (input: any) => host().session(input.conversationId);
const s = (render: (x: NonNullable<ReturnType<typeof of>>) => string | undefined | Promise<string | undefined>) => (input: any) => {
	const info = of(input);
	return info ? render(info) : undefined;
};

/** Commands that ask first when approvals are on (the agent's settings.json "approvals": "manual"). */
const DANGEROUS: [RegExp, string][] = [
	[/\brm\s+(-[a-z]*r[a-z]*f|-[a-z]*f[a-z]*r)\b/i, "recursive delete"],
	[/\bsudo\b/, "runs as root"],
	[/\bgit\s+(push\s+.*--force|reset\s+--hard|clean\s+-[a-z]*f)/i, "rewrites or discards git work"],
	[/\b(mkfs|dd\s+if=|diskutil\s+erase)/i, "writes a disk"],
	[/\b(shutdown|reboot|launchctl\s+unload)\b/i, "stops the system or a service"],
	[/curl[^|]*\|\s*(ba|z)?sh\b/i, "runs a downloaded script"],
];

import { globTool, grepTool, lsTool } from "../builtin/search.ts";
import { createAnchoredEdit, createAnchoredRead } from "../builtin/hashline.ts";
import { lspTool, placeOf } from "../builtin/navigate.ts";
import { diagnosticsFor, knownDiagnostics } from "../lsp.ts";

/** What the project's language servers say: about one file, or everything they have seen. */
const diagnosticsTool = defineTool({
	name: "diagnostics",
	description:
		"Errors and warnings from the project's language servers (its lsp.json): for one file, checked now, or for every file they have seen. After edit and write they come with the result by themselves.",
	parameters: Type.Object({ path: Type.Optional(Type.String({ description: "A file; empty: every file seen" })) }),
	async execute(args: any, api: any) {
		const info = host().session(api.conversationId);
		if (!info) throw new Error("no session");
		if (!args.path) return { output: [{ type: "text" as const, text: knownDiagnostics(info.profile) }] };
		const at = await placeOf(info, String(args.path));
		if (!at) throw new Error(`cannot read ${args.path}`);
		const d = await diagnosticsFor(info.profile, info.cwd, at.machine, at.file, at.text);
		return { output: [{ type: "text" as const, text: d ?? "No language server of this project knows this kind of file (its lsp.json)." }] };
	},
});

// Pictures the read tool shrinks to the model's limits: a shot of a Retina window (2480×1600) is
// over them, and without a processor Pi refuses the file instead of resizing it.
export const images = await createNodePhotonImages();
// Pi's coding tools, with read and edit by line anchors (`builtin/hashline.ts`).
const Coding = createCodingTools({ images });
const coding = (Coding.tools ?? []).map((t: any) => (t.name === "read" ? createAnchoredRead({ images }) : t.name === "edit" ? createAnchoredEdit() : t));

export const assistant = defineExtension({
	name: "assistant",
	tools: [...coding, grepTool, globTool, lsTool, diagnosticsTool, lspTool, clarify, todo, memoryTool, skillView, skillManage, cronTool, delegate, sessionSearch, tasksTool, sshRun, processTool],
	tasks: [CronTask, ProcessTask],
	sections: [
		section("soul", s(soul), { tag: false }),
		section("environment", s(environment)),
		section("subject", s(subject), { tag: false }),
		// Read from the thread's own document: what the person inserted.
		section(
			"inserts",
			async (input: any, context: any) => {
				const doc = (await input.read.snapshot(Inserts as any, input.conversationId, context).catch(() => undefined)) as any;
				return inserted(Object.values<any>(doc?.items ?? {}).filter((i) => !i.removed));
			},
			{ tag: false },
		),
		section("project", s(projectContext), { tag: false }),
		section("skills", s(skills), { tag: false }),
		section("notes", s(notes), { tag: false }),
	],
	hooks: [
		compactWithoutReasoning,
		hook(ToolTask, {
			beforeTool: async (call: any, api: any, context: any) => {
				// The person's hold, felt before any next step — the current answer and the step
				// already running are let through; nothing new begins until the hold lifts.
				// The conversation is the hook's `api`'s, not the call's (Pi's ToolCall has no id of it).
				const info = host().session(api.conversationId);
				if (info && !(await host().hold(info.id, context?.abortSignal))) return { block: "The person stopped the run." };
				if (call.name !== "bash" || !info || approvalsMode(info.profile) !== "manual") return undefined;
				const command = String(call.arguments?.command ?? call.args?.command ?? "");
				const hit = DANGEROUS.find(([re]) => re.test(command));
				if (!hit) return undefined;
				const choice = await host().approve(info.id, String(call.id), command, hit[1]);
				return choice === "deny" ? { block: `The person declined this command (${hit[1]}). Do not retry it; ask or choose another way.` } : undefined;
			},
			// The file's errors from the project's language servers, with the edit that made them
			// (`lsp.ts`): the model sees them as an editor shows them, without asking.
			afterTool: async (call: any, result: any, api: any) => {
				if ((call.name !== "edit" && call.name !== "write") || result?.isError) return undefined;
				const info = host().session(api.conversationId);
				if (!info) return undefined;
				const at = await placeOf(info, String(call.arguments?.path ?? ""));
				if (!at) return undefined;
				const diag = await diagnosticsFor(info.profile, info.cwd, at.machine, at.file, at.text).catch(() => undefined);
				return diag ? { ...result, output: [...(result.output ?? []), { type: "text", text: diag }] } : undefined;
			},
		} as any),
	],
} as any);
