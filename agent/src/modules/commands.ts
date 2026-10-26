// Slash commands typed in a thread: /model, /reasoning, /title, /stop, /retry, /compact, and a
// skill by its name.

import fs from "node:fs";
import path from "node:path";
import { BACKGROUND_CONTEXT as BACKGROUND } from "@earendil-works/chord/context";
import { readAgent, resolveModel, writeAgent } from "../config.ts";
import { findSkill } from "../builtin/skills.ts";
import type { Ctx, Module } from "../http.ts";

const COMMANDS: [string, string][] = [
	["/new", "Start a new session"],
	["/model", "Switch the model: /model <name> [--provider <p>]"],
	["/reasoning", "Set reasoning effort: low, medium, high, xhigh"],
	["/title", "Rename this session"],
	["/stop", "Stop the running turn"],
	["/retry", "Regenerate the last answer"],
	["/compact", "Compress the conversation context"],
];

export default ({ rt, broadcast }: Ctx): Module => {
	async function slash({ session_id, profile, command }: { session_id: string; profile?: string; command: string }) {
		const [name, ...rest] = String(command).trim().split(/\s+/);
		const arg = rest.join(" ");
		const row = session_id ? rt.store.get(session_id) : undefined;
		switch (name) {
			case "/model": {
				if (!row) return { type: "info", output: "No session." };
				const m = arg.match(/^(\S+)(?:\s+--provider\s+(\S+))?/);
				const now = resolveModel();
				if (!m) return { type: "info", output: `Model: ${now.model} (${now.provider})` };
				// The model is one for every thread: /model changes it as the gear does.
				writeAgent("default", { ...readAgent("default"), defaultModel: m[1], defaultProvider: m[2] ?? now.provider });
				broadcast(session_id, "session.info", { model: m[1] });
				return { type: "info", output: `Model set to ${m[1]}${m[2] ? ` (${m[2]})` : ""} for every thread` };
			}
			case "/reasoning":
				if (row && arg) rt.store.update(session_id, { effort: arg });
				return { type: "info", output: `Reasoning effort: ${arg || row?.effort}` };
			case "/title":
				if (row && arg) rt.store.update(session_id, { title: arg });
				broadcast(session_id, "sessions.changed", {});
				return { type: "info", output: `Title: ${arg}` };
			case "/stop":
				await rt.interrupt(session_id);
				return { type: "info", output: "Stopped." };
			case "/retry":
				await rt.undo(session_id);
				return { type: "info", output: "Last exchange removed; send it again." };
			case "/compact": {
				const live = await rt.session(session_id);
				if (live) await live.conversation.compact(arg || undefined, BACKGROUND);
				return { type: "info", output: "Compacting the context." };
			}
			default: {
				// A skill: its body becomes the turn (/skill-name).
				const skill = findSkill(profile || row?.profile || "default", name.slice(1), row?.cwd || undefined);
				if (skill) {
					const body = fs.readFileSync(path.join(skill.dir, "SKILL.md"), "utf8");
					return { type: "skill", message: `[SYSTEM: The user has invoked the "${name.slice(1)}" skill, indicating they want you to follow its instructions. The full skill content is loaded below.]\n\n${body}${arg ? `\n\nThe user has provided the following instruction alongside the skill invocation: ${arg}` : ""}` };
				}
				return { type: "info", output: `Unknown command: ${name}` };
			}
		}
	}
	return {
		rpc: {
			// The session's commands only: skills are found and loaded by the agent itself, not typed.
			"commands.catalog": () => ({ categories: [{ name: "Session", pairs: COMMANDS.map((c) => [c[0], c[1]]) }], pairs: [] }),
			"slash.exec": async (p) => slash(p),
			"command.dispatch": async ({ session_id, profile, name, arg }) => slash({ session_id, profile, command: `/${String(name).replace(/^\//, "")} ${arg ?? ""}`.trim() }),
		},
	};
};
