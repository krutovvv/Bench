// Slash commands typed in a thread: /model, /reasoning, /title, /stop, /retry, /compact, and a
// skill by its name.

import { BACKGROUND_CONTEXT as BACKGROUND } from "@earendil-works/chord/context";
import { readAgent, resolveModel, writeAgent } from "../config.ts";
import { machineOf, readSkill, skillFor } from "../repo.ts";
import type { Ctx, Module } from "../http.ts";
import type { Command, CommandResult } from "../protocol.gen.ts";

const COMMANDS: Command[] = [
	{ name: "/model", description: "Switch the model: /model <name> [--provider <p>]" },
	{ name: "/reasoning", description: "Set reasoning effort: low, medium, high, xhigh" },
	{ name: "/title", description: "Rename this thread" },
	{ name: "/stop", description: "Stop the running turn" },
	{ name: "/retry", description: "Regenerate the last answer" },
	{ name: "/compact", description: "Compress the conversation context" },
];

const output = (text: string): CommandResult => ({ kind: "output", text });

export default ({ rt }: Ctx): Module => {
	async function slash(session_id: string, command: string): Promise<CommandResult> {
		const [name, ...rest] = String(command).trim().split(/\s+/);
		const arg = rest.join(" ");
		const row = session_id ? rt.store.get(session_id) : undefined;
		switch (name) {
			case "/model": {
				const m = arg.match(/^(\S+)(?:\s+--provider\s+(\S+))?/);
				const now = resolveModel();
				if (!m) return output(`Model: ${now.model} (${now.provider})`);
				// The model is one for every thread: /model changes it as the gear does.
				writeAgent("default", { ...readAgent("default"), defaultModel: m[1], defaultProvider: m[2] ?? now.provider });
				return output(`Model set to ${m[1]}${m[2] ? ` (${m[2]})` : ""} for every thread`);
			}
			case "/reasoning":
				if (row && arg) rt.setEffort(session_id, arg);
				return output(`Reasoning effort: ${arg || row?.effort}`);
			case "/title":
				if (row && arg) {
					rt.rename(session_id, arg);
					rt.listChanged();
				}
				return output(`Title: ${arg}`);
			case "/stop":
				await rt.interrupt(session_id);
				return output("Stopped.");
			case "/retry":
				await rt.undo(session_id);
				return output("Last exchange removed; send it again.");
			case "/compact": {
				const live = await rt.session(session_id);
				if (live) await live.conversation.compact(arg || undefined, BACKGROUND);
				return output("Compacting the context.");
			}
			default: {
				// A skill: its body becomes the turn (/skill-name).
				const skill = await skillFor(row?.profile || "default", name.slice(1), row?.cwd || undefined, machineOf(row?.project ?? undefined));
				if (skill) {
					const body = await readSkill(skill);
					return { kind: "send", text: `[SYSTEM: The user has invoked the "${name.slice(1)}" skill, indicating they want you to follow its instructions. The full skill content is loaded below.]\n\n${body}${arg ? `\n\nThe user has provided the following instruction alongside the skill invocation: ${arg}` : ""}` };
				}
				return output(`Unknown command: ${name}`);
			}
		}
	}
	return {
		rpc: {
			// The thread's commands only: skills are found and loaded by the agent itself, not typed.
			"commands.list": () => COMMANDS,
			"thread.command": ({ thread, command }) => slash(thread, command),
		},
	};
};
