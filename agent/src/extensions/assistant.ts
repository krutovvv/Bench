// The assistant: Bench's default extension. Pi's own coding tools (read, write, edit, bash) and
// tool search, plus the person-facing tools (clarify, todo, memory, skills, cron,
// session_search) and the inserts' tools (ssh_run). Its prompt is a
// few short sections (src/prompt.ts). An agent can select other extensions next to it or instead
// of it (its settings.json "extensions").

import { defineExtension, GenerationTask, hook, ProviderDoc, section, ToolTask } from "@earendil-works/pi-durable";
import { createNodePhotonImages } from "@earendil-works/pi-durable/images/node";
import { approvalOf } from "../builtin/approvals.ts";
import { clarify, sessionSearch, todo } from "../builtin/ask.ts";
import { CronTask, cronTool } from "../builtin/cron.ts";
import { ProcessTask, processTool } from "../builtin/process.ts";
import { memoryTool } from "../builtin/memory.ts";
import { skillManage, skillView } from "../builtin/skills.ts";
import { resolveComment } from "../builtin/review.ts";
import { specNote } from "./spec-note.ts";
import { compactWithoutReasoning } from "../turn/compaction.ts";
import { sshRun } from "../builtin/ssh.ts";
import { stepGate, stepNote } from "../turn/steps.ts";
import { fitRequestFor, seeUsage, withNotes } from "../turn/window.ts";
import { toolsToLoad } from "../judge/start.ts";
import { markNotRun, planNudge, seeAnswer, seeNestedResult, seeRequest, seeResult, turnOf } from "../turn/turn.ts";
import { evaluatorFor, judgeActive } from "../judge/client.ts";
import { PROVIDER as EVALUATOR } from "../judge/models.ts";
import { atAnswer, atCall, atRequest, atResult } from "../judge/index.ts";
import { host } from "../host.ts";
import { steer } from "../runtime/steer.ts";
import { frozen } from "../turn/frozen.ts";
import { environment, inserted, notes, projectContext, skills, soul } from "../prompt.ts";
import { Inserts } from "../runtime/inserts.ts";
import { globTool, grepTool, lsTool } from "../builtin/search.ts";
import { codingTools } from "../builtin/hashline.ts";
import { lspTool, placeOf } from "../builtin/navigate.ts";
import { diagnosticsTool } from "../builtin/diagnostics.ts";
import { diagnosticsFor } from "../lsp/lsp.ts";

const of = (input: any) => host().thread(input.conversationId);
const s = (render: (x: NonNullable<ReturnType<typeof of>>) => string | undefined | Promise<string | undefined>) => (input: any) => {
	const info = of(input);
	return info ? render(info) : undefined;
};
/** A section held as it was until the next compaction (`turn/frozen.ts`): a change would rewrite the prompt's head. */
const held = (name: string, render: Parameters<typeof s>[0]) => (input: any) => frozen(String(input.conversationId), name, () => s(render)(input));

// Pictures the read tool shrinks to the model's limits: a shot of a Retina window (2480×1600) is
// over them, and without a processor Pi refuses the file instead of resizing it.
export const images = await createNodePhotonImages();
// Pi's coding tools, with read and edit by line anchors (`builtin/hashline.ts`).
const coding = codingTools({ images });

