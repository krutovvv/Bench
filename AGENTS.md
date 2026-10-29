# AGENTS.md: Bench

Read this first. Then `docs/map.md` to find the file for your change — open that file, not its
neighbours. Read the skill `.agents/skills/bench-kit/SKILL.md` before any UI or insert work, and
`.agents/skills/bench-structure/SKILL.md` (in Russian) before adding a feature, a module, a file
or a new kind of thing, and before any refactor: structure fitting where Bench goes first, then a
file tree readable without docs, then Rust and Zed words.

**Specs.** The why of each module is its `SPEC.md`, beside it (the root `SPEC.md` is Bench's goal;
a few `docs/*.md` carry spec frontmatter too), linked by `parent` into one tree. Read a module's
`SPEC.md` before changing it; a change that moves its boundary or a decision updates it in the
same commit, and `node scripts/specs-check.mjs` checks the links.

## What Bench is

Bench is a native desktop **workbench for working with an AI agent**. It is an open,
thread-centric app, written from scratch in Rust and GPUI
(gpui-kit 0.8). It is a client of its own agent in `agent/`, a Node server on **Pi Durable**.

Bench is not a kit for building other apps, and it has nothing to do with Hermes (an earlier
agent, now gone, with nothing of it left on the wire). There are no profiles for the person: there
are projects. A server's agent is always reinstalled to match this Bench (`launch.rs`).

## The concepts (keep them exact)

- **Thread**: a document of **messages** (the person's, the agent's, with its steps) and the
  person's **inserts**. One conversation in Pi. It never switches mode or gets renamed because of
  an insert. Bench shows Pi's journal of it as rows (`crates/thread`), on a list that **nothing
  moves but the person** — a run that ends, a row that grows, an answer that arrives never scroll
  it (`docs/thread-rewrite.md`).
