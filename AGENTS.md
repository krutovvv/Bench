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
  - model: reasoning, approvals, evaluator, step limit, compaction;
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
- **The gear** (Bench → Settings… in the macOS menu, ⌘,) holds the app's settings, in tabs: Agents (each one's models, voice and evaluator servers;
  add and remove servers' agents), Presets (MCP and language servers a project takes with one
  click, compaction shares, reasoning levels), Appearance.

## The code map

**Where is X: `docs/map.md`**, one table of common changes and the file for each, and the tree of
the source. Why it is built this way: `docs/architecture.md` and each module's `SPEC.md`. In short:

| Where | What |
|---|---|
| `crates/ui` (`bench_ui`), `crates/storybook` | Every element and size, the only place with pixel values; all of them on one page |
| `crates/protocol`, `crates/thread`, `crates/devtools` | The wire, one type per method; the thread as rows from Pi's journal; the /tools utilities |
| `src/main.rs`, `src/app.rs`, `src/app/` | Start; `BenchApp`, one field per area; what belongs to the whole app (`places.rs` answers "where") |
| `src/workspace/`, `src/titlebar/`, `src/sidebar/`, `src/thread/`, `src/settings/` | The window by place, as Zed's workspace |
| `src/inserts/`, `src/env/`, `src/explorer/` | By feature: a registry and one folder or file per kind |
| `src/client/`, `src/media/`, `src/system/`, `src/demo/` | Services that draw nothing; the demo, a fake agent behind `src/client/fake.rs` |
| `agent/src/` | `server.ts` and `modules/`; `runtime/`, `extensions/`, `builtin/`, `turn/`, `judge/`, `machine/`, `mcp/`, `lsp/` |

## How the agent uses Pi Durable

Pi is used unmodified, as its authors describe it. Our code stays in Pi's terms:

