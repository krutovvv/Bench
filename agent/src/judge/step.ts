// The evaluator before every request of a turn but its first (`start.ts` has that one): does the
// step about to be taken need the model to reason, and do the tools of an MCP server it has not
// touched this turn fit it now?
//
// A local model writes its reasoning at its slowest; a step that reads a file, lists a folder or
// runs the tests it just wrote needs none, and "plain" (sure) sends that request without it
// (`runtime/steer.ts`). Never right after a failure: that is what the reasoning is for. A
// server's tools that fit the step are a line for this request: the tools stay offered as they
// were, as a list that changes rewrites the prompt's head and the server's cache with it.

import { mcpServerTools } from "../mcp/mcp.ts";
import { cut } from "../builtin/turn.ts";
import type { Point } from "./point.ts";

/**
 * Whether a plain step goes without reasoning. Off while the dev server's chat template writes its
 * reasoning line ("Reasoning effort is set to low…") at the system prompt's start only when
 * reasoning is on: a step without it rewrites the prompt's head, and the server reads the whole
 * prompt again — twice, as the next step turns it back on (measured: 0 tokens from the cache). On
 * once the template keeps that line whether reasoning is on or not.
 */
export const PLAIN_STEPS = false;
/** How sure the evaluator must be that a step is plain: a step thought through too little costs more than its reasoning. */
const SURE_PLAIN = 0.85;
/** How sure that a server's tools fit the step now. */
const SURE_SERVER = 0.85;
/** The most servers asked about. */
const MAX_SERVERS = 12;

/** The servers asked about this request, by conversation. */
const asked = new Map<string, string[]>();

export const step: Point = {
	name: "step",
	moment: "request",
	ask({ conversation, turn, thread }) {
		if (!turn.request || turn.steps === 0 || !thread) return undefined;
		const last = turn.calls.at(-1);
		const questions: Record<string, any> = {};
		// After a failure the model needs its reasoning: not asked.
		if (PLAIN_STEPS && last && !last.failed && !last.notRun)
			questions.think = {
				type: "choice",
				instructions: "The agent is about to choose its next step (its last steps and their results are in the state). Does that step need it to reason it through?",
				// "think" first: the model leans to the first option, and reasoning is the safe side.
				criteria: {
					think: "Yes: something to work out — an error to understand, a choice between ways, a change to design, results to compare or conclude from.",
					plain: "No: the next step is plain and follows from the last ones — read or list what was found, run what was just written, the next item of a list, a known edit.",
				},
			};
		const used = new Set(turn.calls.map((c) => c.name.match(/^mcp__(.+?)__/)?.[1]).filter(Boolean));
		const servers = mcpServerTools(thread.agent)
			.slice(0, MAX_SERVERS)
			.filter((s) => !used.has(s.name) && !turn.named.has(`server:${s.name}`));
		asked.set(conversation, servers.map((s) => s.name));
		servers.forEach((s, i) => {
			questions[`server${i}`] = {
				type: "choice",
				instructions: `Does the agent's next step need the tools of the MCP server "${s.name}" (tools: ${cut(s.tools.map((t) => t.replace(/^mcp__[^_]+__/, "")).join(", "), 400)})?`,
				criteria: { not_needed: "The next step does not need these tools.", needed: "The next step needs these tools." },
			};
		});
		return Object.keys(questions).length ? { questions } : undefined;
	},
	apply({ conversation, turn }, answers) {
		const servers = asked.get(conversation) ?? [];
		asked.delete(conversation);
		const plain = (answers.think?.probabilities?.plain ?? 0) >= SURE_PLAIN;
		const fit = servers.filter((_, i) => (answers[`server${i}`]?.probabilities?.needed ?? 0) >= SURE_SERVER);
		for (const s of fit) turn.named.add(`server:${s}`);
		const hint = fit.length ? `[Evaluator, for this step: the tools of ${fit.map((s) => `"${s}" (mcp__${s}__*)`).join(", ")} fit it.]` : undefined;
		return plain || hint ? { ...(plain ? { plain } : {}), ...(hint ? { hint } : {}) } : undefined;
	},
};
