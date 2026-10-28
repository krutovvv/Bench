// Projects and their environment: the soul, MCP servers, skills, prompts, the Telegram
// bot, the schedule. "general" is the Host project: the agent's own environment.

import fs from "node:fs";
import path from "node:path";
import { agentDir, agentMcp, compactionAt, getSetting, readAgent, resolveModel, setSetting, writeAgentMcp } from "../config.ts";
import { arm, jobs, newJob, type Job } from "../builtin/cron.ts";
import { projects } from "../builtin/projects.ts";
import { listSkills } from "../builtin/skills.ts";
import { readSkill, skillsFor } from "../repo.ts";
import { HttpError, type Ctx, type Module } from "../http.ts";
import { authorizeServer, connectMcp, mcpStatus } from "../mcp/mcp.ts";
import { lspNeeds, lspStates, parseLspSpec, readLsp, writeLsp, writeLspServer } from "../lsp/lsp.ts";
import { reconnectTelegram, syncTelegram, telegramStatus } from "../telegram.ts";
import { cronView } from "./cron.ts";

/** A project's environment: the agent its threads run with, its folder, the project row. */
export function envOf(id: string): { agent: string; dir: string; project: { id: string; name: string; telegram?: { token: string; allowed_users: string[]; username?: string; proxy?: string } | null } } {
	if (id === "general") {
		const tg = getSetting("telegram");
		return { agent: "default", dir: agentDir("default"), project: { id, name: "Host", telegram: tg?.token ? { token: tg.token, allowed_users: (tg.allowed_users ?? []).map(String), proxy: tg.proxy } : null } };
	}
	const p = projects.get(id);
	if (!p) throw new HttpError(404, "no such project");
	return { agent: `project:${id}`, dir: projects.env(id), project: p };
}

/** Every skill a project's threads can use (its folder's read where it is: `repo.ts`). */
function skillsOf(agent: string) {
	const p = agent.startsWith("project:") ? projects.get(agent.slice(8)) : undefined;
	return p?.machine ? skillsFor(agent, p.folder ?? undefined, p.machine).catch(() => listSkills(agent, p.folder ?? undefined, [])) : Promise.resolve(listSkills(agent));
}

/** The project's MCP servers as the block shows them. `checked`: something tried the server
 * (a thread asked for its tools, or Check); `connected`: its handshake is live now. */
function mcpView(agent: string) {
	const status = mcpStatus(agent);
	return Object.entries(agentMcp(agent)).map(([name, spec]: [string, any]) => {
		const s = status.find((x) => x.name === name);
		return { name, kind: spec?.url ? "http" : "stdio", enabled: spec?.enabled !== false, checked: !!s, connected: !!s?.connected, tools: s?.tools ?? 0, error: s?.error ?? null, needs_auth: !!s?.needsAuth, spec: JSON.stringify(spec, null, 2) };
	});
}

const id1 = (m: RegExpMatchArray) => decodeURIComponent(m[1]);