export const assistant = defineExtension({
	name: "assistant",
	tools: [...coding, grepTool, globTool, lsTool, diagnosticsTool, lspTool, clarify, todo, resolveComment, memoryTool, skillView, skillManage, cronTool, sessionSearch, sshRun, processTool],
	tasks: [CronTask, ProcessTask],
	sections: [
		section("soul", held("soul", soul), { tag: false }),
		section("environment", held("environment", environment)),
		// Read from the thread's own document: what the person inserted.
		section(
			"inserts",
			async (input: any, context: any) => {
				const doc = (await input.read.snapshot(Inserts, input.conversationId, context).catch(() => undefined)) as any;
				return inserted(Object.values<any>(doc?.items ?? {}).filter((i) => !i.removed));
			},
			{ tag: false },
		),
		section("project", held("project", projectContext), { tag: false }),
		section("skills", held("skills", skills), { tag: false }),
		section("notes", held("notes", notes), { tag: false }),
	],
	hooks: [
		compactWithoutReasoning,
		// The turn as the step limit and the evaluator see it (`turn/turn.ts`): every request
		// notes the person's last message; past the step limit it carries the instruction to
		// answer (`turn/steps.ts`). Every request fits the model's window (`turn/window.ts`),
		// the provider's count after it correcting the estimate. The answer is the evaluator's to
		// check once (`judge/done.ts`).
		hook(GenerationTask, {
			// A turn's first request also carries the evaluator's word on its skill and servers (`judge/start.ts`).
			beforeRequest: async (request: any, api: any, context: any) => {
				const conversation = String(api.conversationId);
				seeRequest(conversation, request.messages);
				const { hints, plain } = await atRequest(conversation, context?.abortSignal).catch(() => ({ hints: [] as string[], plain: false }));
				// A plain step goes without reasoning: a word for its provider session (`runtime/steer.ts`).
				if (plain) {
					const session = ((await api.snapshot(ProviderDoc, api.conversationId, context).catch(() => undefined)) as any)?.sessionId;
					if (session) steer(session, { plain: true });
				}
				const note = stepNote(conversation);
				const extra = [...hints.map((h) => ({ role: "user", content: h, timestamp: Date.now() })), ...(note ? [note] : [])];
				const messages = extra.length ? [...request.messages, ...extra] : request.messages;
				const fitted = await fitRequestFor(conversation, messages, context?.abortSignal);
				return fitted || extra.length ? { messages: fitted ?? messages } : undefined;
			},
			afterResponse: (message: any, api: any) => {
				seeUsage(String(api.conversationId), message?.usage);
				seeAnswer(String(api.conversationId), message);
			},
			onYield: async (answer: any, api: any, context: any) => {
				const back = await atAnswer(String(api.conversationId), answer, context?.abortSignal).catch(() => undefined);
				return back ? { continue: back } : undefined;
			},
		} as any),
		hook(ToolTask, {
			beforeTool: async (call: any, api: any, context: any) => {
				const r = await gate(call, api, context);
				// A refused call is marked so: the turn reads its result as never run, not as a failure.
				if (r?.block) markNotRun(String(api.conversationId), call.id);
				return r;
			},
			// The file's errors from the project's language servers, with the edit that made them
			// (`lsp.ts`): the model sees them as an editor shows them, without asking.
			// The module's spec, the first time the thread touches a file under it (`specs.ts`):
			// the decisions and contracts the code does not show, found without being looked for.
			// The plan left unchanged for several steps (`planNudge`): the model is reminded to update it.
			// With the evaluator on (`judge/`), its points instead: a long search's matches come
			// likeliest first, the plan's items are marked, the course is checked.
			// The tool's own output is cut to its share of the window first (`turn/window.ts`), and the
			// notes follow it whole: a cut of the joined text would drop them with the middle.
			afterTool: async (call: any, result: any, api: any, context: any) => {
				if (ownCall(call)) {
					// A script's question to the evaluator counts with the points' (the thread's meters).
					if (String(call.name).endsWith(".classify") && evaluatorModel(call) && !result?.isError) host().count(String(api.conversationId), "call");
					return undefined;
				}
				const after = await notesAfter(call, result, api, context);
				const noted = await withNotes(String(api.conversationId), after.result, after.notes, context?.abortSignal);
				// The MCP tools the evaluator named at the turn's start are offered from the next step.
				const load = call.parent ? [] : toolsToLoad(turnOf(String(api.conversationId)));
				if (!load.length) return noted === result ? undefined : noted;
				return { ...noted, control: { ...(noted.control ?? {}), addTools: [...(noted.control?.addTools ?? []), ...load] } };
			},
		} as any),
	],
} as any);

