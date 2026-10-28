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
- **Its own code, about 9k lines**: the server and its modules, each with its RPC and REST routes
  (`src/server.ts`, `src/modules/`; the wire is written from `crates/protocol` into
  `src/protocol.gen.ts`), the threads' core and the bridge from Pi's events to Bench's
  (`src/runtime/`), settings and the session index (`src/config.ts`, `src/store.ts`), the
  assistant extension (`src/extensions/assistant.ts`) with a short prompt (`src/prompt.ts`) and
  tools of its own (`src/builtin/`), where tools work (`src/machine/`: a server over SSH, a
  project kept in its folder), language servers (`src/lsp/`), MCP and its sign-in (`src/mcp/`),
  Telegram (`src/telegram.ts`, a bot per project, through a proxy where `TELEGRAM_PROXY` or
  `HTTPS_PROXY` is set).

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
| `read`, `write`, `bash` | Pi's coding tools, on the project's machine, in its folder; `read` shows every line as `LINE:HASH\|text` |
| `edit` | changes lines by those anchors (`src/builtin/hashline.ts`); with `oldText`/`newText` it is Pi's own edit |
| `grep`, `glob`, `ls` | search the project's files, through the same machine |
| `lsp`, `diagnostics` | definitions, references, outline, hover and problems from the project's language servers; edits come back with their diagnostics |
| `clarify` | a question with choices (Bench's card, Telegram's buttons) |
| `todo` | the plan Bench shows over the composer |
| `memory` | notes kept across sessions: the project's `memories/MEMORY.md`, the shared `USER.md`; only when asked |
| `skill_view`, `skill_manage` | the agent's skills (`skills/<…>/SKILL.md`), listed in the prompt |
| `cron` | scheduled prompts: durable Pi tasks that sleep until their time; answers go to Telegram or the app |
| `delegate` | subagents: child conversations owned by the call, run in parallel |
| `process` | a command in the background, its output's tail kept, its exit the outcome |
| `session_search` | the text of past sessions |
| `ssh_run` | a command on this Mac or a `~/.ssh/config` server; without a host, on the project's machine or the thread's terminal insert |
| `tool_search` + `mcp__<server>__<tool>` | MCP servers; a server with many tools is deferred behind tool_search |

Web search comes from the official MCP servers of Tavily or Brave, added to an agent's `mcp.json`.

## Inserts, in Pi's terms

A thread is a Pi conversation; what the person puts into it with `/` is kept in it
(`src/runtime/inserts.ts`): an entry `bench.insert {id, key}` in the transcript where it was put
(a write, so the model doesn't see it and it is never queued), and its live state in the
conversation's document `bench.inserts`, changed in commits. Bench calls `thread.insert` and
`thread.insert.patch`; `thread.open` returns the inserts with their place. The `inserts`
prompt section reads the document and says, one line per insert, what it is and which tool acts
on it (`src/prompt.ts` `inserted()`); what each shows at that moment comes with the message as
`<block-state>`, kept out of the history.

A message sent while a run is live waits in Pi's inbox (`src/runtime/queue.ts`): it can be taken
back, edited, or sent now, which aborts the run and steers.

## Projects and their environment

A project is a row in `bench.sqlite` (name, an optional folder, an optional server, a Telegram
bot). Its environment is laid out like Pi's agent directory, in `~/.bench/projects/<id>/`:
`SYSTEM.md` (its soul), `mcp.json`, `lsp.json`, `skills/`, `prompts/`, `memories/MEMORY.md`. Its
threads share the machine's own agent, `~/.bench/agents/default/` (the "Host" project in Bench):
model, `USER.md`, standing instructions. A project's settings stay on this Mac even when its
work is on a server; the file tools reach them there (`src/machine/settings.ts`). MCP servers that
need OAuth sign in through `src/mcp/oauth.ts` (callback on `localhost:47823`, credentials in
`~/.bench/mcp-oauth/`).

## Its home: `~/.bench`

`bench.sqlite` (settings, keys, providers, projects, the thread index and its search, cron jobs),
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
