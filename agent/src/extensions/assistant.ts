// The assistant: Bench's default extension. Pi's own coding tools (read, write, edit, bash) and
// tool search, plus the person-facing tools (clarify, todo, memory, skills, cron, delegate,
// session_search) and the inserts' tools (tasks, ssh_run, remote_files, hosts). Its prompt is a
// few short sections (src/prompt.ts). An agent can select other extensions next to it or instead
// of it (its settings.json "extensions").

import { defineExtension, hook, section, ToolTask } from "@earendil-works/pi-durable";
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

const Coding = createCodingTools();

export const assistant = defineExtension({
	name: "assistant",
	tools: [...(Coding.tools ?? []), clarify, todo, memoryTool, skillView, skillManage, cronTool, delegate, sessionSearch, tasksTool, sshRun, remoteFiles, hostsTool],
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
		} as any),
	],
} as any);