/** Code mode's own calls of a model (`models.classify()`, `models.generateImages()`): a script's
 *  use of the evaluator is not a step the evaluator weighs, nor a result it reads. */
function ownCall(call: any): boolean {
	return String(call.name).startsWith("pi.codemode.");
}

/** A script's model call on the evaluator (`bench/evaluator`), not another provider's model. */
function evaluatorModel(call: any): boolean {
	const model = call.arguments?.model;
	return (typeof model === "string" ? model.split("/")[0] : model?.provider) === EVALUATOR;
}

/** Before a call: the person's hold, the step limit, the evaluator's repeat and risk, approvals. */
async function gate(call: any, api: any, context: any): Promise<{ block?: string } | undefined> {
	// The person's hold, felt before any next step — the current answer and the step
	// already running are let through; nothing new begins until the hold lifts.
	// The conversation is the hook's `api`'s, not the call's (Pi's ToolCall has no id of it).
	const info = host().thread(api.conversationId);
	if (info && !(await host().hold(info.id, context?.abortSignal))) return { block: "The person stopped the run." };
	// A script asking the evaluator: only where the project uses it (its Settings, the gear's servers).
	if (ownCall(call)) {
		return evaluatorModel(call) && !(info && evaluatorFor(info.agent))
			? { block: "The evaluator is off in this project (its Settings → Model → Evaluator) or has no server in the gear: decide these in the script's code, or by reading them yourself." }
			: undefined;
	}
	// A step of the turn (a script's nested calls are its own): refused past the project's limit.
	if (!call.parent) {
		const refused = stepGate(String(api.conversationId));
		if (refused) return { block: refused };
	}
	const { shell, command, hit } = approvalOf(info?.agent, call);
	// The evaluator: a repeat of a failed or known call is refused (`judge/repeat.ts`); with
	// approvals on, a command the patterns let through is judged too (`judge/risk.ts`). A script's
	// nested calls (codemode) too: a script is no way around either.
	const judged = await atCall(String(api.conversationId), call, shell && !hit, context?.abortSignal).catch(() => ({}) as { block?: string; approve?: string });
	if (judged.block) return { block: judged.block };
	const why = hit ?? judged.approve;
	if (!why || !info) return undefined;
	const choice = await host().approve(info.id, String(call.id), command, why);
	return choice === "deny" ? { block: `The person declined this command (${why}). Do not retry it; ask or choose another way.` } : undefined;
}

/** The result as the evaluator left it (reordered, or as it was), and the notes that come after it. */
async function notesAfter(call: any, result: any, api: any, context: any): Promise<{ result: any; notes: string[] }> {
	const conversation = String(api.conversationId);
	const top = !call.parent;
	if (top) seeResult(conversation, call, result);
	else seeNestedResult(conversation, call, result);
	// Read before the evaluator is asked: when it fails now, this step still had the classic reminder's turn.
	const evaluated = top && judgeActive(conversation);
	const none: { result?: any; notes: string[] } = { notes: [] };
	const judged = top ? await atResult(conversation, call, result, context?.abortSignal).catch(() => none) : none;
	if (judged.result) result = judged.result;
	const nudge = top && !evaluated ? planNudge(conversation) : undefined;
	const notes = [nudge, ...judged.notes].filter((n): n is string => !!n);
	if (!["read", "edit", "write"].includes(call.name) || result?.isError) return { result, notes };
	const info = host().thread(api.conversationId);
	if (!info) return { result, notes };
	const spec = await specNote(info, String(api.conversationId), String(call.arguments?.path ?? "")).catch(() => undefined);
	if (spec) notes.push(spec);
	if (call.name !== "read") {
		const at = await placeOf(info, String(call.arguments?.path ?? ""));
		const diag = at ? await diagnosticsFor(info.agent, info.cwd, at.machine, at.file, at.text).catch(() => undefined) : undefined;
		if (diag) notes.push(diag);
	}
	return { result, notes };
}
