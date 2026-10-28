// The Explorer's editor and a project's language servers (`../lsp.ts`): a file's problems as it
// is typed, the hover at a place, where a symbol is defined, and all the problems they know. The editor sends the file's text as
// it is (unsaved too); the servers are the ones the project's threads use, in the same folder.

import { defaultCwd } from "../config.ts";
import { projects } from "../builtin/projects.ts";
import { HttpError, required, type Module } from "../http.ts";
import { definitionAt, hoverAt, knownProblems, problemsOf, type Spot } from "../lsp.ts";
import { onMachine } from "./files.ts";

/** A project's servers: its agent, the folder they run in, and its machine. "general" is Host. */
async function serversOf(id: string): Promise<{ agent: string; root: string; machine?: string }> {
	if (!id || id === "general") return { agent: "default", root: defaultCwd("default") };
	const p = projects.get(id);
	if (!p) throw new HttpError(404, "no such project");
	const machine = p.machine ?? undefined;
	// On a server, an empty folder is the login directory there (as its threads have it).
	const root = machine ? ((await onMachine(machine, projects.dir(id) || "~"))?.path ?? "") : projects.dir(id);
	return { agent: `project:${id}`, root, machine };
}

/** The request's file: `{project, machine, path, text}`, and `{line, character}` for a place.
 * Undefined: the file is not where the project's servers run (the agent's own text of a project
 * on a server). */
async function asked(body: any) {
	const s = await serversOf(String(body?.project ?? ""));
	const file = required(body?.path, "path");
	if ((body?.machine || undefined) !== s.machine) return undefined;
	return { ...s, file, text: String(body?.text ?? "") };
}

const spot = (body: any): Spot => ({ line: Number(body?.line ?? 0), character: Number(body?.character ?? 0) });

export default (): Module => ({
	routes: {
		// `known: false`: no server of the project knows this kind of file, or none is up yet.
		"POST /api/lsp/diagnostics": async (req) => {
			const a = await asked(req.body);
			const problems = a && (await problemsOf(a.agent, a.root, a.machine, a.file, a.text));
			return { known: problems !== undefined, problems: problems ?? [] };
		},
		// Every file's errors and warnings the project's servers know now (its Problems).
		"POST /api/lsp/problems": async (req) => ({ files: knownProblems((await serversOf(String(req.body?.project ?? ""))).agent) }),
		"POST /api/lsp/hover": async (req) => {
			const a = await asked(req.body);
			if (!a) return { markdown: "" };
			return { markdown: await hoverAt(a.agent, a.root, a.machine, a.file, a.text, spot(req.body)) };
		},
		"POST /api/lsp/definition": async (req) => {
			const a = await asked(req.body);
			if (!a) return { places: [] };
			return { places: await definitionAt(a.agent, a.root, a.machine, a.file, a.text, spot(req.body)) };
		},
	},
});
