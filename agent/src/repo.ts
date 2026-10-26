// What a project's own folder says to the agent: its instructions (AGENTS.md or CLAUDE.md) and its
// skills (.agents/skills, .claude/skills), from the folder up to its repository's root — read where
// the folder is. On this agent's machine from the disk; on a project's server (`ssh-env.ts`) with
// one script over the same connection, kept for a short while, since every turn's prompt asks.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { projects } from "./builtin/projects.ts";
import { frontMatter, repoSkillDirs, skillsIn, type Skill } from "./builtin/skills.ts";
import { machineEnv, quote } from "./ssh-env.ts";

/** One instructions file: its path and what it says (at most 20 000 characters). */
export interface Instructions {
	file: string;
	text: string;
}

export interface Repo {
	/** From the repository's root down to the folder. */
	instructions: Instructions[];
	/** The folder's skills, the nearest one of a name first. */
	skills: Skill[];
}

const CAP = 20000;

/** The server a project works on, if it is not this agent's machine. */
export function machineOf(project: string | undefined): string | undefined {
	return project ? (projects.get(project)?.machine ?? undefined) : undefined;
}

/** What `folder` (on `machine`, or here) says to the agent. */
export async function repoOf(folder: string, machine?: string): Promise<Repo> {
	if (!folder) return { instructions: [], skills: [] };
	return machine ? remote(machine, folder) : local(folder);
}

function local(folder: string): Repo {
	if (path.resolve(folder) === os.homedir()) return { instructions: [], skills: repoSkillDirs(folder).flatMap((d) => skillsIn(d, "repo")) };
	let root = folder;
	try {
		root = execFileSync("git", ["-C", folder, "rev-parse", "--show-toplevel"], { encoding: "utf8", timeout: 2000, stdio: ["ignore", "pipe", "ignore"] }).trim() || folder;
	} catch {}
	const dirs: string[] = [];
	for (let d = path.resolve(folder); ; d = path.dirname(d)) {
		dirs.unshift(d);
		if (d === root || d === path.dirname(d)) break;
	}
	const instructions: Instructions[] = [];
	for (const d of dirs)
		for (const name of ["AGENTS.md", "CLAUDE.md"]) {
			const f = path.join(d, name);
			if (fs.existsSync(f)) {
				instructions.push({ file: f, text: fs.readFileSync(f, "utf8").trim().slice(0, CAP) });
				break;
			}
		}
	return { instructions, skills: repoSkillDirs(folder).flatMap((d) => skillsIn(d, "repo")) };
}

const kept = new Map<string, { at: number; repo: Promise<Repo> }>();
const KEEP_MS = 30_000;

function remote(machine: string, folder: string): Promise<Repo> {
	const key = `${machine}\0${folder}`;
	const hit = kept.get(key);
	if (hit && Date.now() - hit.at < KEEP_MS) return hit.repo;
	const repo = readRemote(machine, folder).catch((e) => {
		kept.delete(key);
		throw e;
	});
	kept.set(key, { at: Date.now(), repo });
	return repo;
}

/** Forget what was read on a server (a skill written there, a test). */
export function forgetRepos(): void {
	kept.clear();
}

// From the folder up to the repository's root (a .git), the home or /, the root printed first.
// Each find is marked by a \001 line: `I <file>` (instructions) or `S <skills root>\t<SKILL.md>`
// (the head of a skill's SKILL.md, enough for its front matter).
const SCRIPT = `d=$(cd -- "$1" 2>/dev/null && pwd) || exit 0
dirs=""
while :; do
  dirs="$d
$dirs"
  if [ -e "$d/.git" ] || [ "$d" = "$HOME" ] || [ "$d" = / ]; then break; fi
  d=$(dirname -- "$d")
done
printf '%s\\n' "$dirs" | while IFS= read -r x; do
  [ -n "$x" ] || continue
  if [ "$x" != "$HOME" ]; then
    for n in AGENTS.md CLAUDE.md; do
      if [ -f "$x/$n" ]; then printf '\\001I %s\\n' "$x/$n"; head -c ${CAP} -- "$x/$n"; printf '\\n'; break; fi
    done
  fi
  for s in "$x/.agents/skills" "$x/.claude/skills"; do
    [ -d "$s" ] || continue
    find "$s" -maxdepth 5 -name SKILL.md -type f 2>/dev/null | sort | while IFS= read -r f; do
      printf '\\001S %s\\t%s\\n' "$s" "$f"; head -c 4000 -- "$f"; printf '\\n'
    done
  done
done`;