| Piece | In Pi's terms |
|---|---|
| Thread | a Pi conversation in `~/.bench/pi.sqlite`; every step durable, a turn survives a restart. Bench shows its journal: `thread.open` (items after a head, the queue, the inserts' state) and the `thread.*` events from Pi's commits (`agent/src/runtime/journal.ts`) |
| Insert | an entry `bench.insert {id, key}` written where it was put (a write: the model doesn't see it, never queued) + its state in the conversation document `bench.inserts` `{items:{id:{key,state,folded,removed,after?}}}`, changed in commits. `after` moves one whose entry's place is not where it belongs: an insert put while a message waits in the composer goes after that message. RPC `thread.insert`, `thread.insert.patch`, told as `ThreadEvent::Inserts` (`src/inserts/SPEC.md`) |
| What the agent knows of inserts | the `inserts` prompt section, read with `input.read.snapshot(Inserts, conversationId)`, one line per kind (`prompt.ts` `inserted()`), plus `<block-state>` with each message |
| Prompt | Pi sections (soul, environment, inserts, project, skills, notes); all but inserts held as first read until the next compaction (`turn/frozen.ts`): a changed section rewrites the prompt's head and the model server's cache with it |
| Queue | Pi's inbox: a message sent while a run is live is a follow-up; "send now" aborts it and steers. Bench's id for each message is Pi's `requestId`: the row shown on Enter is matched by it, never by its words |
| Plan, questions | the agent's own `todo` and `clarify` calls in the journal, the answer the call's result (`docs/thread-objects.md`) |
| Streams of the model's server | no such limit between conversations in Pi, so ours: the main provider's `parallel` (the gear's "Streams"), a gate in `beforeRequest` (`runtime/streams.ts`). Whatever asks the model takes one: a conversation at work keeps its stream to the end of its run and gives it up while it waits for something else; a title, a review, a compaction outside a run takes one for its request; the others wait in line, in order |
| Pause | the conversation document `bench.hold` (`hold.ts`) and a gate in `beforeTool`: a held run stays held across a restart |
| Code mode | Pi's `codemode` extension (`@earendil-works/pi-codemode`, a QuickJS sandbox): one step runs a script that calls the thread's tools and returns only what it prints. Every thread has it beside the tools it calls directly (`forAgent` adds it to the conversation's extensions at every turn), with no setting: not a mode for code, several steps in one. Scripts reach MCP servers (each a namespace) and `models.*`: `models.classify()` on `bench/evaluator` where the project uses the evaluator |
| Fork | `conversation.fork(at)`: "Fork from here" on an answer makes a new thread through that entry |
| `/terminal` | tool `ssh_run` (`builtin/ssh.ts`); without a host it takes the thread's terminal insert |
| `/tools` | no tool of its own: the utility's input and output come in `<block-state>` |
| `/browser` | the Playwright MCP through `@earendil-works/pi-mcp` (`mcp__browser__*`), one tab per thread (state `{tab,url}`). The server browser is its own project, `~/Projects/browser-server-mcp` (github.com/krutovvv/browser-server-mcp) (an MCP server with a live view); Bench is one of its clients (`src/inserts/browser/gateway.rs`), watching the gateway the project's `browser` MCP server names as `live`, with the key `~/.bench/ui/browser.json` keeps for it under `keys` |
| Model | one for every thread of every project, set in the gear: the general agent's `defaultProvider`/`defaultModel`; a project has no model of its own. The fallback (`fallback_providers[0]`) is a pi-ai provider of ours (`fallback.ts`): the main model's error or silence sends the same request to it (Pi retries only with the same model), and the thread's `info {fallback}` shows a red chip by its repo |
| Where tools work | Pi's `ExecutionEnv`, one per conversation (`runtime/tools-host.ts` `envFor`): `NodeExecutionEnv` on the agent's machine, `SshExecutionEnv` (`machine/ssh.ts`) on a project's machine over SSH, wrapped by `machine/confined.ts` for a project kept in its folder. Pi's read/write/edit/bash and our grep/glob/ls work through it unchanged |
| Compaction | Pi's compaction policy; a project's share of the window becomes a per-model override under its own provider (`project_<id>.<provider>`), through a getter Pi reads at every use (`compactionOverrides`); 20% of the context stays word for word, and after the summary the evaluator puts the person's words in force, the files in work and exact results (`judge/summary.ts`) |
| After an edit | a `ToolTask` `afterTool` hook (`builtin/diagnostics.ts`) adds the file's diagnostics from the project's language servers (`lsp/lsp.ts`) to edit and write results |
| Read and edit | Pi's `read` wrapped to show every line as `LINE:HASH|text`, and an `edit` of the same name that changes lines by those anchors (`builtin/hashline.ts`): no old text to reproduce, a stale anchor refused with the lines as they are now, the changed lines returned with new anchors. An edit with `oldText`/`newText` is Pi's own edit |
| Code navigation | tool `lsp` (`builtin/navigate.ts`) asks the same language servers: a place is a file, a line and the word on it, never a column. The servers start at a thread's first tool call (`envFor` → `warmUp`); one still loading the project (rust-analyzer's `experimental/serverStatus`) is waited for 20 s, then said to be loading |
| Evaluator | Pi's classifier model `bench/evaluator` (`judge/models.ts`): the gear's System One servers, a main one and its fallback, which our hooks ask at fixed points of a turn and code mode scripts too. A project uses it when its Settings say so (on in a new project). What it does at each point, and the classic way without it: `agent/src/judge/SPEC.md` |
| Step limit | a project's most steps in one turn: refuses calls in `beforeTool` and tells the model to answer in `beforeRequest` (`turn/steps.ts`) |
| Schedule | a durable `defineTask` per job that sleeps until its next run (`builtin/cron.ts`) |
| Background processes | `process` (`builtin/process.ts`): a background task of the conversation (`bench.process`, `background`, `abandonOnRestart`) that runs the command in its `ExecutionEnv`, its output's tail in the checkpoint, its exit the outcome; Bench reads the task records from Pi's commits (`thread.process`). A restart of the agent ends it for good ("stopped by restart") |
| MCP | `@earendil-works/pi-mcp`, a project's `mcp.json`, with Pi's `exposure`: `codemode` by default (`agent/src/mcp/SPEC.md`). OAuth via `mcp/oauth.ts` (callback on `localhost:47823`) |
| Telegram | grammY, one bot per project, the same Pi conversations; `TELEGRAM_PROXY` or `HTTPS_PROXY` in the agent's env |

Extend with Pi's packages and popular ones before writing your own. When something in Pi is
missing or buggy, work around it in our code and say so in a comment. Never patch Pi.

## How the owner wants Bench built

These hold over any habit of your own.

- **The base frameworks first.** The agent is built on Pi Durable as its authors intend: before
  writing a mechanism, check whether Pi Durable, pi-ai, pi-mcp or code mode already has it (docs
  vendored in `.agents/skills/pi-durable-dev-rules/references/`; sources in
  `agent/node_modules/@earendil-works/`; live docs at pi.dev/docs/latest). Use theirs. The same for
  the UI: GPUI, gpui-kit and Zed's patterns before anything custom (Zed is the reference for
  organisation and names: workspace, docks, panels, items, `FakeFs`).
- **Hooks are Pi's sanctioned extension point.** Our evaluator points in the agent loop
  (`agent/src/judge/`) stay: they are real gains. Pi's own guide shows `onYield` sending the agent
  back, exactly our use.
- **Modules are self-contained**, in folders and files whose names tell a reader, a person or an
  agent, where the logic is from the file tree alone.
- **See the patterns and share them.** One thing written twice becomes one implementation. A family
  of similar features gets one scaffold: a widget has a frame, a head and a body, so there is one
  container and each widget brings its content; changing the container changes them all (an
  answer's code block and the code inside an action card are one widget, `bench_ui::code_frame`).
- **Anticipate where the app grows.** When adding a feature, ask what else of its kind will come and
  lay the common ground (a registry, a container, a trait) from the first lines.
- **Symmetric, logical code.** Siblings are built alike (inserts, env sections, gear tabs).
- **Causes, not symptoms.** No flags threaded through layers to undo an effect, no special cases for
  one observed bug, no negative margins to cancel a padding, no `if self.demo` in product code (the
  demo is a fake agent, as Zed's `FakeFs`), no app-wide "click claimed" flag where GPUI has
  `stop_propagation`, no pixel values or bare spacing outside `crates/ui`, no side effects in
  `Render`, no polling where something can say it changed.
- **The look is final** (below). Refactors keep pixels identical; do not redesign. If a mechanical
  cleanup shifts something slightly, the owner accepts fixing it later, but check with pictures.
- **Documents match the code.** Specs, docs and skills: each fact lives in one place, others link
  to it. A change that moves a decision updates its spec in the same commit.

## How to work here

- **The person writes in Russian; answer in Russian.**
- **Bench develops itself, in one app.** The person works in `/Applications/Bench.app` and talks
  to you through it. It is only a client: threads, the queue and the agent live in the agent
  process and in `~/.bench`, so rebuilding and reopening it costs a few seconds and loses
  nothing — an unfinished run goes on in the agent and the window comes back to it.
- **The change loop**, the same for every change: change and look at your own copy
  (`.agents/skills/bench-kit/scripts/shot.sh`, on the demo's made-up data), checks and one commit,
  `scripts/bundle-macos.sh` at once, ask the person to restart ("собрано — перезапусти Bench"). Never
  rebuild the app to see a change. The steps are `.agents/skills/bench-kit/SKILL.md`'s.
- **Never kill Bench or quit it** (`pkill`, deleting its `.app`, an ad-hoc quit; only `ship.sh`
  from outside): it is the window the person works in.
- **Don't push without asking.** The repository is public: no private names, tokens or the
  person's data in commits.
- **Back up before deleting data** (`~/.bench/backups/`).
- **Screenshots only of your own copy** (`.agents/skills/bench-kit/scripts/shot.sh`, with
  `BENCH_DEMO=…` or `BENCH_HOME=<a copy>`), never the person's window.
- **Disk space is tight.** Clean `target/` (`cargo clean --profile dev`) before it fills. No second
  target dir, so no Rust work in a separate worktree.
- **Checks.** The main check of anything the person will see is the picture of your own copy: a
  change is not done until you have looked at it. Many screens touched: every demo state against a
  reference (`.agents/skills/bench-kit/scripts/pixels/`).
  Then `cargo build` with zero warnings, `cargo test --workspace`, and
  `cd agent && node --test test/*.test.ts` for agent changes.

## The look, in short

The look is a Mac app's — Xcode, Finder, Mail, Messages, System Settings — settled as final on
2026-10-28: extend it in the same voice, with Apple's concrete values, never a look of one's own.
Assemble from `bench_ui` (`crates/ui`, shown by the storybook). A size or a gap is a `bench_ui`
constant or `Gap` step (`.gap(Gap::M)`, `.px(Gap::L)`), never a `px(…)` or a bare Tailwind spacing
(`gap_2`, `px_3`) written in a screen; a missing piece is added to `crates/ui` first. The rules: `.agents/skills/bench-kit/references/design-look-rules.md`.

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
