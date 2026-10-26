// Projects and their environment: the soul, MCP servers, skills, prompts, accesses, the
// Telegram bot, the schedule. "general" is the Host project: the agent's own environment.

import fs from "node:fs";
import path from "node:path";
import { agentDir, agentMcp, getSetting, setSetting, writeAgentMcp } from "../config.ts";
import { arm, jobs, newJob, type Job } from "../builtin/cron.ts";
import { hosts } from "../builtin/hosts.ts";
import { projects } from "../builtin/projects.ts";
import { listSkills as skillsOf } from "../builtin/skills.ts";
import { HttpError, type Ctx, type Module } from "../http.ts";
import { authorizeServer, connectMcp, mcpStatus } from "../mcp.ts";
import { syncTelegram } from "../telegram.ts";
import { cronView } from "./cron.ts";

/** A project's environment: the agent its threads run with, its folder, the project row. */
function envOf(id: string): { agent: string; dir: string; project: { id: string; name: string; telegram?: { token: string; allowed_users: string[]; username?: string } | null } } {
	if (id === "general") {
		const tg = getSetting("telegram");
		return { agent: "default", dir: agentDir("default"), project: { id, name: "Host", telegram: tg?.token ? { token: tg.token, allowed_users: (tg.allowed_users ?? []).map(String) } : null } };
	}
	const p = projects.get(id);
	if (!p) throw new HttpError(404, "no such project");
	return { agent: `project:${id}`, dir: projects.env(id), project: p };
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

export default ({ rt }: Ctx): Module => ({
	routes: {
		// `browser`: the project has the watchable server browser among its MCP servers (Bench
		// offers /browser only there); `host_browser`: the machine's own agent has it.
		"GET /api/projects": () => ({
			projects: projects.list().map((p) => ({ ...p, browser: !p.host && "browser" in agentMcp(`project:${p.id}`) })),
			host_browser: "browser" in agentMcp("default"),
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
				const prev = projects.get(id1(m));
				if (!prev) throw new HttpError(404, "no such project");
				return { project: projects.save({ ...prev, ...req.body, id: prev.id }) };
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
		// Everything the environment thread shows, in one read.
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/env$/,
			(_req, m) => {
				const id = id1(m);
				const { agent, dir, project: p } = envOf(id);
				const soulFile = path.join(dir, "SYSTEM.md");
				return {
					project: p,
					dir,
					soul: fs.existsSync(soulFile) ? fs.readFileSync(soulFile, "utf8") : "",
					mcp: mcpView(agent),
					// Every skill its threads can use, with where it lives: its own, its folder's (Host: the machine's).
					skills: skillsOf(agent).map((k) => ({ name: k.name, description: k.description, scope: k.scope, dir: k.dir })),
					prompts: fs.existsSync(path.join(dir, "prompts")) ? fs.readdirSync(path.join(dir, "prompts")).filter((f) => f.endsWith(".md")).map((f) => f.replace(/\.md$/, "")).sort() : [],
					mcp_json: fs.existsSync(path.join(dir, "mcp.json")) ? fs.readFileSync(path.join(dir, "mcp.json"), "utf8") : "",
					hosts: hosts.list().filter((h) => h.project === (id === "general" ? "" : id)).map((h) => ({ id: h.id, name: h.name, kind: h.kind, host: h.host, user: h.user, port: h.port, path: h.path })),
					cron: jobs.list(agent).map(cronView),
					// The bot's token is shown whole (the person's own machine and bot).
					telegram: p.telegram ? { username: p.telegram.username ?? "", allowed_users: p.telegram.allowed_users, has_token: !!p.telegram.token, token: p.telegram.token ?? "" } : null,
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
				// The general bot stays a setting; a project's is the project's.
				if (p.id === "general") setSetting("telegram", token ? { ...(getSetting("telegram") ?? {}), token, allowed_users: allowed } : undefined);
				else projects.save({ ...(projects.get(p.id) as any), telegram: token ? { token, allowed_users: allowed } : null });
				await syncTelegram(rt);
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
				// The page to open; the server connects again once the browser comes back.
				return { url: await authorizeServer(envOf(id1(m)).agent, decodeURIComponent(m[2])) };
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
			(_req, m) => {
				const skill = skillsOf(envOf(id1(m)).agent).find((k) => k.name === decodeURIComponent(m[2]));
				if (!skill) throw new HttpError(404, "no such skill");
				return { name: skill.name, dir: skill.dir, scope: skill.scope, text: fs.readFileSync(path.join(skill.dir, "SKILL.md"), "utf8") };
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
			(req, m) => {
				const { agent, dir } = envOf(id1(m));
				const skill = skillsOf(agent).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
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
				const skill = skillsOf(agent).find((k) => k.name === decodeURIComponent(m[2]) && k.dir.startsWith(dir));
				if (!skill) throw new HttpError(404, "no such skill");
				fs.rmSync(skill.dir, { recursive: true, force: true });
				return { ok: true };
			},
		],
		// Prompts a thread can insert (@ in the composer): the general ones and the project's own.
		[
			"GET",
			/^\/api\/projects\/([^/]+)\/prompts$/,
			(_req, m) => {
				const id = id1(m);
				const read = (dir: string, own: boolean) =>
					fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith(".md")).map((f) => ({ name: f.replace(/\.md$/, ""), text: fs.readFileSync(path.join(dir, f), "utf8"), own })) : [];
				const own = read(path.join(envOf(id).dir, "prompts"), true);
				const general = id === "general" ? [] : read(path.join(agentDir("default"), "prompts"), false).filter((g) => !own.some((o) => o.name === g.name));
				return { prompts: [...own, ...general].sort((a, b) => a.name.localeCompare(b.name)) };
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
		// An access taken out of the project. A file host belongs to the project that made it, so it
		// is deleted with its password; an SSH row mirrors ~/.ssh/config and only loses the tag.
		[
			"DELETE",
			/^\/api\/projects\/([^/]+)\/hosts\/([^/]+)$/,
			(_req, m) => {
				const h = hosts.get(decodeURIComponent(m[2]));
				// The Host project's env is "general"; its accesses are the ones with no project tag.
				const want = id1(m) === "general" ? "" : id1(m);
				if (h && h.project === want) {
					if (h.kind === "ssh") hosts.save({ ...h, project: "" });
					else hosts.remove(h.id);
				}
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
					job = newJob({ profile: agent, name, prompt, schedule });
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
