# Bench's agent

The agent behind Bench: a small Node server on [Pi Durable](https://github.com/earendil-works/pi)
(`@earendil-works/pi-durable`). Pi runs the conversations and keeps every step on disk, so a turn
survives a crash or a restart and continues where it stopped. Bench installs the server, starts it,
and talks to it over a WebSocket and a REST API; over SSH it does the same on another machine.

## What it is made of

- **Pi Durable, used as its authors describe it**, unmodified: conversations, extensions, tools,
  hooks, durable tasks, the SQLite storage, compaction, Pi's coding tools (`read`, `write`, `edit`,
  `bash`) and its `tool_search` for deferred tools.
- **Official and popular packages for the rest**: `@earendil-works/pi-mcp` (MCP servers),
  [grammY](https://grammy.dev) (Telegram), [croner](https://croner.56k.guru) (cron expressions).
- **Its own code, about 3k lines**: the server and protocol (`src/server.ts`), the bridge from Pi's
  events to Bench's (`src/runtime.ts`), settings and the session index (`src/config.ts`,
  `src/store.ts`), the assistant extension (`src/extensions/assistant.ts`) with a short prompt
  (`src/prompt.ts`) and a few tools of its own (`src/builtin/`), Telegram (`src/telegram.ts`).

## Extensions

An extension is Pi's unit of behaviour: tools, prompt sections, hooks, durable tasks. Built in:
`assistant` (the default), `mcp:<agent>` (that agent's MCP servers from its `mcp.json`, Pi's format and `exposure`, connected with its first session) and `tool-search`. Your own go in the agent's
`~/.bench/agents/<agent>/extensions/<name>.ts` and are picked up without a restart: a changed file is imported
again and Pi replaces the extension in place (new turns use the new code). A file exports a
function that receives the building blocks, so it needs no packages of its own:

```ts
export default ({ pi, tools, prompt }) =>
	pi.defineExtension({
		name: "research",
		tools: [tools.read, tools.bash, tools.delegate],
		sections: [pi.section("role", () => "You research a question with sources and answer briefly.")],
	});
```

An agent picks its extensions in `~/.bench/agents/<name>/settings.json`:
`{ "defaultProvider": "dev", "defaultModel": "…", "extensions": ["research"] }`.

## Tools of the assistant

| Tool | What it does |
|---|---|
| `read`, `write`, `edit`, `bash` | Pi's coding tools, in the session's folder |
| `clarify` | a question with choices (Bench's card, Telegram's buttons) |
| `todo` | the plan Bench shows over the composer |
| `memory` | notes kept across sessions (`memories/MEMORY.md`, `USER.md`), only when asked |
| `skill_view`, `skill_manage` | the agent's skills (`skills/<…>/SKILL.md`), listed in the prompt |
| `cron` | scheduled prompts: durable Pi tasks that sleep until their time; answers go to Telegram or the app |
| `delegate` | subagents: child conversations owned by the call, run in parallel |
| `session_search` | the text of past sessions |
| `ssh_run` | a command on the server a thread is about (`ssh:<alias>`) |
| `remote_files` | ls, read, write, delete, mkdir, rename on a host's files (FTP, FTPS, SFTP) in a thread about them (`files:<host id>`) |
| `tool_search` + `mcp__<server>__<tool>` | MCP servers; a server with many tools is deferred behind tool_search |

Web search comes from the official MCP servers of Tavily or Brave, added to an agent's `mcp.json`.

## Its home: `~/.bench`

`bench.sqlite` (settings, keys, providers, sessions, cron jobs), `pi.sqlite` (the conversations),
`agents/<name>/` — an agent, laid out like Pi's agent directory (`settings.json`, `mcp.json`, `SYSTEM.md`, `AGENTS.md`, `skills/`, `prompts/`, `extensions/`, plus `memories/`), `app/` (the
installed server), `server.json` (port, token), `server.log`. On the first start it imports from
`~/.hermes` once: settings, keys, providers, each profile as an agent (SOUL.md → SYSTEM.md, its own MCP servers), notes, the person's own skills (none that ship
with Hermes), cron jobs (paused).

## Run by hand

```bash
npm ci
node src/server.ts                         # writes ~/.bench/server.json
node scripts/probe.ts "What time is it?"   # a turn through the protocol
node --test test/*.test.ts
```

Node 22.18+ (it runs the TypeScript directly; no parameter properties, enums or namespaces).
