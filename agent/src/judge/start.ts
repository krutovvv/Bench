// The evaluator at a turn's start: before the model's first request it is asked which of the
// thread's skills fits the person's request, and which of its MCP servers the work will need; at
// the thread's opening message, also whether the request leaves open what only the person can say
// (where the work belongs, which program) — sixty steps of a local model went to the wrong project
// before the person said which one it was. A
// sure answer is a line for the model with that request: load this skill, use these servers'
// tools — and those servers' tools are offered from the turn's next step (`control.addTools` on
// its first result, as `tool_search` loads them). The model looked for them itself before — a step of `skill_view` or `tool_search` each,
// or none, as when it drove Chrome through rea with the browser server at hand.

import { skillsFor, machineOf } from "../repo.ts";
import { mcpServerTools } from "../mcp/mcp.ts";
import { projects } from "../builtin/projects.ts";
import { cut } from "../builtin/turn.ts";
import type { Point } from "./point.ts";

/** How sure the evaluator must be to name a skill, or a server: the skills that fitted came at
 *  0.83–0.95, a wrong one (computer-use for a folder of backups) at 0.71. */
const SURE_SKILL = 0.8;
const SURE_SERVER = 0.75;
/** The most skills and servers offered (a choice holds at most 255 options). */
const MAX_SKILLS = 120;
const MAX_SERVERS = 12;

/** How sure the evaluator must be that the request leaves open what only the person can say. */
const SURE_UNCLEAR = 0.75;
/** The most projects named in the state. */
const MAX_PROJECTS = 40;

/** The most tools offered at once this way. */
const MAX_LOAD = 30;

/** The skills and servers asked about this turn, by question name. */
const offered = new Map<string, { skills: string[]; servers: string[]; tools: string[][]; loadable: string[][] }>();

export const start: Point = {
	name: "start",
	moment: "request",
	async ask({ conversation, turn, thread }) {
		if (turn.started || !turn.request || turn.steps > 0 || !thread) return undefined;
		turn.started = true;
		const skills = (await skillsFor(thread.agent, thread.cwd, machineOf(thread.project)).catch(() => [])).slice(0, MAX_SKILLS);
		const servers = mcpServerTools(thread.agent).slice(0, MAX_SERVERS);
		// The thread's opening message only: a later one leans on what was said before, which the
		// evaluator does not see.
		const opening = turn.opening;
		if (!skills.length && !servers.length && !opening) return undefined;
		offered.set(conversation, { skills: skills.map((k) => k.name), servers: servers.map((s) => s.name), tools: servers.map((s) => s.tools), loadable: servers.map((s) => s.loadable) });
		const questions: Record<string, any> = {};
		const all = projects.list();
		const here = all.find((p) => p.id === thread.project);
		if (opening)
			questions.clear = {
				type: "choice",
				instructions:
					"The person's request opens the thread (thread_project, thread_folder: where its work happens; projects: the person's others). Can the agent start the work, or does it depend on something only the person can say?",
				// "clear" first: the model leans to the first option, and starting is the safe side — a question costs the person a turn.
				criteria: {
					clear: "Clear enough to start: what to do and where follows from the request and the thread's place, or the agent can find it out itself.",
					ask_first:
						"It depends on something only the person can say and the request leaves open or contradicts: which project or folder the work belongs to (another than this thread's?), which program, file or account, what result they want. Guessing would send the work the wrong way.",
				},
			};
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
		return {
			questions,
			...(opening ? { state: { thread_project: here?.name ?? "Host (the agent's own environment)", thread_folder: thread.cwd, projects: all.slice(0, MAX_PROJECTS).map((p) => p.name) } } : {}),
		};
	},
	apply({ conversation, turn }, answers) {
		const o = offered.get(conversation);
		offered.delete(conversation);
		if (!o) return undefined;
		const lines: string[] = [];
		// Before anything else: what only the person can say is asked, not guessed.
		if ((answers.clear?.probabilities?.ask_first ?? 0) >= SURE_UNCLEAR)
			lines.push("it leaves open something the work depends on that only the person can say (which project or folder, which program or file, what result) — before any other step, ask them with clarify: one question, with choices");
		const skill = answers.skill;
		const p = skill?.probabilities?.[skill.choice] ?? 0;
		if (skill && skill.choice !== "none" && p >= SURE_SKILL) {
			const name = o.skills[Number(skill.choice.slice(1))];
			if (name) {
				lines.push(`the skill "${name}" fits it: load it with skill_view first and follow it`);
				turn.named.add(`skill:${name}`);
			}
		}
		const chosen = o.servers.map((s, i) => [s, i] as const).filter(([, i]) => (answers[`server${i}`]?.probabilities?.needed ?? 0) >= SURE_SERVER);
		// A server behind code mode (Pi's default) is named for a script; a deferred one's tools are
		// offered from the next step; a direct one is offered already.
		if (chosen.length) {
			const scripted = chosen.filter(([, i]) => o.loadable[i].length < o.tools[i].length);
			const loaded = chosen.filter(([, i]) => o.loadable[i].length);
			if (scripted.length) lines.push(`the tools it needs are likely those of ${scripted.map(([s]) => `"${s}" (tools.mcp__${s}__* in a codemode script)`).join(", ")}`);
			if (loaded.length) lines.push(`the tools it needs are likely those of ${loaded.map(([s]) => `"${s}" (mcp__${s}__*)`).join(", ")}, offered from your next step`);
			turn.load = loaded.flatMap(([, i]) => o.loadable[i]).slice(0, MAX_LOAD);
		}
		return lines.length ? { hint: `[Evaluator, for this request: ${lines.join("; ")}.]` } : undefined;
	},
};

/** The tools to offer from the next step, once (`afterTool` puts them in the result's `control.addTools`). */
export function toolsToLoad(turn: { load: string[] }): string[] {
	const load = turn.load;
	turn.load = [];
	return load;
}
