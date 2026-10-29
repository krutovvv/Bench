// The evaluator mid-turn on skills: `start.ts` names a skill for the request as it opens, but a
// turn's work moves on — to a release, a report, a form of the person's own — and the skill for that
// part is never loaded; the model works it out again, slower and its own way. Every so many steps
// the evaluator is asked which skill, if any, fits what the agent is doing now; a sure one not yet
// named or loaded this turn is a note with that step's result.

import { skillsFor, machineOf } from "../repo.ts";
import { host } from "../host.ts";
import { cut } from "../turn/turn.ts";
import type { Point } from "./point.ts";

/** Steps between two looks. */
export const SKILL_EVERY = 10;
/** How sure the evaluator must be to name one (as at the start). */
const SURE = 0.8;
const MAX_SKILLS = 120;

/** The skills offered at this look, by conversation. */
const offered = new Map<string, string[]>();

export const skill: Point = {
	name: "skill",
	moment: "result",
	async ask({ conversation, turn }) {
		if (!turn.request || turn.steps - turn.skillAt < SKILL_EVERY) return undefined;
		turn.skillAt = turn.steps;
		const thread = host().thread(conversation);
		if (!thread) return undefined;
		const loaded = new Set(turn.calls.filter((c) => c.name === "skill_view").map((c) => String(c.args?.name ?? "")));
		const skills = (await skillsFor(thread.agent, thread.cwd, machineOf(thread.project)).catch(() => []))
			.filter((k) => !loaded.has(k.name) && !turn.named.has(`skill:${k.name}`))
			.slice(0, MAX_SKILLS);
		if (!skills.length) return undefined;
		offered.set(conversation, skills.map((k) => k.name));
		return {
			questions: {
				skill: {
					type: "choice",
					instructions: "Which of these skills (the person's own procedures) fits what the agent is doing now, judging by its last steps?",
					// "none" first: naming no skill is the safe side.
					criteria: { none: "None of these fits what it is doing now.", ...Object.fromEntries(skills.map((k, i) => [`s${i}`, `${k.name}: ${cut(k.description.replace(/\s+/g, " "), 200)}`])) },
				},
			},
		};
	},
	apply({ conversation, turn }, answers) {
		const names = offered.get(conversation) ?? [];
		offered.delete(conversation);
		const a = answers.skill;
		if (!a || a.choice === "none" || (a.probabilities?.[a.choice] ?? 0) < SURE) return undefined;
		const name = names[Number(a.choice.slice(1))];
		if (!name) return undefined;
		turn.named.add(`skill:${name}`);
		return { note: `[Evaluator: the skill "${name}" fits what you are doing now: load it with skill_view and follow it.]` };
	},
};
