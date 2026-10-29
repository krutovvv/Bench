// The assistant: Bench's default extension. Pi's own coding tools (read, write, edit, bash) and
// tool search, plus the person-facing tools (clarify, todo, memory, skills, cron,
// session_search) and the inserts' tools (ssh_run). Its prompt is a
// few short sections (src/prompt.ts). An agent can select other extensions next to it or instead
// of it (its settings.json "extensions").

import path from "node:path";
import { Type } from "@earendil-works/pi-ai";
import { defineExtension, defineTool, GenerationTask, hook, ProviderDoc, section, ToolTask } from "@earendil-works/pi-durable";
import { createNodePhotonImages } from "@earendil-works/pi-durable/images/node";
import { createCodingTools } from "@earendil-works/pi-durable/tools";
import { approvalsMode } from "../config.ts";
import { clarify, planNudge, sessionSearch, todo } from "../builtin/ask.ts";
import { CronTask, cronTool } from "../builtin/cron.ts";
import { ProcessTask, processTool } from "../builtin/process.ts";
import { memoryTool } from "../builtin/memory.ts";
import { skillManage, skillView } from "../builtin/skills.ts";
import { resolveComment } from "../builtin/review.ts";
import { specFor, specsOf } from "../specs.ts";
import { machineOf } from "../repo.ts";
import { compactWithoutReasoning } from "../turn/compaction.ts";
import { sshRun } from "../builtin/ssh.ts";
import { stepGate, stepNote } from "../turn/steps.ts";
import { fitRequestFor, fitResultFor, seeUsage } from "../turn/window.ts";
import { toolsToLoad } from "../judge/start.ts";
import { markNotRun, seeAnswer, seeNestedResult, seeRequest, seeResult, turnOf } from "../turn/turn.ts";
import { judgeActive, judgeFor } from "../judge/client.ts";
import { PROVIDER as EVALUATOR } from "../judge/models.ts";
import { atAnswer, atCall, atRequest, atResult } from "../judge/index.ts";
import { host } from "../host.ts";
import { steer } from "../runtime/steer.ts";
import { frozen } from "../turn/frozen.ts";
import { slot } from "../turn/kept.ts";
import { environment, inserted, notes, projectContext, skills, soul } from "../prompt.ts";
import { Inserts } from "../runtime/inserts.ts";

const of = (input: any) => host().thread(input.conversationId);
const s = (render: (x: NonNullable<ReturnType<typeof of>>) => string | undefined | Promise<string | undefined>) => (input: any) => {
	const info = of(input);
	return info ? render(info) : undefined;
};
/** A section held as it was until the next compaction (`turn/frozen.ts`): a change would rewrite the prompt's head. */
const held = (name: string, render: Parameters<typeof s>[0]) => (input: any) => frozen(String(input.conversationId), name, () => s(render)(input));

/** Commands that ask first when approvals are on (the agent's settings.json "approvals": "manual"). */
const DANGEROUS: [RegExp, string][] = [
	[/\brm\s+(-[a-z]*r[a-z]*f|-[a-z]*f[a-z]*r)\b/i, "recursive delete"],
	[/\bsudo\b/, "runs as root"],
	[/\bgit\s+(push\s+.*--force|reset\s+--hard|clean\s+-[a-z]*f)/i, "rewrites or discards git work"],
	[/\b(mkfs|dd\s+if=|diskutil\s+erase)/i, "writes a disk"],
	[/\b(shutdown|reboot|launchctl\s+unload)\b/i, "stops the system or a service"],
	[/curl[^|]*\|\s*(ba|z)?sh\b/i, "runs a downloaded script"],
];

import { globTool, grepTool, lsTool } from "../builtin/search.ts";
import { createAnchoredEdit, createAnchoredRead } from "../builtin/hashline.ts";
import { lspTool, placeOf } from "../builtin/navigate.ts";
import { diagnosticsFor, knownDiagnostics } from "../lsp/lsp.ts";

/** What the project's language servers say: about one file, or everything they have seen. */
const diagnosticsTool = defineTool({
	name: "diagnostics",
	description:
		"Errors and warnings from the project's language servers (its lsp.json): for one file, checked now, or for every file they have seen. After edit and write they come with the result by themselves.",
	parameters: Type.Object({ path: Type.Optional(Type.String({ description: "A file; empty: every file seen" })) }),
	async execute(args: any, api: any) {
		const info = host().thread(api.conversationId);
		if (!info) throw new Error("no session");
		if (!args.path) return { output: [{ type: "text" as const, text: knownDiagnostics(info.agent) }] };
		const at = await placeOf(info, String(args.path));
		if (!at) throw new Error(`cannot read ${args.path}`);
		const d = await diagnosticsFor(info.agent, info.cwd, at.machine, at.file, at.text);
		return { output: [{ type: "text" as const, text: d ?? "No language server of this project knows this kind of file (its lsp.json)." }] };
	},
});

