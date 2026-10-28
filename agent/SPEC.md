---
id: agent
type: module-design
status: draft
title: agent/ — Bench's own agent, a Node server on Pi Durable
parent: architecture
depends-on: [protocol]
references: [projects-and-agents, thread-objects]
---

# The agent

The process that runs threads. Bench is its client and nothing else is: this Mac always has one,
and a server has one when the person adds it in the gear (Bench installs and starts it over SSH,
`client`). Each agent has its own models, voice and general Telegram bot; this Mac's also keeps
the list of servers' agents.

## Boundary

- **Pi Durable does the work, unmodified and in Pi's terms**: a thread is a Pi conversation in
  `~/.bench/pi.sqlite`, every step durable. How each Bench concept maps onto Pi — inserts,
  queue, plan, pause, code mode, fork, compaction, processes, subagents, MCP, Telegram — is the
  table "How the agent uses Pi Durable" in `AGENTS.md`; it is not repeated in the specs. What Pi
  lacks is worked around here and said so in a comment (the pause, the fallback model, background
  processes); Pi is never patched.
- **The wire is `protocol`'s.** The agent imports its types from the generated
  `src/protocol.gen.ts` and never defines a message shape of its own. JSON-RPC over
  `/api/ws?token=`, REST under `/api/` with `X-Bench-Token`.
- **Its home is `~/.bench`** (`BENCH_HOME`; laid out in `config.ts`): `bench.sqlite` (settings,
  keys, providers, the projects and the session index), `pi.sqlite` (Pi's conversations),
  `agents/<name>/` laid out as Pi's agent directory, `projects/<id>/` (each project's
  environment files), cron, attachments, cache, and `app/` (the installed server).

## How it is cut

- `server.ts` only starts and collects `modules/`: each module declares its RPC handlers and REST
  routes (`http.ts` `Module`) for one area — threads, projects, agents, presets, cron, files,
  language servers, audio, settings, backup, commands.
- `runtime/` — the threads' core (`agent-runtime`).
- `builtin/` — the agent's own tools and the stores behind them (`agent-tools`).
- `machine/` — where the tools work (`agent-machine`).
- `extensions.ts`, `extensions/assistant.ts` — Pi extensions: the default "assistant" (tools,
  prompt sections, hooks), the MCP servers' tools, and the person's own per agent, reloaded
  without a restart.
- `prompt.ts` — the system prompt as Pi sections (soul, environment, inserts, project, skills,
  notes), each resent only when it changed so the provider's prompt cache stays warm, and short on
  purpose: a local model pays for every line.
- `mcp/`, `lsp/`, `telegram.ts` — MCP servers (pi-mcp, OAuth), a project's language servers,
  the Telegram bots (grammY). MCP command servers and language servers of a project on a server
  run there, over the same SSH connection.

Edges: `modules` → `runtime` → `extensions.ts` → `builtin`; the runtime also uses a few of
`builtin`'s stores directly (projects, cron, processes). Tools reach the server only through
`host.ts` (kept light because every tool imports it), and the runtime installs itself there at
start. `runtime/tools-host.ts` is the one place that picks a conversation's machine.

## Invariants

- Nothing that the thread shows is kept outside Pi (`thread-objects`): the session index in
  `bench.sqlite` (titles, sources, the words searched) is built from Pi's commits, never written
  beside them.
- The agent tells Bench what changed (`Event`); it never expects to be polled.
- Tests: `cd agent && node --test test/*.test.ts`.
