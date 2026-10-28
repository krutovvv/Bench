// `ssh_run`: a command in the thread's terminal ("ssh:local", the insert Bench shows in the
// conversation) — here — or, when the person names one, on a ~/.ssh/config host through the
// system's ssh; without either, on its project's machine.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";
import { execFile } from "node:child_process";
import { threadOf } from "../host.ts";
import { quote } from "../machine/ssh.ts";
import { projects } from "./projects.ts";

const MAX = 30_000;

export const sshRun = defineTool({
	name: "ssh_run",
	description:
		"Run a shell command on the server this thread is about (the terminal above the conversation) and get its output. " +
		"One command per call; for anything that changes the server, say what you will run first.",
	parameters: Type.Object({
		command: Type.String(),
		timeout: Type.Optional(Type.Number({ description: "Seconds (default 120)" })),
		host: Type.Optional(Type.String({ description: "Another ~/.ssh/config host, only when the person names one" })),
	}),
	execute: async (args, api, context) => {
		const s = threadOf(api.conversationId);
		// The terminal inserted in the thread (the last one, when there are several); "ssh:" alone
		// is this machine's.
		const inserted = [...(s.inserts ?? [])].reverse().find((i) => i.key.startsWith("ssh:"))?.key.slice(4);
		const insertedAlias = inserted === undefined ? "" : inserted || "local";
		// Else its project's machine, in its folder there.
		const p = s.project ? projects.get(s.project) : undefined;
		const alias = args.host || insertedAlias || p?.machine || "";
		if (!alias) throw new Error("this thread is not about a server; pass host");
		const command = !args.host && !insertedAlias && p?.folder ? `cd ${quote(p.folder)} && ${args.command}` : args.command;
		const local = alias === "local";
		const [cmd, argv] = local ? ["/bin/zsh", ["-lc", command]] : ["ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", alias, command]];
		const out = await new Promise<{ code: number; text: string }>((resolve) => {
			const child = execFile(cmd, argv as string[], { timeout: (args.timeout ?? 120) * 1000, maxBuffer: 8 * 1024 * 1024, signal: context?.abortSignal }, (err: any, stdout, stderr) => {
				resolve({ code: err ? (typeof err.code === "number" ? err.code : 1) : 0, text: `${stdout}${stderr}` });
			});
			child.stdin?.end();
		});
		const text = out.text.length > MAX ? `[… ${out.text.length - MAX} earlier characters]\n${out.text.slice(-MAX)}` : out.text;
		return { output: [{ type: "text" as const, text: `${text}\n[exit ${out.code} on ${alias}]` }], ...(out.code ? { isError: true } : {}) };
	},
});
