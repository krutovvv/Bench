// The evaluator at a turn's start: before the model's first request it is asked which of the
// thread's skills fits the person's request, and which of its MCP servers the work will need. A
// sure answer is a line for the model with that request: load this skill, use these servers'
// tools. The model looked for them itself before — a step of `skill_view` or `tool_search` each,
// or none, as when it drove Chrome through rea with the browser server at hand.

import { skillsFor, machineOf } from "../repo.ts";
import { mcpServerTools } from "../mcp/mcp.ts";
import { cut } from "../builtin/turn.ts";
import type { Point } from "./point.ts";

/** How sure the evaluator must be to name a skill, or a server. */
const SURE_SKILL = 0.6;
const SURE_SERVER = 0.75;
/** The most skills and servers offered (a choice holds at most 255 options). */
const MAX_SKILLS = 120;
const MAX_SERVERS = 12;

/** The skills and servers asked about this turn, by question name. */
const offered = new Map<string, { skills: string[]; servers: string[] }>();

export const start: Point = {
	name: "start",
	moment: "request",
	async ask({ conversation, turn, thread }) {
		if (turn.started || !turn.request || turn.steps > 0 || !thread) return undefined;
		turn.started = true;
		const skills = (await skillsFor(thread.agent, thread.cwd, machineOf(thread.project)).catch(() => [])).slice(0, MAX_SKILLS);
		const servers = mcpServerTools(thread.agent).slice(0, MAX_SERVERS);
		if (!skills.length && !servers.length) return undefined;
		offered.set(conversation, { skills: skills.map((k) => k.name), servers: servers.map((s) => s.name) });
		const questions: Record<string, any> = {};
		if (skills.length)
			questions.skill = {
				type: "choice",
				instructions: "Which of these skills (the person's own procedures) fits the person's request best?",
				// "none" first: the model leans to the first option, and naming no skill is the safe side.
				criteria: { none: "None of these fits the request.", ...Object.fromEntries(skills.map((k, i) => [`s${i}`, `${k.name}: ${cut(k.description.replace(/\s+/g, " "), 200)}`])) },
			};
		// A choice of two per server: what it is about stays in the question, short.
		servers.forEach((s, i) => {
			questions[`server${i}`] = {
				type: "choice",
				instructions: `Will the work on the person's request need the tools of the MCP server "${s.name}" (tools: ${cut(s.tools.map((t) => t.replace(/^mcp__[^_]+__/, "")).join(", "), 400)})?`,
				criteria: { not_needed: "The work does not need these tools.", needed: "The work needs these tools." },
			};
		});
		return { questions };
	},
	apply({ conversation }, answers) {
		const o = offered.get(conversation);
		offered.delete(conversation);
		if (!o) return undefined;
		const lines: string[] = [];
		const skill = answers.skill;
		const p = skill?.probabilities?.[skill.choice] ?? 0;
		if (skill && skill.choice !== "none" && p >= SURE_SKILL) {
			const name = o.skills[Number(skill.choice.slice(1))];
			if (name) lines.push(`the skill "${name}" fits it: load it with skill_view first and follow it`);
		}
		const servers = o.servers.filter((_, i) => (answers[`server${i}`]?.probabilities?.needed ?? 0) >= SURE_SERVER);
		if (servers.length) lines.push(`the tools it needs are likely those of ${servers.map((s) => `"${s}" (mcp__${s}__*)`).join(", ")}`);
		return lines.length ? { hint: `[Evaluator, for this request: ${lines.join("; ")}.]` } : undefined;
	},
};
