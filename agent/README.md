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
- **Its own code, about 5k lines**: the server and protocol (`src/server.ts`), the bridge from Pi's
  events to Bench's (`src/runtime.ts`), settings and the session index (`src/config.ts`,
  `src/store.ts`), the assistant extension (`src/extensions/assistant.ts`) with a short prompt
  (`src/prompt.ts`) and a few tools of its own (`src/builtin/`), the inserts (`src/inserts.ts`), MCP sign-in
  (`src/mcp-oauth.ts`), Telegram (`src/telegram.ts`, a bot per project, through a proxy where
  `TELEGRAM_PROXY` or `HTTPS_PROXY` is set).

## Extensions

An extension is Pi's unit of behaviour: tools, prompt sections, hooks, durable tasks. Built in:
`assistant` (the default), `mcp:<agent>` (the MCP servers from its `mcp.json`, Pi's format and
`exposure`, connected with its first session) and `tool-search`. Your own go in
`~/.bench/agents/default/extensions/<name>.ts` and are picked up without a restart: a changed file is imported
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

The agent picks its extensions in `~/.bench/agents/default/settings.json`:
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
| `tasks` | the person's tasks (in `bench.sqlite`): list, add, change, complete, sum the time spent |
| `ssh_run` | a command on this Mac or a `~/.ssh/config` server; without a host, the thread's terminal insert |
| `remote_files`, `hosts` | ls, read, write, delete, mkdir, rename on a file host (FTP, FTPS, SFTP); the project's hosts |
| `tool_search` + `mcp__<server>__<tool>` | MCP servers; a server with many tools is deferred behind tool_search |

Web search comes from the official MCP servers of Tavily or Brave, added to an agent's `mcp.json`.

## Inserts, in Pi's terms

A thread is a Pi conversation; what the person puts into it with `/` is kept in it
(`src/inserts.ts`): an entry `bench.insert {id, key}` in the transcript where it was put (a write,
so the model doesn't see it and it is never queued), and its live state in the conversation's
document `bench.inserts`, changed in commits. Bench calls `session.insert` and
`session.insert.patch`; `session.resume` returns the inserts with their place. The `inserts`
prompt section reads the document and says, one line per insert, what it is and which tool acts
on it (`src/prompt.ts` `inserted()`); what each shows at that moment comes with the message as
`<block-state>`, kept out of the history.

A message sent while a run is live waits in Pi's inbox (`session.queue`); "send now" aborts it
and steers. Pi 1.4 leaves an aborted item in the inbox, so `dropFromInbox` takes it out in a
commit.

## Projects and their environment

A project is a row in `bench.sqlite` (name, an optional folder, an optional server, a Telegram
bot). Its environment is laid out like Pi's agent directory, in `~/.bench/projects/<id>/`:
`SYSTEM.md` (its soul), `mcp.json`, `skills/`, `prompts/`. Its threads share the machine's own
agent, `~/.bench/agents/default/` (the "Host-Mac" project in Bench): model, memory, standing instructions.
MCP servers that need OAuth sign in through `src/mcp-oauth.ts` (callback on `localhost:47823`,
credentials in `~/.bench/mcp-oauth/`).

## Its home: `~/.bench`

`bench.sqlite` (settings, keys, providers, projects, sessions, tasks, hosts, cron jobs),
`pi.sqlite` (the conversations), `agents/default/` and `projects/<id>/` (laid out like Pi's agent
directory, plus `memories/`), `app/` (the installed server), `server.json` (port, token),
`server.log`, `backups/`.

## Run by hand

```bash
npm ci
node src/server.ts                         # writes ~/.bench/server.json
node scripts/probe.ts "What time is it?"   # a turn through the protocol
node --test test/*.test.ts
```

Node 22.18+ (it runs the TypeScript directly; no parameter properties, enums or namespaces).