- **Insert**: a small live tool put into a thread with `/`. The words are `/terminal`, `/browser`
  (where the project has the server browser's MCP) and each utility's own (`/base64`, `/json-yaml`…;
  `/tools …` lists them all). Every `/` adds a new insert where the conversation is. An insert:
  - shows something live (a terminal, a utility, a browser tab);
  - tells the agent what it shows with every message (`<block-state>`, kept out of the thread Bench
    shows; the model's history keeps it, as taking it out would rewrite the prompt and its cache);
  - has an agent tool that acts on it;
  - is restored exactly on return.

  Kinds that can be live only once (the utilities) show "click to show it here" on the older
  ones.
- **Card**: a form or an editor that appears in the thread as an agent-message block, under the
  insert that asked for it (a task). A project's parts that are not files (an MCP server, a cron
  job) open as documents in the Explorer instead.
- **Agent**: a process of `agent/` that runs threads, its machine's service (launchd on a Mac,
  systemd on Linux: it runs without Bench, comes back when it dies or the machine restarts, and
  one per machine). This Mac's always; a server's when the person adds it in the gear (Bench
  installs it, here or over SSH, the same way: `src/client/service.rs`). Closing Bench stops no
  agent; Restart in the agent's popover restarts its service. The Mac's agent keeps the
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
  - model: reasoning, approvals, code mode, compaction;
  - Telegram bot;
  - schedule (cron).

  What it *is set to* (name, where it lives, model, Telegram) is its **Settings** (the last row of its Explorer); what it
  *contains* (its folder, soul, prompts, skills, MCP and language servers, schedule) is the
  **Explorer** (`docs/explorer.md`). The agent changes the same files when asked, from any
  thread of the project. Nothing of one project's environment reaches another, Host's
  included: its soul, MCP servers, skills, prompts, memory and settings are its own threads'
  alone (only the model is the gear's, one for all). **Host** is an agent's own environment: this Mac's is always first in the
  sidebar (installing, building, looking after the machine); a server's (e.g. Dev) names that
  server and is followed by the server's own projects. `/terminal` opens on the thread's machine
  (`thread_machine()`).
- **Explorer**: the third way to look at a project, beside threads and Settings — its files and
  the agent's texts as an IDE shows them (a tree in the sidebar, documents in tabs, the thread
  beside). It is the familiar view of a project, built into Bench: no external editor is
  referenced or needed.
  The window is Zed's workspace: a left dock (threads or Explorer), the center, a right dock.
  A thread sits beside the documents in the Code view; two threads side by side (`src/thread/beside.rs`)
  stay in code but are not offered: two runs at once ask more of a local model than most have.
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
| `crates/protocol` | The wire, in one place: every method a type with its answer (`rpc.rs`, `Request`), what the agent says unasked (`event.rs`, `Event`), a thread (`thread.rs`: `ThreadOpen`, `ThreadEvent`); `cargo test -p protocol` writes `agent/src/protocol.gen.ts` |
| `crates/thread` | The thread as rows from Pi's journal; the rules of order are tests |
| `crates/devtools` | The /tools utilities as tested pure functions |
| `src/main.rs`, `src/app.rs` | Start; `BenchApp`: the window's state and its construction. Only these two are loose in `src/`: every other file is in the folder named for where it is in the window or what it does |
| `src/app/` | What belongs to the whole app: `model.rs` (the plain data every area uses: `ProjectKey`, `Location`, `ProjectEntry`, `Target`; what one area keeps lives in its folder), `events.rs` (the agents' `protocol::Event`s applied: to the views showing a thread, the sidebar's list), `keys.rs` (shortcuts), `menu.rs` (the macOS menu: Settings…, New Window), `armed.rs` (what waits for a second press: deletes, closing unsaved), `defaults.rs` (the agent's defaults), `subscriptions.rs` (what `new` listens to, its timers), `backend.rs` (the agent's restart, its log), `repo.rs` (a thread's git repo) |
| `src/app/places.rs`, `src/app/agents.rs` | **Projects and agents**: where each project lives, the sidebar's projects from their sources; the links to this Mac's and servers' agents |
| `src/client/` | Talking to the agent, as Zed's `client`: `rpc/` (`Client`: a `crates/protocol` request answered as a future of its own response type; `Incoming` what the agent says unasked), `rest/` (REST: files, providers; `with_settings` where a call is about agent settings), `connection.rs` (where it runs, the tunnel), `launch.rs` (installing it and bringing it up, the same script here or over SSH), `service.rs` (the agent as its machine's service: launchd, systemd), `probe.rs` (`bench --probe`) |
| `src/workspace/` | The window as Zed's workspace: `dock.rs` the docks as Zed's (`Dock`: a list of `Panel`s, one shown; `left_dock` threads or Explorer, `right_dock` a thread beside the center (the open one beside a document, a second one beside a thread: `src/thread/beside.rs`), `bottom_dock` the project's terminals, with tabs, `src/inserts/terminal/panel.rs`), `mod.rs` the `Center` (thread, document, settings) and `render_panel`; `layout.rs` draws them around the center |
| `src/titlebar/` | The bar over the center: the thread's title, the documents' project, a settings page's title; at its right end only Chat \| Code (`tail.rs`). The thread's project, repo and meters are the line under its composer (`src/thread/composer/foot.rs`); the Code view has its own status bar under the documents (`src/explorer/statusbar.rs`: the caret, the thread beside ⌘J, the terminal ⌃`); the chat has none |
| `src/sidebar/` | The threads panel, as Claude's sidebar: New thread, New project, Search over the list (`search.rs`: ⌘K, a list over the window), projects (`project_row.rs`, `project_list.rs`), threads (`thread_row.rs`), the menu, what is done to an item (`items.rs`: pin, color, rename, ⌫⌫, ↑↓; `marks.rs`); the left dock's foot under any panel: the agent and its popover, the gear (`agent.rs`) |
| `src/thread/` | A thread on screen as its own entity, `ThreadView` (`view.rs`: its `ActiveThread`, composer, events; the app is reached through a weak handle and told which thread with a `ThreadRef`), on `gpui::list` (`mod.rs`, `list.rs`), laid out as a chat — bubbles and a composer pinned under the list — or as a document, chosen in `look.rs`; `rows/` what the list shows (one file per kind of row: `message.rs`, `work.rs`, `queued.rs`, `asks.rs`, `processes.rs`, `changed_files.rs`, `extras.rs`); `composer/` the next message (`input.rs`, `slash.rs`, `attachments.rs`, `drafts.rs`, `send.rs`); its meters (`meters.rs`, `speed.rs`); opening threads (`open.rs`, the app's) and every agent's threads (`store.rs`); the right dock's thread panel (`panel.rs`, `beside.rs`) |
| `src/settings/` | The gear (`mod.rs`: Agents, Presets, Appearance tabs; `agent_form.rs`, `presets.rs`, `providers.rs`, `appearance.rs`) and the new project's page (`new_project.rs`) |
| `src/inserts/` | **The insert registry** (`mod.rs`: `InsertKind` with its `/` word and choices, `KINDS`; `slash.rs` the `/` list; `state.rs` each thread's inserts and each kind's state) and one folder per kind: `terminal`, `tools`, `browser` — each `mod.rs` + `state.rs` (+ `view.rs`) |
| `src/explorer/` | **The Explorer**: a project as an IDE shows it — `tree/` its tree in the left dock (drawn, its parts, its context menus, new/rename/delete), `pane/` documents in the center with tabs as Zed's pane (each kind an `Item`: `item.rs`, a file `file.rs`, the tabs' keys `doc.rs`, kept between starts `persistence.rs`), `search/` ⌘P and ⌘⇧F, `code/` the language servers in the editor, the Problems tab ⌘⇧M, lines changed since HEAD and a file's diff |
| `src/env/` | A project's environment: `Section` (its data, save, revert) and one of two: `SettingsForm` in `FORMS` (fields on the Settings page `page.rs`) or `ExplorerParts` in `PARTS` (parts in the Explorer's tree, opened in the middle), one file per section |
| `src/media/` | What an answer carries besides text and how it shows (`view.rs`, `text.rs`, `playback.rs`, `chip.rs`), fetching, diagrams, LaTeX; attachments sent, the microphone and playback (`audio.rs`, `video.rs`, `voice.rs`) |
| `src/system/` | What Bench asks of the machine: `ssh.rs`, `sshconfig.rs` (the servers of `~/.ssh/config`), `notify.rs` |
| `src/demo/` | A made-up session for screenshots (`BENCH_DEMO=1\|top\|landing\|long\|new\|server\|tool\|env\|explorer`, `env` being the blog's Settings, `explorer` the blog in the Explorer with `BENCH_EXPLORER=settings\|part\|lsp\|thread\|problems\|terminal\|diff` for its settings, an MCP server, a language server, the thread beside, its Problems or its terminal sent to the thread, the post's diff, and `BENCH_DEMO_NEW=1` with `BENCH_EXPLORER=thread` presses that panel's "+" two seconds in (a new thread in the dock, the document kept in the middle); `BENCH_CLARIFY=1` [+`_MULTI=1`] puts the agent's question card over the thread; `BENCH_DEMO_BESIDE=<thread id>` (e.g. `d-tool`) opens that thread beside the open one; `BENCH_DEMO_DROP=chat|between|right` carries a file into the window and drops it there (an attachment in the thread under it); `BENCH_DEMO_JOURNAL=<file>` draws a real thread (`thread.open`'s answer saved from the agent) in the landing's place, `BENCH_DEMO_WORK=<n>[:<i>…]` opens its n-th piece of work with those lines unfolded; `BENCH_DEMO_PROCS=1` unfolds the landing thread's background processes' card with its output; `BENCH_DEMO_QUEUE=1` puts two messages in the queue after the last row and the agent's plan above them (`BENCH_DEMO_SCROLL=end` keeps the thread at its end); `BENCH_DEMO_TOOL=html|preview|long`
  opens the tool insert with the pane that scrolls inside the thread (its own wheel, not the
  thread's), with lines enough to scroll; `long` pastes a page so tall that the card's half-the-
  window cap shows and the pane scrolls what doesn't fit; the other hooks in
  `.agents/skills/bench-kit/references/build-verification.md`): `mod.rs` loads it, `journal.rs`, `threads.rs`, `env.rs`, `inserts.rs`, `drop.rs` |
| `agent/src/server.ts`, `agent/src/modules/` | The agent's server: starts, collects the modules (each declares its RPC and REST routes) |
| `agent/src/runtime/` | `runtime.ts` `Runtime`: the threads' core (make, open, send, stop, fork); its parts, each a file reached as `rt.<part>`: `models.ts` (the model, its fallback `fallback.ts`), `stream.ts` (a run's events → what Bench hears), `journal.ts` (`Journal`: a thread read whole, each commit's news, the search index; the journal as the items Bench shows), `hold.ts`, `queue.ts`, `asks.ts` (questions, approvals: `bench.approvals`), `inserts.ts` (the insert entry and document), `title.ts`, `usage.ts`; `tools-host.ts` the tools' host and where they work (`envFor`); `run.ts` a prompt run to its answer |
| `agent/src/prompt.ts`, `agent/src/extensions/assistant.ts` | The prompt sections and the default extension (tools, sections, hooks) |
| `agent/src/builtin/` | The agent's own tools and stores: `ssh`, `process`, `cron`, `memory`, `skills`, `ask`, `projects`, `servers` (the servers' agents), `presets`, `search` (grep, glob, ls), `navigate` (`lsp`: definitions, references, outline, symbols, hover from the project's language servers), `hashline` (read and edit by line anchors) |
| `agent/src/machine/`, `agent/src/lsp/` | Where tools work: a project's machine over SSH as Pi's `ExecutionEnv` (`machine/ssh.ts`), a project kept in its folder (`machine/confined.ts`); its language servers (`lsp/lsp.ts`, diagnostics after every edit; `lsp/presets.ts`) |
| `agent/src/mcp/` (`mcp.ts`, `oauth.ts`), `agent/src/telegram.ts` | MCP servers (pi-mcp, OAuth), Telegram bots (grammY, a proxy where needed) |

## How the agent uses Pi Durable

Pi is used unmodified, as its authors describe it. Our code stays in Pi's terms:

| Piece | In Pi's terms |
|---|---|
| Thread | a Pi conversation in `~/.bench/pi.sqlite`; every step durable, a turn survives a restart. Bench shows its journal: `thread.open` (items after a head, the queue, the inserts' state) and the `thread.*` events from Pi's commits (`agent/src/runtime/journal.ts`) |
| Insert | an entry `bench.insert {id, key}` written where it was put (a write: the model doesn't see it, never queued) + its state in the conversation document `bench.inserts` `{items:{id:{key,state,folded,removed,after?}}}`, changed in commits. `after` moves one whose entry's place is not where it belongs: an insert put while a message waits in the composer goes after that message. RPC `session.insert`, `session.insert.patch` |
| What the agent knows of inserts | the `inserts` prompt section, read with `input.read.snapshot(Inserts, conversationId)`, one line per kind (`prompt.ts` `inserted()`), plus `<block-state>` with each message |
| Prompt | Pi sections (soul, environment, inserts, project, skills, notes); all but inserts held as first read until the next compaction (`builtin/frozen.ts`): a changed section rewrites the prompt's head and the model server's cache with it |
| Queue | Pi's inbox: a message sent while a run is live is a follow-up; "send now" aborts it and steers. Bench's id for each message is Pi's `requestId`: the row shown on Enter is matched by it, never by its words |
| Plan, questions | the agent's own `todo` and `clarify` calls in the journal, the answer the call's result (`docs/thread-objects.md`) |
| Pause | the conversation document `bench.hold` (`hold.ts`) and a gate in `beforeTool`: a held run stays held across a restart |
| Code mode | Pi's `codemode` extension (`@earendil-works/pi-codemode`, a QuickJS sandbox): one step runs a script that calls the thread's tools and returns only what it prints. A project's choice (`codeMode` in its agent settings, Settings → Model → Code mode); `forAgent` adds it to the conversation's extensions at every turn, so a thread takes a change from its next turn |
| Fork | `conversation.fork(at)`: "Fork from here" on an answer makes a new thread through that entry |
| `/terminal` | tool `ssh_run` (`builtin/ssh.ts`); without a host it takes the thread's terminal insert |
| `/tools` | no tool of its own: the utility's input and output come in `<block-state>` |
| `/browser` | the Playwright MCP through `@earendil-works/pi-mcp` (`mcp__browser__*`), one tab per thread (state `{tab,url}`). The server browser is its own project, `~/Projects/browser-server-mcp` (github.com/krutovvv/browser-server-mcp) (an MCP server with a live view); Bench is one of its clients (`src/inserts/browser/gateway.rs`), watching the gateway the project's `browser` MCP server names as `live`, with the key `~/.bench/ui/browser.json` keeps for it under `keys` |
| Model | one for every thread of every project, set in the gear: the general agent's `defaultProvider`/`defaultModel`; a project has no model of its own. The fallback (`fallback_providers[0]`) is a pi-ai provider of ours (`fallback.ts`): the main model's error or silence sends the same request to it (Pi retries only with the same model), and the thread's `info {fallback}` shows a red chip by its repo |
| Where tools work | Pi's `ExecutionEnv`, one per conversation (`runtime/tools-host.ts` `envFor`): `NodeExecutionEnv` on the agent's machine, `SshExecutionEnv` (`machine/ssh.ts`) on a project's machine over SSH, wrapped by `machine/confined.ts` for a project kept in its folder. Pi's read/write/edit/bash and our grep/glob/ls work through it unchanged |
| Compaction | Pi's compaction policy; a project's share of the window becomes a per-model override under its own provider (`project_<id>.<provider>`), through a getter Pi reads at every use (`compactionOverrides`); 20% of the context stays word for word, and after the summary the evaluator puts the person's words in force, the files in work and exact results (`judge/summary.ts`) |
| After an edit | a `ToolTask` `afterTool` hook adds the file's diagnostics from the project's language servers (`lsp.ts`) to edit and write results |
| Read and edit | Pi's `read` wrapped to show every line as `LINE:HASH|text`, and an `edit` of the same name that changes lines by those anchors (`builtin/hashline.ts`): no old text to reproduce, a stale anchor refused with the lines as they are now, the changed lines returned with new anchors. An edit with `oldText`/`newText` is Pi's own edit |
| Code navigation | tool `lsp` (`builtin/navigate.ts`) asks the same language servers: a place is a file, a line and the word on it, never a column. The servers start at a thread's first tool call (`envFor` → `warmUp`); one still loading the project (rust-analyzer's `experimental/serverStatus`) is waited for 20 s, then said to be loading |
| Evaluator, step limit | not a Pi model: a System One server (Jev's API) our hooks ask (`agent/src/judge/`, its `Point`s in `POINTS`, one request per moment): `beforeRequest` names a turn's skill and MCP servers (their tools offered from its next step) and, when the window would overflow, says what to cut first, `beforeTool` refuses a repeat of a failed or known call and judges a command's risk with approvals on, `afterTool` orders a long search and a JSON list, judges a failure, marks the plan's items (a `bench.plan` entry, a later snapshot of the plan) and checks the course every 10 steps, a long result keeps the parts the request needs (`builtin/window.ts`), `onYield` sends back once an answer whose facts no result supports or an unfinished turn with an `<evaluator>` message the thread does not show; a project uses it only when its Settings turn it on (Model → Evaluator, off by default: on a shared GPU it takes the room other utilities need) and the gear's connection is on; otherwise the classic way. A project's step limit refuses calls in `beforeTool` and tells the model to answer in `beforeRequest` (`builtin/steps.ts`) |
| Schedule | a durable `defineTask` per job that sleeps until its next run (`builtin/cron.ts`) |
| Background processes | `process` (`builtin/process.ts`): a background task of the conversation (`bench.process`, `background`, `abandonOnRestart`) that runs the command in its `ExecutionEnv`, its output's tail in the checkpoint, its exit the outcome; Bench reads the task records from Pi's commits (`thread.process`). A restart of the agent ends it for good ("stopped by restart") |
| MCP | `@earendil-works/pi-mcp`, a project's `mcp.json`; OAuth via `mcp/oauth.ts` (callback on `localhost:47823`) |
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

The look is a Mac app's — Xcode, Finder, Mail, Messages, System Settings — settled as final on
2026-10-28: extend it in the same voice, with Apple's concrete values, never a look of one's own.
Assemble from `bench_ui` (`crates/ui`, shown by the storybook). A size or a gap is a `bench_ui`
constant or `Gap` step, never a `px(…)` written in a screen; a missing piece is added to
`crates/ui` first. The rules: `.agents/skills/bench-kit/references/design-look-rules.md`.

- **Colours.** One look, two halves: `BENCH_LIGHT` and `BENCH_DARK` in
  `crates/ui/src/theme/palettes.rs` plus their syntax JSON (`assets/themes/bench-*.json`, checked
  against the palette by `cargo test -p bench_ui`). Light or dark is the person's Mode, macOS's by
  default; there is no theme or accent to choose (one accent, Bench's blue). A thread's or a
  project's mark is a Finder tag dot (`bench_ui::mark`).
- **Type.** The thread's words 14, its meta 13; sidebar rows 13, group heads 11 semibold in
  their own case; bars 12, the title 13 semibold; controls 13; code Menlo 12 at Zed's leading.
- **Controls.** Mac ones: segmented controls, switches, pull-downs, navigator glyphs, a
  completion list for `/`, the menu bar with every shortcut (`src/app/menu.rs`).
- **The thread.** A classic chat: the person's grey bubbles, no avatars, a centred column,
  the composer pinned as one rounded field (plus, text, mic, Send). The document look stays in
  code (`src/thread/look.rs`).
- **Blocks.** Grouped boxes: the page's ground, a soft hairline, a card's corner
  (`block_frame`); every block head is `block_head`. Fields are `FIELD_H` (28).
- **Scrolling.** Only the thread scrolls, with a Mac's overlay scrollbar. No scroll inside a
  scroll, except the terminal and editors. Lists are as long as they are. Empty states take one
  line.
- **One place for each change.** A change to how blocks look goes to all of them at once.
