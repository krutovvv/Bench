# AGENTS.md: Bench

Read this first. Then `docs/map.md` to find the file for your change — open that file, not its
neighbours. Read the skill `.agents/skills/bench-kit/SKILL.md` before any UI or insert work, and
`.agents/skills/bench-structure/SKILL.md` (in Russian) before adding a feature, a module, a file
or a new kind of thing, and before any refactor: structure fitting where Bench goes first, then a
file tree readable without docs, then Rust and Zed words.

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
  an insert. Bench shows Pi's journal of it as rows (`crates/thread`), on a list that **nothing
  moves but the person** — a run that ends, a row that grows, an answer that arrives never scroll
  it (`docs/thread-rewrite.md`).
- **Insert**: a small live tool put into a thread with `/`. The words are `/terminal`, `/tasks`,
  `/tools`, `/browser`. Every `/` adds a new insert where the conversation is. An insert:
  - shows something live (a terminal, a list, a utility, a browser tab);
  - tells the agent what it shows with every message (`<block-state>`, kept out of the history);
  - has an agent tool that acts on it;
  - is restored exactly on return.

  Kinds that can be live only once (the task list, the utilities) show "click to show it here"
  on the older ones.
- **Card**: a form or an editor that appears in the thread as an agent-message block, under the
  insert that asked for it (a task). A project's parts that are not files (an MCP server, a cron
  job) open as documents in the Explorer instead.
- **Agent**: a process of `agent/` that runs threads. This Mac's always; a server's when the
  person adds it in the gear (Bench installs and starts it over SSH). The Mac's agent keeps the
  list of servers' agents. Each agent has its own models, voice and general Telegram bot.
- **Project**: the sidebar's only unit. It belongs to one agent and says where its work is
  (`docs/remote-projects.md`):
  - **agent**: this Mac's or a server's;
  - **machine**: the agent's own, or — with this Mac's agent — a server it works on over SSH
    (nothing installed there);
  - **folder** on that machine, optional; its agent can be **kept in it** (tools refuse paths
    outside, commands cannot write outside).

  In code a project is a `ProjectKey` (agent, id there) and where it lives a `Location`;
  `src/app/places.rs` answers "where" for everything else. Its **environment**:
  - soul (`SYSTEM.md`);
  - MCP servers (`mcp.json`, OAuth where needed), language servers (`lsp.json`);
  - skills, prompts;
  - model: reasoning, approvals, compaction;
  - Telegram bot;
  - schedule (cron).

  What it *is set to* (name, where it lives, model, Telegram) is its **Settings** (the last row of its Explorer); what it
  *contains* (its folder, soul, prompts, skills, MCP and language servers, schedule) is the
  **Explorer** (`docs/explorer.md`). The agent changes the same files when asked, from any
  thread of the project. **Host** is an agent's own environment: this Mac's is always first in the
  sidebar (installing, building, looking after the machine); a server's (e.g. Dev) names that
  server and is followed by the server's own projects. `/terminal` opens on the thread's machine
  (`thread_machine()`).
- **Explorer**: the third way to look at a project, beside threads and Settings — its files and
  the agent's texts as an IDE shows them (a tree in the sidebar, documents in tabs, the thread
  beside). It is the familiar view of a project, built into Bench: no external editor is
  referenced or needed.
  The window is Zed's workspace: a left dock (threads or Explorer), the center, a right dock.
  Two threads can sit side by side: a thread's menu in the sidebar has "Open beside".
- **The gear** (Bench → Settings… in the macOS menu, ⌘,) holds the app's settings, in tabs: Agents (each one's models and voice;
  add and remove servers' agents), Presets (MCP and language servers a project takes with one
  click, compaction shares, reasoning levels), Appearance.

## The code map

**Where is X: `docs/map.md`** (one table of common changes and the file for each). Why it is
built this way: `docs/architecture.md`. In short:

| Where | What |
|---|---|
| `crates/ui` (`bench_ui`) | Every element and size: theme (radius 4 / 6), Lucide icons, `Gap`, text, buttons, fields, blocks, rows, forms, lists, bars. The only place with pixel values |
| `crates/storybook` | All elements on one page (`cargo run -p storybook`); `reference/` has the pictures |
| `crates/protocol` | `thread.open` and `thread.*` types; `cargo test -p protocol` writes `agent/src/protocol.gen.ts` |
| `crates/thread` | The thread as rows from Pi's journal; the rules of order are tests |
| `crates/devtools` | The /tools utilities as tested pure functions |
| `src/main.rs`, `src/app.rs` | Start; `BenchApp`: the window's state and its construction. Only these two are loose in `src/`: every other file is in the folder named for where it is in the window or what it does |
| `src/app/` | What belongs to the whole app: `model.rs` (plain data: `ProjectKey`, `Location`, …), `events.rs` (the agent's events applied), `keys.rs` (shortcuts), `menu.rs` (the macOS menu: Settings…, New Window), `connect.rs`, `subscriptions.rs` (what `new` listens to, its timers), `backend.rs`, `projects.rs` (git) |
| `src/app/places.rs`, `src/app/agents.rs` | **Projects and agents**: where each project lives, the sidebar's projects from their sources; the links to this Mac's and servers' agents |
| `src/client/` | Talking to the agent, as Zed's `client`: `rpc/` (WebSocket JSON-RPC: `Cmd` → `Evt`), `rest/` (REST: files, providers), `connection.rs` (where it runs, the tunnel), `launch.rs` (installing and starting it locally or over SSH) |
| `src/workspace/` | The window as Zed's workspace: `dock.rs` the docks as Zed's (`Dock`: a list of `Panel`s, one shown; `left_dock` threads or Explorer, `right_dock` a thread beside the center (the open one beside a document, a second one beside a thread: `src/thread/beside.rs`), `bottom_dock` the project's terminal, `src/inserts/terminal/panel.rs`), `mod.rs` the `Center` (thread, document, settings) and `render_panel`; `layout.rs` draws them around the center |
| `src/titlebar/` | The bar over the center: the thread's title and branch, the documents' project with the thread-beside button, a settings page's title |
| `src/sidebar/` | The threads panel: projects (`project_row.rs`, `project_list.rs`), threads (`thread_row.rs`), the menu, what is done to an item (`items.rs`: pin, color, rename, ⌫⌫, ↑↓; `marks.rs`), the strip over it (`top.rs`) |
| `src/thread/` | A thread on screen as its own entity, `ThreadView` (`view.rs`: its `ActiveThread`, composer, events; the app is reached through a weak handle and told which thread with a `ThreadRef`), on `gpui::list` (`mod.rs`, `list.rs`), its rows (`message.rs`, `work.rs`, `queued.rs`), what hangs around it (`extras.rs`, `processes.rs`, `changed_files.rs`, `asks.rs`), the composer (`composer.rs`, `slash.rs`, `attachments.rs`, `drafts.rs`), sending (`send.rs`), opening threads (`open.rs`, the app's), the right dock's thread panel (`panel.rs`) |
| `src/settings/` | The gear (`mod.rs`: Agents, Presets, Appearance tabs; `agent_form.rs`, `presets.rs`, `providers.rs`, `appearance.rs`) and the new project's page (`new_project.rs`) |
| `src/statusbar/` | The bar under everything: `mod.rs`, the agent's item (`agent.rs`), the context and speed panels (`context.rs`, `speed.rs`) |
| `src/inserts/` | **The insert registry** (`mod.rs`: `InsertKind`, `KINDS`, `words()`; `subjects.rs` the `/` list) and one folder per kind: `terminal`, `tasks`, `tools`, `browser` — each `mod.rs` + `state.rs` (+ `view.rs`) |
| `src/explorer/` | **The Explorer**: a project as an IDE shows it — its tree in the left dock (`tree.rs`, `parts.rs`) with its context menus (`menus.rs`), documents in the center with tabs, the project's settings among them — each kind an `Item` as in Zed (`item.rs`; a file `file.rs`; the tabs' keys `doc.rs`, `tabs.rs`, `view.rs`), new/rename/delete (`ops.rs`), ⌘P (`file_finder.rs`), ⌘⇧F (`project_search.rs`), the language servers in the editor (`lsp.rs`) and the Problems tab ⌘⇧M (`problems.rs`), what it was left with kept between starts (`persistence.rs`), lines changed since HEAD and a file's diff (`git.rs`) |
| `src/env/` | A project's environment: `Section` (its data, save, revert) and one of two: `SettingsForm` in `FORMS` (fields on the Settings page `page.rs`) or `ExplorerParts` in `PARTS` (parts in the Explorer's tree, opened in the middle), one file per section |
| `src/media/` | What an answer carries besides text and how it shows (`view.rs`, `text.rs`, `playback.rs`, `chip.rs`), fetching, diagrams, LaTeX; attachments sent, the microphone and playback (`audio.rs`, `video.rs`, `voice.rs`) |
| `src/system/` | What Bench asks of the machine: `ssh.rs`, `sshconfig.rs` (the servers of `~/.ssh/config`), `notify.rs` |
| `src/demo/` | A made-up session for screenshots (`BENCH_DEMO=1\|top\|landing\|long\|new\|tasks\|server\|tool\|env\|explorer`, `env` being the blog's Settings, `explorer` the blog in the Explorer with `BENCH_EXPLORER=settings\|part\|lsp\|thread\|problems\|terminal\|diff` for its settings, an MCP server, a language server, the thread beside, its Problems or its terminal sent to the thread, the post's diff; `BENCH_CLARIFY=1` [+`_MULTI=1`] puts the agent's question card over the thread; `BENCH_DEMO_BESIDE=<thread id>` (e.g. `d-tool`) opens that thread beside the open one; `BENCH_DEMO_DROP=chat|between|right` carries a file into the window and drops it there (an attachment in the thread under it); `BENCH_DEMO_QUEUE=1` puts two messages in the queue after the last row (`BENCH_DEMO_SCROLL=end` keeps the thread at its end); the other hooks in `.agents/skills/bench-kit/references/build-verification.md`): `mod.rs` loads it, `journal.rs`, `threads.rs`, `env.rs`, `inserts.rs`, `drop.rs` |
| `agent/src/server.ts`, `agent/src/modules/` | The agent's server: starts, collects the modules (each declares its RPC and REST routes) |
| `agent/src/runtime.ts`, `agent/src/thread.ts` | Pi conversations, the queue, inserts; the journal as the items Bench shows |
| `agent/src/inserts.ts` | The insert entry and document |
| `agent/src/prompt.ts`, `agent/src/extensions/assistant.ts` | The prompt sections and the default extension (tools, sections, hooks) |
| `agent/src/builtin/` | The agent's own tools and stores: `tasks`, `ssh`, `cron`, `subagent`, `memory`, `skills`, `ask`, `projects`, `servers` (the servers' agents), `presets`, `search` (grep, glob, ls) |
| `agent/src/ssh-env.ts`, `confined-env.ts`, `lsp.ts` | A project's machine over SSH as Pi's `ExecutionEnv`; a project kept in its folder; its language servers (diagnostics after every edit) |
| `agent/src/mcp.ts`, `mcp-oauth.ts`, `telegram.ts` | MCP servers (pi-mcp, OAuth), Telegram bots (grammY, a proxy where needed) |

## How the agent uses Pi Durable

Pi is used unmodified, as its authors describe it. Our code stays in Pi's terms:

| Piece | In Pi's terms |
|---|---|
| Thread | a Pi conversation in `~/.bench/pi.sqlite`; every step durable, a turn survives a restart. Bench shows its journal: `thread.open` (items after a head, the queue, the inserts' state) and the `thread.*` events from Pi's commits (`agent/src/thread.ts`) |
| Insert | an entry `bench.insert {id, key}` written where it was put (a write: the model doesn't see it, never queued) + its state in the conversation document `bench.inserts` `{items:{id:{key,state,folded,removed,after?}}}`, changed in commits. `after` moves one whose entry's place is not where it belongs: an insert put while a message waits in the composer goes after that message. RPC `session.insert`, `session.insert.patch` |
| What the agent knows of inserts | the `inserts` prompt section, read with `input.read.snapshot(Inserts, conversationId)`, one line per kind (`prompt.ts` `inserted()`), plus `<block-state>` with each message |
| Prompt | Pi sections (soul, environment, inserts, project, skills, notes), resent only when changed |
| Queue | Pi's inbox: a message sent while a run is live is a follow-up; "send now" aborts it and steers. Pi 1.4 leaves an aborted item in the inbox, so `dropFromInbox` removes it in a commit |
| `/tasks` | tool `tasks` (`builtin/tasks.ts`), data in `bench.sqlite`; one list for all projects, a project is a tag (`slug` of its name), the default filter in its threads |
| `/terminal` | tool `ssh_run` (`builtin/ssh.ts`); without a host it takes the thread's terminal insert |
| `/tools` | no tool of its own: the utility's input and output come in `<block-state>` |
| `/browser` | the Playwright MCP through `@earendil-works/pi-mcp` (`mcp__browser__*`), one tab per thread (state `{tab,url}`). The server browser is its own project, `~/Projects/browser-server-mcp` (github.com/krutovvv/browser-server-mcp) (an MCP server with a live view); Bench is one of its clients (`src/inserts/browser/gateway.rs`) |
| Model | one for every thread of every project, set in the gear: the general agent's `defaultProvider`/`defaultModel`; a project has no model of its own. The fallback (`fallback_providers[0]`) is a pi-ai provider of ours (`fallback.ts`): the main model's error or silence sends the same request to it (Pi retries only with the same model), and `session.info {fallback}` turns the status bar red |
| Where tools work | Pi's `ExecutionEnv`, one per conversation (`runtime.ts` `envFor`): `NodeExecutionEnv` on the agent's machine, `SshExecutionEnv` (`ssh-env.ts`) on a project's machine over SSH, wrapped by `confined-env.ts` for a project kept in its folder. Pi's read/write/edit/bash and our grep/glob/ls work through it unchanged |
| Compaction | Pi's compaction policy; a project's share of the window becomes a per-model override under its own provider (`project_<id>.<provider>`), through a getter Pi reads at every use (`compactionOverrides`) |
| After an edit | a `ToolTask` `afterTool` hook adds the file's diagnostics from the project's language servers (`lsp.ts`) to edit and write results |
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
- **The change loop** (the same for every change, `.agents/skills/bench-kit/SKILL.md`):
  1. **Change and look, on your own copy.** `cargo build`, then
     `.agents/skills/bench-kit/scripts/shot.sh target/debug/bench /tmp/after.png 10 BENCH_DEMO=…`:
     a separate copy of the debug binary on made-up data (no app, no permissions), shot and
     quit. Read the picture; again until it is right. This is the main check of a change.
     Never rebuild the app to see a change.
     (It needs Bench allowed in Privacy & Security → Screen & System Audio Recording; without a
     picture, ask the person to allow it.)
  2. **Check and commit.** Tests (below), one change one commit.
  3. **Build the app** — at once, no need to ask: `scripts/bundle-macos.sh` rebuilds
     `/Applications/Bench.app` in place and quits nothing; the running Bench goes on from its
     old copy. Uncommitted changes stop it.
  4. **Ask the person to restart Bench** ("собрано — перезапусти Bench"): they quit it and open
     it again, and the new build starts. Never quit, restart or open Bench yourself: you run
     inside it, and your own run would be cut off.

  From outside Bench (a terminal, another app), `.agents/skills/bench-kit/scripts/ship.sh` does
  3 and 4 itself: quits Bench the honest way (`osascript … quit`), rebuilds it, opens it. The way
  back from a bad change is an earlier commit, rebuilt — there is no second app and no second
  build: Bench is one app, signed with the Apple Development certificate, so macOS asks for
  each permission once.
- **Never kill Bench or quit it** (`pkill`, deleting its `.app`, an ad-hoc quit; only `ship.sh`
  from outside): it is the window the person works in.
- **Don't push without asking.** The repository is public: no private names, tokens or the
  person's data in commits.
- **Back up before deleting data** (`~/.bench/backups/`).
- **Screenshots only of your own copy** (`.agents/skills/bench-kit/scripts/shot.sh`, with
  `BENCH_DEMO=…` or `BENCH_HOME=<a copy>`), never the person's window.
- **Disk space is tight.** Clean `target/` (`cargo clean --profile dev`) before it fills.
- **Checks.** The main check of anything the person will see is the picture of your own copy
  (step 1 of the loop, `shot.sh` on demo data): a change is not done until you have looked at it.
  Then `cargo build` with zero warnings, `cargo test --workspace`, and
  `cd agent && node --test test/*.test.ts` for agent changes.

## The look, in short

The look is Zed's: flat and precise, with one accent. Never invent a look or a control; assemble
from `bench_ui` (`crates/ui`, shown by the storybook). A size or a gap is a `bench_ui` constant or
`Gap` step, never a `px(…)` written in a screen; a missing piece is added to `crates/ui` first.

- **Blocks.** Every bordered block is rounded (`rounded_md`, theme radius). Every block head is
  `block_head` (32 px). Fields are `FIELD_H` (28 px). Buttons are rounded and consistent.
- **Scrolling.** Only the thread scrolls. There is no scroll inside a scroll, except the
  terminal. Editors are as tall as their text (`editor_height`). Lists are as long as they are.
  Empty states take one line.
- **Messages.** In the thread there is one font size, the messages'. Copy is an icon.
- **One place for each change.** A change to how blocks look goes to all of them at once.
