// The module's spec, the first time a thread touches a file under it (`specs.ts`): the decisions
// and contracts the code does not show, found without being looked for. A note with the result of
// read, edit and write (`extensions/assistant.ts`), once per thread for each spec.

import path from "node:path";
import { machineOf } from "../repo.ts";
import { specFor, specsOf } from "../specs.ts";
import { slot } from "../turn/kept.ts";

/** The specs each conversation was told of: one line per spec per thread, not per file. */
const toldSpecs = slot(() => new Set<string>());

/** "This file is under <spec>": once per thread for each spec, none outside the specs. */
export async function specNote(info: { cwd: string; project?: string }, conversation: string, p: string): Promise<string | undefined> {
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
