// The assistant: Bench's default extension. Pi's own coding tools (read, write, edit, bash) and
// tool search, plus the person-facing tools (clarify, todo, memory, skills, cron, delegate,
// session_search) and the inserts' tools (tasks, ssh_run, remote_files, hosts). Its prompt is a
// few short sections (src/prompt.ts). An agent can select other extensions next to it or instead
// of it (its settings.json "extensions").

import fs from "node:fs";
import path from "node:path";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { Type } from "@earendil-works/pi-ai";
import { defineExtension, defineTool, hook, section, ToolTask } from "@earendil-works/pi-durable";
import { createCodingTools } from "@earendil-works/pi-durable/tools";
import { approvalsMode } from "../config.ts";
import { clarify, sessionSearch, todo } from "../builtin/ask.ts";
import { CronTask, cronTool } from "../builtin/cron.ts";
import { memoryTool } from "../builtin/memory.ts";
import { skillManage, skillView } from "../builtin/skills.ts";
import { delegate } from "../builtin/subagent.ts";
import { compactWithoutReasoning } from "../builtin/compaction.ts";
import { remoteFiles } from "../builtin/remote.ts";
import { sshRun } from "../builtin/ssh.ts";
import { hostsTool } from "../builtin/hosts.ts";
import { tasksTool } from "../builtin/tasks.ts";
import { host } from "../host.ts";
import { environment, inserted, notes, projectContext, skills, soul, subject } from "../prompt.ts";
import { Inserts } from "../inserts.ts";

const of = (input: any) => host().session(input.conversationId);
const s = (render: (x: NonNullable<ReturnType<typeof of>>) => string | undefined) => (input: any) => {
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
import { projects } from "../builtin/projects.ts";
import { diagnosticsFor, knownDiagnostics } from "../lsp.ts";
import { machineEnv } from "../ssh-env.ts";

/** A file a tool named, where it is and what it holds now (on the project's machine, if any). */
async function placeOf(info: { cwd: string; project?: string }, p: string): Promise<{ file: string; machine?: string; text: string } | undefined> {
	if (!p) return undefined;
	const machine = info.project ? (projects.get(info.project)?.machine ?? undefined) : undefined;
	if (machine) {
		const env = await machineEnv(machine, info.cwd);
		const file = path.posix.resolve(env.cwd, p);
		const r = await env.readTextFile(file, BACKGROUND_CONTEXT);
		return r.ok ? { file, machine, text: r.value } : undefined;
	}
	const file = path.resolve(info.cwd, p);
	try {
		return { file, text: fs.readFileSync(file, "utf8") };
	} catch {
		return undefined;
	}
}

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

const Coding = createCodingTools();

export const assistant = defineExtension({
	name: "assistant",
	tools: [...(Coding.tools ?? []), grepTool, globTool, lsTool, diagnosticsTool, clarify, todo, memoryTool, skillView, skillManage, cronTool, delegate, sessionSearch, tasksTool, sshRun, remoteFiles, hostsTool],
	tasks: [CronTask],
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
			beforeTool: async (call: any) => {
				if (call.name !== "bash") return undefined;
				const info = host().session(call.conversationId ?? call.conversation?.id);
				if (!info || approvalsMode(info.profile) !== "manual") return undefined;
				const command = String(call.arguments?.command ?? call.args?.command ?? "");
				const hit = DANGEROUS.find(([re]) => re.test(command));
				if (!hit) return undefined;
				const choice = await host().approve(info.id, command, hit[1]);
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