export default ({ rt, publish }: Ctx): Module => ({
	routes: {
		// `browser`: the project has the watchable server browser among its MCP servers (Bench
		// offers /browser only there); `host_browser`: the machine's own agent has it;
		// `host_telegram`: the agent's general bot (its Host's) is set.
		"GET /api/projects": () => ({
			projects: projects.list().map((p) => ({ ...p, browser: !p.host && "browser" in agentMcp(`project:${p.id}`) })),
			host_browser: "browser" in agentMcp("default"),
			host_telegram: !!getSetting("telegram")?.token,
		}),
		"POST /api/projects": (req) => {
			if (!String(req.body?.name ?? "").trim()) throw new HttpError(400, "name is required");
			return { project: projects.save({ ...req.body, name: String(req.body.name).trim() }) };
		},
	},
	dynamic: [
		[
			"PUT",
			/^\/api\/projects\/([^/]+)$/,
			(req, m) => {
				const id = id1(m);
				const prev = id === "general" ? undefined : projects.get(id);
				if (id !== "general" && !prev) throw new HttpError(404, "no such project");
				// Its folder, set apart: the threads that worked in the old one follow it.
				const { folder, ...rest } = req.body ?? {};
				if (folder !== undefined) {
					try {
						projects.setFolder(id, folder);
					} catch (e: any) {
						throw new HttpError(400, e.message);
					}
				}
				if (!prev) return { ok: true };
				// Its agent and machine are chosen when it is made: never changed here.
				const { host: _h, machine: _m, ...own } = rest;
				return { project: projects.save({ ...projects.get(id)!, ...own, id }) };
			},
		],
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)$/,
			(_req, m) => {
				projects.remove(id1(m));
				return { ok: true };
			},
		],
		// Everything a project's Settings show, in one read.
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/env$/,
			async (_req, m) => {
				const id = id1(m);
				const { agent, dir, project: p } = envOf(id);
				const soulFile = path.join(dir, "SYSTEM.md");
				return {
					// When its conversations are summarized (the Model section), and the window it is
					// a share of.
					// Its language servers (lsp.json) and how each is doing.
					// What its machine lacks for them is said before any is started (no Node there).
					lsp: { servers: readLsp(agent), states: { ...lspStates(agent), ...(await lspNeeds(agent, projects.get(id)?.machine ?? undefined)) } },
					compaction: compactionAt(agent) ?? null,
					code_mode: Boolean(readAgent(agent).codeMode),
					step_limit: readAgent(agent).stepLimit ?? null,
					window: resolveModel().contextLength,
					project: p,
					dir,
					// Where its threads work now: its folder, else the one its agent keeps for it.
					cwd: projects.workDir(id),
					soul: fs.existsSync(soulFile) ? fs.readFileSync(soulFile, "utf8") : "",
					mcp: mcpView(agent),
					// Every skill its threads can use, with where it lives: its own, its folder's (Host: the machine's).
					skills: (await skillsOf(agent)).map((k) => ({ name: k.name, description: k.description, scope: k.scope, dir: k.dir })),
					prompts: fs.existsSync(path.join(dir, "prompts")) ? fs.readdirSync(path.join(dir, "prompts")).filter((f) => f.endsWith(".md")).map((f) => f.replace(/\.md$/, "")).sort() : [],
					mcp_json: fs.existsSync(path.join(dir, "mcp.json")) ? fs.readFileSync(path.join(dir, "mcp.json"), "utf8") : "",
					cron: jobs.list(agent).map(cronView),
					// The bot's token is shown whole (the person's own machine and bot). The state is
					// this agent's live view of the connection (Settings shows and Check refreshes it).
					telegram: p.telegram ? { username: p.telegram.username ?? "", allowed_users: p.telegram.allowed_users, has_token: !!p.telegram.token, token: p.telegram.token ?? "", proxy: p.telegram.proxy ?? "", status: telegramStatus.get(id === "general" ? "" : id) ?? null } : null,
				};
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/soul$/,
			(req, m) => {
				fs.writeFileSync(path.join(envOf(id1(m)).dir, "SYSTEM.md"), String(req.body?.text ?? ""));
				return { ok: true };
			},
		],
		// Its own Telegram bot: the token and who may write to it; "" token = none.
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/telegram$/,
			async (req, m) => {
				const { project: p } = envOf(id1(m));
				const token = String(req.body?.token ?? p.telegram?.token ?? "").trim();
				const allowed = (req.body?.allowed_users ?? p.telegram?.allowed_users ?? []).map(String).filter(Boolean);
				// Its own proxy URL; blank means the agent's env again.
				const proxy = String(req.body?.proxy ?? p.telegram?.proxy ?? "").trim();
				// The general bot stays a setting; a project's is the project's.
				if (p.id === "general") setSetting("telegram", token ? { ...(getSetting("telegram") ?? {}), token, allowed_users: allowed, proxy: proxy || undefined } : undefined);
				else projects.save({ ...(projects.get(p.id) as any), telegram: token ? { token, allowed_users: allowed, proxy: proxy || undefined } : null });
				// Check again: forget the bot so the sync below starts it anew and the state shows
				// what the settings do now, not what they did at boot.
				if (req.body?.retry) reconnectTelegram(p.id === "general" ? "" : p.id);
				await syncTelegram(rt);
				return { ok: true };
			},
		],
		// One language server of lsp.json: its entry and how it is doing; its entry written
		// (`{text}`: its JSON as edited, or `{spec}`: the object), new or changed.
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/lsp\/([^/]+)$/,
			async (_req, m) => {
				const id = id1(m);
				const { agent } = envOf(id);
				const name = decodeURIComponent(m[2]!);
				const spec = readLsp(agent)[name];
				if (!spec) throw new HttpError(404, "no such language server");
				const states = { ...lspStates(agent), ...(await lspNeeds(agent, projects.get(id)?.machine ?? undefined)) };
				return { name, spec, text: JSON.stringify(spec, null, 2), state: states[name] ?? "" };
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/lsp\/([^/]+)$/,
			(req, m) => {
				const { agent } = envOf(id1(m));
				const name = decodeURIComponent(m[2]!).trim();
				if (!name) throw new HttpError(400, "name it first");
				let spec;
				try {
					spec = parseLspSpec(req.body?.spec ?? String(req.body?.text ?? ""));
				} catch (e) {
					throw new HttpError(400, (e as Error).message);
				}
				writeLspServer(agent, name, spec);
				return { ok: true };
			},
		],
		// A language server out of lsp.json.
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)\/lsp\/([^/]+)$/,
			(_req, m) => {
				const agent = envOf(id1(m)).agent;
				const { [decodeURIComponent(m[2]!)]: _gone, ...rest } = readLsp(agent);
				writeLsp(agent, rest);
				return { ok: true };
			},
		],
		// Language servers: the whole lsp.json (text, as edited), or one named spec (a preset).
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/lsp$/,
			(req, m) => {
				const agent = envOf(id1(m)).agent;
				// A named spec, as Bench gives it from the person's presets.
				if (req.body?.name && req.body?.spec) {
					writeLsp(agent, { ...readLsp(agent), [String(req.body.name)]: req.body.spec });
					return { ok: true };
				}
				let parsed: any;
				try {
					parsed = JSON.parse(String(req.body?.text ?? ""));
				} catch (e) {
					throw new HttpError(400, `not JSON: ${(e as Error).message}`);
				}
				if (typeof parsed?.servers !== "object") throw new HttpError(400, "it needs a servers object");
				writeLsp(agent, parsed.servers);
				return { ok: true };
			},
		],
		// MCP: the whole mcp.json, one server's entry, on/off, gone, check, sign in.
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/mcp$/,
			(req, m) => {
				let parsed: any;
				try {
					parsed = JSON.parse(String(req.body?.text ?? ""));
				} catch (e) {
					throw new HttpError(400, `not JSON: ${(e as Error).message}`);
				}
				if (typeof parsed?.mcpServers !== "object") throw new HttpError(400, "it needs an mcpServers object");
				writeAgentMcp(envOf(id1(m)).agent, parsed.mcpServers);
				return { ok: true };
			},
		],
		[
			"POST",
			/^\/api\/projects\/([^/]+)\/mcp\/check$/,
			async (_req, m) => {
				// One reconnect answers for all of them: it replaces every entry of the agent.
				const { agent } = envOf(id1(m));
				try {
					await connectMcp(agent);
					rt.reloadMcp(agent);
				} catch (e) {
					console.error(`[mcp] check ${agent}: ${(e as Error)?.message ?? e}`);
				}
				return { ok: true, mcp: mcpView(agent) };
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/mcp\/([^/]+)\/spec$/,
			(req, m) => {
				const { agent } = envOf(id1(m));
				let spec: any;
				try {
					spec = JSON.parse(String(req.body?.text ?? ""));
				} catch (e) {
					throw new HttpError(400, `not JSON: ${(e as Error).message}`);
				}
				const servers = agentMcp(agent);
				servers[decodeURIComponent(m[2])] = spec;
				writeAgentMcp(agent, servers);
				return { ok: true };
			},
		],
		[
			"POST",
			/^\/api\/projects\/([^/]+)\/mcp\/([^/]+)\/authorize$/,
			async (_req, m) => {
				// The page to open; the server connects again once the browser comes back, and Bench
				// is told the project's environment changed.
				const project = id1(m);
				return { url: await authorizeServer(envOf(project).agent, decodeURIComponent(m[2]), () => publish({ type: "env_changed", project })) };
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/mcp\/([^/]+)$/,
			(req, m) => {
				const { agent } = envOf(id1(m));
				const name = decodeURIComponent(m[2]);
				const servers = agentMcp(agent);
				if (!servers[name]) throw new HttpError(404, "no such server");
				servers[name] = { ...servers[name], enabled: req.body?.enabled !== false };
				writeAgentMcp(agent, servers);
				return { ok: true };
			},
		],
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)\/mcp\/([^/]+)$/,
			(_req, m) => {
				const { agent } = envOf(id1(m));
				const servers = agentMcp(agent);
				delete servers[decodeURIComponent(m[2])];
				writeAgentMcp(agent, servers);
				return { ok: true };
			},
		],
		// Skills: read one, a new one pasted whole (its front matter names it), change, delete.
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/,
			async (_req, m) => {
				const skill = (await skillsOf(envOf(id1(m)).agent)).find((k) => k.name === decodeURIComponent(m[2]));
				if (!skill) throw new HttpError(404, "no such skill");
				return { name: skill.name, dir: skill.dir, scope: skill.scope, text: await readSkill(skill) };
			},
		],
		[
			"POST",
			/^\/api\/projects\/([^/]+)\/skills$/,
			(req, m) => {
				const text = String(req.body?.text ?? "");
				const name = text.match(/^---[\s\S]*?^name:\s*["']?([\w.-]+)["']?\s*$/m)?.[1];
				if (!name) throw new HttpError(400, "the skill needs front matter with name: <name>");
				const dir = path.join(envOf(id1(m)).dir, "skills", name);
				if (fs.existsSync(path.join(dir, "SKILL.md"))) throw new HttpError(409, `the project already has a skill "${name}"`);
				fs.mkdirSync(dir, { recursive: true });
				fs.writeFileSync(path.join(dir, "SKILL.md"), text);
				return { ok: true, name };
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/,
			async (req, m) => {
				const { agent, dir } = envOf(id1(m));
				const skill = (await skillsOf(agent)).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
				if (!skill) throw new HttpError(404, "no such skill of the project");
				fs.writeFileSync(path.join(skill.dir, "SKILL.md"), String(req.body?.text ?? ""));
				return { ok: true };
			},
		],
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)\/skills\/([^/]+)$/,
			(_req, m) => {
				const { agent, dir } = envOf(id1(m));
				const skill = listSkills(agent).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
				if (!skill) throw new HttpError(404, "no such skill");
				fs.rmSync(skill.dir, { recursive: true, force: true });
				return { ok: true };
			},
		],
		// Prompts a thread can insert (@ in the composer): the project's own (Host's for Host).
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/prompts$/,
			(_req, m) => {
				const id = id1(m);
				const read = (dir: string, own: boolean) =>
					fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith(".md")).map((f) => ({ name: f.replace(/\.md$/, ""), text: fs.readFileSync(path.join(dir, f), "utf8"), own })) : [];
				const own = read(path.join(envOf(id).dir, "prompts"), true);
				return { prompts: own.sort((a, b) => a.name.localeCompare(b.name)) };
			},
		],
		[
			"PUT",
			/^\/api\/projects\/([^/]+)\/prompts\/([^/]+)$/,
			(req, m) => {
				const dir = path.join(envOf(id1(m)).dir, "prompts");
				fs.mkdirSync(dir, { recursive: true });
				fs.writeFileSync(path.join(dir, `${path.basename(decodeURIComponent(m[2]))}.md`), String(req.body?.text ?? ""));
				return { ok: true };
			},
		],
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)\/prompts\/([^/]+)$/,
			(_req, m) => {
				fs.rmSync(path.join(envOf(id1(m)).dir, "prompts", `${path.basename(decodeURIComponent(m[2]))}.md`), { force: true });
				return { ok: true };
			},
		],
		// A new scheduled job of the project (the general environment's: "general").
		[
			"POST",
			/^\/api\/projects\/([^/]+)\/cron$/,
			async (req, m) => {
				const { agent } = envOf(id1(m));
				const name = String(req.body?.name ?? "").trim();
				const prompt = String(req.body?.prompt ?? "").trim();
				const schedule = String(req.body?.schedule ?? "").trim();
				if (!name || !prompt || !schedule) throw new HttpError(400, "name, prompt and schedule are required");
				let job: Job;
				try {
					job = newJob({ agent: agent, name, prompt, schedule });
				} catch (e) {
					throw new HttpError(400, `the schedule: ${(e as Error).message}`);
				}
				jobs.save(job);
				await arm(job);
				return cronView(jobs.get(job.id)!);
			},
		],
	],
});