// Pictures the read tool shrinks to the model's limits: a shot of a Retina window (2480×1600) is
// over them, and without a processor Pi refuses the file instead of resizing it.
export const images = await createNodePhotonImages();
// Pi's coding tools, with read and edit by line anchors (`builtin/hashline.ts`).
const Coding = createCodingTools({ images });
const coding = (Coding.tools ?? []).map((t: any) => (t.name === "read" ? createAnchoredRead({ images }) : t.name === "edit" ? createAnchoredEdit() : t));

/** The specs each conversation was told of: one line per spec per thread, not per file. */
const toldSpecs = slot(() => new Set<string>());

/** "This file is under <spec>": once per thread for each spec, none outside the specs. */
async function specNote(info: { cwd: string; project?: string }, conversation: string, p: string): Promise<string | undefined> {
	if (!p) return undefined;
	const machine = machineOf(info.project) ?? null;
	const file = path.posix.resolve(info.cwd || "/", p);
	const { root, specs } = await specsOf(machine, path.posix.dirname(file));
	const spec = specFor(root, specs, file);
	if (!spec) return undefined;
	const told = toldSpecs.get(conversation);
	if (told.has(spec.id)) return undefined;
	told.add(spec.id);
	return `[${path.posix.relative(root, file)} is under the spec ${path.posix.relative(root, spec.path)} ("${spec.title}"): read it before changing what the module does, and update it when a decision or contract there changes.]`;
}

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
			// Last, what the model reads is cut to its share of the window (`turn/window.ts`).
			afterTool: async (call: any, result: any, api: any, context: any) => {
				if (ownCall(call)) {
					// A script's question to the evaluator counts with the points' (the thread's meters).
					if (String(call.name).endsWith(".classify") && evaluatorModel(call) && !result?.isError) host().count(String(api.conversationId), "call");
					return undefined;
				}
				const done = await notesAfter(call, result, api, context);
				const fitted = (await fitResultFor(String(api.conversationId), done ?? result, context?.abortSignal)) ?? done;
				// The MCP tools the evaluator named at the turn's start are offered from the next step.
				const load = call.parent ? [] : toolsToLoad(turnOf(String(api.conversationId)));
				if (!load.length) return fitted;
				const base = fitted ?? result;
				return { ...base, control: { ...(base.control ?? {}), addTools: [...(base.control?.addTools ?? []), ...load] } };
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
		return evaluatorModel(call) && !judgeFor(String(api.conversationId))
			? { block: "The evaluator is off in this project (its Settings → Model → Evaluator) or has no server in the gear: decide these in the script's code, or by reading them yourself." }
			: undefined;
	}
	// A step of the turn (a script's nested calls are its own): refused past the project's limit.
	if (!call.parent) {
		const refused = stepGate(String(api.conversationId));
		if (refused) return { block: refused };
	}
	const shell = ["bash", "ssh_run"].includes(call.name) && !!info && approvalsMode(info.agent) === "manual";
	const command = String(call.arguments?.command ?? "");
	const hit = shell ? DANGEROUS.find(([re]) => re.test(command))?.[1] : undefined;
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

/** The notes that come with a call's result, and the result as the evaluator left it; undefined: unchanged. */
async function notesAfter(call: any, result: any, api: any, context: any): Promise<any> {
	const conversation = String(api.conversationId);
	const top = !call.parent;
	if (top) seeResult(conversation, call, result);
	else seeNestedResult(conversation, call, result);
	// Read before the evaluator is asked: when it fails now, this step still had the classic reminder's turn.
	const evaluated = top && judgeActive(conversation);
	const none: { result?: any; notes: string[] } = { notes: [] };
	const judged = top ? await atResult(conversation, call, result, context?.abortSignal).catch(() => none) : none;
	const ranked = !!judged.result;
	if (judged.result) result = judged.result;
	const nudge = planNudge(conversation, call);
	const first = [evaluated ? undefined : nudge, ...judged.notes].filter((n): n is string => !!n);
	const withNotes = (notes: string[]) =>
		notes.length || ranked ? { ...result, output: [...(result.output ?? []), ...notes.map((text) => ({ type: "text", text }))] } : undefined;
	if (!["read", "edit", "write"].includes(call.name) || result?.isError) return withNotes(first);
	const info = host().thread(api.conversationId);
	if (!info) return withNotes(first);
	const notes: string[] = [...first];
	const spec = await specNote(info, String(api.conversationId), String(call.arguments?.path ?? "")).catch(() => undefined);
	if (spec) notes.push(spec);
	if (call.name !== "read") {
		const at = await placeOf(info, String(call.arguments?.path ?? ""));
		const diag = at ? await diagnosticsFor(info.agent, info.cwd, at.machine, at.file, at.text).catch(() => undefined) : undefined;
		if (diag) notes.push(diag);
	}
	return withNotes(notes);
}