async function readRemote(machine: string, folder: string): Promise<Repo> {
	const env = await machineEnv(machine, "");
	const r = await env.run(`sh -c ${quote(SCRIPT)} sh ${quote(folder)}`, BACKGROUND_CONTEXT);
	if (r.code !== 0) throw new Error(`${machine}: ${r.stderr.trim() || `reading ${folder} failed (${r.code})`}`);
	const instructions: Instructions[] = [];
	const roots: string[] = [];
	const found: Skill[] = [];
	for (const chunk of r.stdout.toString().split("\x01").slice(1)) {
		const nl = chunk.indexOf("\n");
		const head = chunk.slice(0, nl < 0 ? undefined : nl);
		const body = nl < 0 ? "" : chunk.slice(nl + 1);
		if (head.startsWith("I ")) {
			instructions.push({ file: head.slice(2), text: body.trim().slice(0, CAP) });
		} else if (head.startsWith("S ")) {
			const [root, file] = head.slice(2).split("\t");
			if (!root || !file) continue;
			if (!roots.includes(root)) roots.push(root);
			const dir = path.posix.dirname(file);
			const rel = path.posix.relative(root, dir).split("/");
			// As here: hidden folders are not walked, and a skill's own folders hold no skills.
			if (dir === root || rel.some((p) => p.startsWith("."))) continue;
			const fm = frontMatter(body);
			const name = rel[rel.length - 1]!;
			found.push({ name: String(fm.name ?? name), description: String(fm.description ?? "").trim(), category: rel.slice(0, -1).join("/"), dir, scope: "repo", machine });
		}
	}
	// The nearest skills root first, as `repoSkillDirs` orders them here.
	const order = (k: Skill) => roots.length - 1 - roots.findIndex((r) => k.dir.startsWith(`${r}/`));
	const skills = found.filter((k) => !found.some((o) => k.dir.startsWith(`${o.dir}/`))).sort((a, b) => order(a) - order(b) || a.name.localeCompare(b.name));
	return { instructions, skills };
}

/** A file of a skill (SKILL.md by default), wherever the skill is. */
export async function readSkill(skill: Skill, file = "SKILL.md"): Promise<string> {
	const target = path.posix.resolve(skill.dir, file);
	if (target !== path.posix.join(skill.dir, "SKILL.md") && !target.startsWith(skill.dir + "/")) throw new Error("file is outside the skill folder");
	if (!skill.machine) return fs.readFileSync(target, "utf8");
	const env = await machineEnv(skill.machine, "");
	const r = await env.readTextFile(target, BACKGROUND_CONTEXT);
	if (!r.ok) throw new Error(r.error.message);
	return r.value;
}

/** The other files in a skill's folder, relative to it. */
export async function skillFiles(skill: Skill): Promise<string[]> {
	if (!skill.machine) return fs.readdirSync(skill.dir, { recursive: true }).map(String).filter((f) => f !== "SKILL.md" && fs.statSync(path.join(skill.dir, f)).isFile());
	const env = await machineEnv(skill.machine, "");
	const r = await env.run(`cd ${quote(skill.dir)} && find . -type f ! -name SKILL.md 2>/dev/null | sed 's|^\\./||' | sort`, BACKGROUND_CONTEXT);
	return r.code === 0 ? r.stdout.toString().split("\n").filter(Boolean) : [];
}
