# AGENTS.md: Bench

Read this first. Read the skill `.agents/skills/bench-kit/SKILL.md` before any UI or insert work.

## What Bench is

Bench is a native desktop **workbench for working with an AI agent**. It is an open,
thread-centric app, written from scratch in Rust and GPUI
(gpui-kit 0.8). It is a client of its own agent in `agent/`, a Node server on **Pi Durable**.

Bench is not a kit for building other apps, and it has nothing to do with Hermes. Hermes was an
earlier agent, now gone; a few wire names (the `X-Hermes-Session-Token` header, "profile" in the
protocol) are kept only so older agents on servers keep answering. There are no profiles for the
person: there are projects.

## The concepts (keep them exact)

- **Thread**: a document of **messages** (the person's, the agent's, with its steps) and the
  person's **inserts**. One conversation in Pi. It never switches mode or gets renamed because of
  an insert.
- **Insert**: a small live tool put into a thread with `/`. The words are `/terminal`, `/ssh`,
  `/files`, `/ftp`, `/tasks`, `/tools`, `/browser`. Every `/` adds a new insert where the
  conversation is. An insert:
  - shows something live (a terminal, a folder, a host, a list, a utility, a browser tab);
  - tells the agent what it shows with every message (`<block-state>`, kept out of the history);
  - has an agent tool that acts on it;
  - is restored exactly on return.

  Kinds that can be live only once (the browser, the local folder) show "click to show it here"
  on the older ones.
- **Card**: a form or an editor that appears in the thread as an agent-message block, under the
  insert or section that asked for it. Examples: a new server, a task, an MCP server, a skill, a
  cron job, an open file. Bench has no settings pages for such jobs.
- **Project**: the sidebar's only unit. It may have a folder (threads work in it; its ⋯ menu has
  "Open in Zed") and may live on a server (its agent runs there, over SSH). Its **environment**:
  - soul (`SYSTEM.md`);
  - MCP servers (`mcp.json`, OAuth where needed);
  - skills, prompts;
  - accesses (SSH and file hosts);
  - Telegram bot;
  - schedule (cron).

  It is edited in its environment thread, one agent message per section, with the card under the
  section being edited. **Host** is this machine's own project (always first in the sidebar) (installing, building,
  looking after the machine). A server project (`host` set, e.g. Dev) runs entirely on its
  server: its agent there keeps the threads, skills, MCP, schedule; `/terminal` (`ssh:local`) is
  the thread's machine (`thread_machine()`), so it opens on that server.
- **The gear** holds the only classic settings: providers and the look.

## The code map

| Where | What |
|---|---|
| `src/main.rs`, `src/bench.rs`, `src/theme.rs`, `src/icons.rs` | Start, flags, theme (radius 4 / 6), Lucide icons (`assets/icons/`) |
| `src/app.rs`, `src/app/*.rs` | The app state (`BenchApp`) and its logic, one file per area |
| `src/ui/*.rs` | Rendering: `mod.rs` (shared pieces: `block_head`, `FIELD_H`, `editor_height`, `file_icon`, `form_action`), `sidebar.rs`, `chat.rs` (the thread), `composer.rs`, `statusbar.rs`, `project_list.rs` (environment sections) |
| `src/ui/inserts/` | **The insert registry**: `mod.rs` (the `InsertKind` trait, `KINDS`, `words()`, `card`), one file per kind: `terminal`, `dir` (/files), `files` (/ftp), `tasks`, `tools`, `browser` |
| `src/app/inserts.rs` | Putting, patching, removing inserts; their state sync; the context sent with a message |
| `src/gateway.rs`, `src/rest.rs`, `src/connection.rs`, `src/pi_agent.rs` | The protocol to the agent (WebSocket JSON-RPC + REST), installing/starting it locally or over SSH |
| `src/app/subjects.rs`, `src/app/slash.rs` | The `/` words and what they put in |
| `src/tasks.rs`, `src/task_store.rs`, `src/ssh*.rs`, `src/term.rs`, `src/remote_browser.rs`, `src/devtools/` | The tools' own engines: tasks, SSH config and terminals, the browser gateway client, 19 utilities as tested pure functions |
| `src/demo.rs` | A made-up session for screenshots and the web demo (`BENCH_DEMO=1\|tasks\|server\|tool\|env`) |
| `agent/src/server.ts` | The agent's server: RPC and REST routes |
| `agent/src/runtime.ts` | Pi conversations to Bench's events; the queue; inserts |
| `agent/src/inserts.ts` | The insert entry and document |
| `agent/src/prompt.ts`, `agent/src/extensions/assistant.ts` | The prompt sections and the default extension (tools, sections, hooks) |
| `agent/src/builtin/` | The agent's own tools: `tasks`, `ssh`, `remote`, `hosts`, `cron`, `subagent`, `memory`, `skills`, `ask`, `projects` |
| `agent/src/mcp.ts`, `mcp-oauth.ts`, `telegram.ts` | MCP servers (pi-mcp, OAuth), Telegram bots (grammY, a proxy where needed) |

## How the agent uses Pi Durable

Pi is used unmodified, as its authors describe it. Our code stays in Pi's terms:

