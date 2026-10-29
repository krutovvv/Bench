// Approvals: with a project's approvals on (its settings.json "approvals": "manual"), a shell
// command that matches one of these patterns asks the person before it runs (`host.approve`, the
// approval card); one they let through is judged by the evaluator too (`judge/risk.ts`). The default
// extension's `beforeTool` asks (`extensions/assistant.ts`).

import { approvalsMode } from "../config.ts";

/** Commands that ask first when approvals are on (the agent's settings.json "approvals": "manual"). */
const DANGEROUS: [RegExp, string][] = [
	[/\brm\s+(-[a-z]*r[a-z]*f|-[a-z]*f[a-z]*r)\b/i, "recursive delete"],
	[/\bsudo\b/, "runs as root"],
	[/\bgit\s+(push\s+.*--force|reset\s+--hard|clean\s+-[a-z]*f)/i, "rewrites or discards git work"],
	[/\b(mkfs|dd\s+if=|diskutil\s+erase)/i, "writes a disk"],
	[/\b(shutdown|reboot|launchctl\s+unload)\b/i, "stops the system or a service"],
	[/curl[^|]*\|\s*(ba|z)?sh\b/i, "runs a downloaded script"],
];

/** A call as approvals see it: a shell command under approvals on (`shell`), its text, and the
 *  pattern's reason when it matches one (`hit`). */
export function approvalOf(agent: string | undefined, call: { name: string; arguments?: any }): { shell: boolean; command: string; hit?: string } {
	const shell = ["bash", "ssh_run"].includes(call.name) && !!agent && approvalsMode(agent) === "manual";
	const command = String(call.arguments?.command ?? "");
	return { shell, command, hit: shell ? DANGEROUS.find(([re]) => re.test(command))?.[1] : undefined };
}