| Piece | In Pi's terms |
|---|---|
| Thread | a Pi conversation in `~/.bench/pi.sqlite`; every step durable, a turn survives a restart |
| Insert | an entry `bench.insert {id, key}` written where it was put (a write: the model doesn't see it, never queued) + its state in the conversation document `bench.inserts` `{items:{id:{key,state,folded,removed,after?}}}`, changed in commits. `after` moves one whose entry's place is not where it belongs: an insert put while a message waits in the composer goes after that message. RPC `session.insert`, `session.insert.patch`; `session.resume` returns them |
| What the agent knows of inserts | the `inserts` prompt section, read with `input.read.snapshot(Inserts, conversationId)`, one line per kind (`prompt.ts` `inserted()`), plus `<block-state>` with each message |
| Prompt | Pi sections (soul, environment, inserts, project, skills, notes), resent only when changed |
| Queue | Pi's inbox: a message sent while a run is live is a follow-up; "send now" aborts it and steers. Pi 1.4 leaves an aborted item in the inbox, so `dropFromInbox` removes it in a commit |
| `/tasks` | tool `tasks` (`builtin/tasks.ts`), data in `bench.sqlite`; one list for all projects, a project is a tag (`slug` of its name), the default filter in its threads |
| `/terminal`, `/ssh` | tool `ssh_run` (`builtin/ssh.ts`); without a host it takes the thread's terminal insert |
| `/ftp` | tools `remote_files`, `hosts` (`builtin/remote.ts`, `hosts.ts`); without a host it takes the thread's insert |
| `/files` | Pi's coding tools (`read`, `write`, `edit`, `bash`) in the project's folder; Bench reads the folder through `/api/fs/…` |
| `/tools` | no tool of its own: the utility's input and output come in `<block-state>` |
| `/browser` | the Playwright MCP through `@earendil-works/pi-mcp` (`mcp__browser__*`), one tab per thread (state `{tab,url}`). The server browser is its own project, `~/Projects/browser-server-mcp` (github.com/krutovvv/browser-server-mcp) (an MCP server with a live view); Bench is one of its clients (`src/remote_browser.rs`) |
| Model | one for every thread of every project, set in the gear: the general agent's `defaultProvider`/`defaultModel`; a project has no model of its own. The fallback (`fallback_providers[0]`) is a pi-ai provider of ours (`fallback.ts`): the main model's error or silence sends the same request to it (Pi retries only with the same model), and `session.info {fallback}` turns the status bar red |
| Schedule | a durable `defineTask` per job that sleeps until its next run (`builtin/cron.ts`) |
| Subagents | `delegate`: child conversations owned by the tool's task (`builtin/subagent.ts`) |
| MCP | `@earendil-works/pi-mcp`, a project's `mcp.json`; OAuth via `mcp-oauth.ts` (callback on `localhost:47823`) |
| Telegram | grammY, one bot per project, the same Pi conversations; `TELEGRAM_PROXY` or `HTTPS_PROXY` in the agent's env |

Extend with Pi's packages and popular ones before writing your own. When something in Pi is
missing or buggy, work around it in our code and say so in a comment. Never patch Pi.

## How to work here

- **Bench develops itself, in one app.** The person works in `/Applications/Bench.app` and talks
  to you through it. It is only a client: threads, the queue and the agent live in the agent
  process and in `~/.bench`, so rebuilding and reopening it costs a few seconds and loses
  nothing — an unfinished run goes on in the agent and the window comes back to it.
- **One change, one commit; a build only when the person asks.** Commit every change, then go on
  to the next one: the app is not rebuilt per commit. When the person asks for a build ("пересобери",
  "обнови", "ship") — usually once, at the end of a batch of changes — run
  `.agents/skills/bench-kit/scripts/ship.sh`: it quits Bench the honest way (`osascript … quit`,
  never a kill), rebuilds `/Applications/Bench.app` in place and opens it again. Uncommitted
  changes stop it. The way back from a bad change is an earlier commit, rebuilt — there is no
  second app parked beside it. `ship.sh --dev` builds `dist/Bench Dev.app` only when two states
  of the interface must be compared; it shares `~/.bench` with Bench and isolates nothing but a
  window. Both are signed with the Apple Development certificate, so macOS asks for each
  permission once.
- **Never kill Bench or quit it outside `ship.sh`** (`pkill`, deleting its `.app`, an ad-hoc
  quit): it is the window the person works in.
- **Don't push without asking.** The repository is public: no private names, tokens or the
  person's data in commits.
- **Back up before deleting data** (`~/.bench/backups/`).
- **Screenshots only of your own copy** (`.agents/skills/bench-kit/scripts/shot.sh`, with
  `BENCH_DEMO=…` or `BENCH_HOME=<a copy>`), never the person's window.
- **Disk space is tight.** Clean `target/` (`cargo clean --profile dev`) before it fills.
- **Checks.** Run `cargo build` (and `cargo test` for logic), and
  `cd agent && node --test test/*.test.ts` for agent changes.

## The look, in short

The look is Zed's: flat and precise, with one accent. Never invent a look or a control; assemble
from the pieces in the skill's UI catalog.

- **Blocks.** Every bordered block is rounded (`rounded_md`, theme radius). Every block head is
  `block_head` (32 px). Fields are `FIELD_H` (32 px). Buttons are rounded and consistent.
- **Scrolling.** Only the thread scrolls. There is no scroll inside a scroll, except the
  terminal. Editors are as tall as their text (`editor_height`). Lists are as long as they are.
  Empty states take one line.
- **Messages.** In the thread there is one font size, the messages'. Copy is an icon.
- **One place for each change.** A change to how blocks look goes to all of them at once.
