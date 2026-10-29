# Where is what

One page to open before reading code. Find the row, open that file, not its neighbours.

## The common changes

| You are asked to… | Open |
|---|---|
| change a size, a gap, a font, a color, a button's look | `crates/ui/src/` (`metrics.rs`, `text.rs`, `button.rs`, `theme/`) — see it in `cargo run -p storybook` |
| change Bench's colours (its light and dark half) | `crates/ui/src/theme/palettes.rs` (`BENCH_LIGHT`, `BENCH_DARK`), the syntax JSON of each half in `assets/themes/bench-*.json` (written by `scripts/gen_themes.py`); a mark's colours: `crates/ui/src/mark.rs` |
| add an icon | `assets/icons/<name>.svg`, a line in `crates/ui/src/icons.rs` |
| change how a message, an answer or "N actions" looks | `src/thread/rows/message.rs`, `src/thread/rows/work.rs` (a call's line: `rows/tool_labels.rs`) |
| change what the thread shows or in what order | `crates/thread/src/rows.rs` (the rules are tests in `tests.rs`) |
| change a queued message's row or its buttons | `src/thread/rows/queued.rs` (the row and its actions: `queued`, `queue_edit_*`) |
| switch the thread's look (the chat: bubbles, a pinned composer; the document: avatars, the composer as the last row) | `src/thread/look.rs` (`pub type Current`) |
| a review comment on code (⌘⇧L) | `src/explorer/pane/comment.rs` (the band), `Reference::Comment` in `src/thread/composer/attachments.rs`, `resolve_comment` in `agent/src/builtin/review.ts` |
| change the composer | `src/thread/composer/` (`input.rs` the field, its pop-up `slash.rs`, prompts `prompts.rs`, files and references (code, review comments, problems, a terminal) `attachments.rs`, drafts `drafts.rs`, sending `send.rs`) |
| the specs: which spec a file is under (the agent), the Explorer's "Specs" and a spec's tab | `agent/src/specs.ts` (+ the hook in `agent/src/extensions/assistant.ts`, `GET /api/specs`), `src/explorer/specs/` (`tree.rs`, `doc.rs`) |
| a turn's review (findings on its changes, sent as comments) | `agent/src/runtime/review.ts` (`thread.review`), drawn in `src/thread/rows/changed_files.rs` |
| what a turn changed (git trees at a run's start and end) | `agent/src/runtime/turns.ts`, drawn by `src/thread/rows/changed_files.rs`, a file's turn diff as a tab `src/explorer/code/turn_diff.rs` |
| change a kind of row in the thread (a question, the plan, processes, changed files, pages read) | `src/thread/rows/` (`asks.rs`, `answer.rs`, `processes.rs`, `changed_files.rs`, `extras.rs`) |
| change the bottom dock's terminals (⌃`, ⌘T, the tabs) | `src/inserts/terminal/panel.rs` (the shell itself: `session.rs`, `view.rs`) |
| change an insert (`/terminal`, `/tools`, `/browser`) | `src/inserts/<kind>/` — `mod.rs` the block, `state.rs` its data and actions, `view.rs` a long body |
| add an insert | `.agents/skills/bench-kit/references/build-insert.md` |
| change a project's Settings (name, where, model, Telegram) or its parts (MCP, language servers, schedule) | `src/env/<section>.rs` (`mcp.rs`, `schedule.rs`, `telegram.rs`…); the page `src/env/page.rs`; a new section: `src/env/mod.rs` header |
| export a project's environment to a file or import one (backups, a move to another project) | `src/env/backup.rs` (the Settings rows, the save and open dialogs, the alert); the agent packs and unpacks it in `agent/src/modules/backup.ts` (test `agent/test/backup.test.ts`) |
| change the Explorer (the tree and its menus, documents, tabs, ⌘P, ⌘⇧F) | `src/explorer/`: `mod.rs` its state; `tree/` the tree (`view.rs` drawn, `parts.rs` the project's parts, `menus.rs` its context menus, `ops.rs` listing, new/rename/delete); `pane/` the middle as Zed's pane (`doc.rs` the tabs' keys and open/close/save, `item.rs` a document as Zed's `Item` — a new kind of document is a `DocKey` variant and its `Item` —, `file.rs` a file in an editor, `tabs.rs`, `view.rs`, `persistence.rs` kept between starts); `search/` (`file_finder.rs` ⌘P, `project_search.rs` ⌘⇧F); `code/` (`lsp.rs` problems, hover and definition, `problems.rs` the Problems tab ⌘⇧M, `git.rs` changed lines and a file's diff, `lang.rs` a file's language); its pieces `crates/ui/src/ide.rs`; the agent's `/api/fs/*` in `agent/src/modules/files.ts`; why: `docs/explorer.md` |
| change what the window's docks or center show, add a panel to a dock | `src/workspace/dock.rs` (`Dock`, `Panel`), `src/workspace/mod.rs` (`Center`, `render_panel`), the layout in `src/workspace/layout.rs` |
| change the bar over the center (a thread's title, the documents' project, a settings page's title) | `src/titlebar/mod.rs`; the documents' tab bar `src/explorer/pane/view.rs` (`render_doc`), the tabs themselves `src/explorer/pane/tabs.rs` |
| change what happens on an agent event | `src/app/events.rs` |
| open, delete or search threads | `src/thread/open.rs` |
| make anything wait for a second press (delete, remove, close unsaved) | `src/app/armed.rs` (`self.armed.confirm(key)`), the control `bench_ui::confirm_buttons` |
| change what a thread on screen keeps (its composer, its run, its events) | `src/thread/view.rs` (`ThreadView`), its state `src/thread/active.rs` (`ActiveThread`, `ThreadRef`) |
| change the second thread beside the open one ("Open beside") or the right dock's thread head | `src/thread/beside.rs`, `src/thread/panel.rs`; the menu item in `src/sidebar/menu.rs` |
| change an answer's media (images, video, audio, diagrams, formulas, code blocks) | `src/media/` (`view.rs`, `text.rs`, `playback.rs`, `chip.rs`) |
| change dictation (the mic, text while speaking: pieces sent at pauses) | `src/media/voice.rs`, the pauses `src/media/pauses.rs` (tests), the recorder `src/media/audio.rs`; its switch in the gear's Voice (`src/settings/mod.rs`), kept in `dictation.json` |
| change the sidebar | `src/sidebar/` (`mod.rs` the panel, `project_row.rs` a project, `thread_row.rs` a thread, `menu.rs` the ⋯ menu, `top.rs` the strip over it; loading projects: `project_list.rs`); what is done to a thread or project there — pin, color, rename, delete, ↑ ↓ ⌫ — `src/sidebar/items.rs`, marks in `src/sidebar/marks.rs` |
| the agent's item, the gear (the left dock's foot) | `src/sidebar/agent.rs` |
| the Code view's status bar (caret, thread beside, terminal) | `src/explorer/statusbar.rs` |
| the line under the composer (project, repo, meters) | `src/thread/composer/foot.rs` |
| ⌘K, the search over threads and projects | `src/sidebar/search.rs` |
| change a thread's meters (context, speed, fallback, by its repo) | `src/thread/meters.rs`, the speed meter `src/thread/speed.rs` |
| change the gear (tabs Agents, Presets, Appearance) | `src/settings/mod.rs` (the tabs, built from `bench_ui::settings_*`), `src/settings/providers.rs`, `src/settings/agent_form.rs` (add/remove a server's agent), `src/settings/presets.rs`, `src/settings/appearance.rs` (Appearance) |
| change a keyboard shortcut | `src/app/keys.rs` (the built-in inserts' shortcuts on or off: `src/app/shortcuts.rs`); the macOS menu (Settings… ⌘,, New Window ⌘⇧N) `src/app/menu.rs` |
| change a /tools utility | `crates/devtools/src/` (pure functions with tests) |
| change what Bench sends or receives | a method or an event: `crates/protocol` (`rpc.rs` one type per method with its answer, `event.rs`, `thread.rs`), its handler in `agent/src/modules/`, Bench's side `app.request`/`send`/`send_at` (`src/app/agents.rs`), events in `src/app/events.rs`; the socket `src/client/rpc/`; REST `src/client/rest/` |
| change the agent's answer to an RPC or a REST route | `agent/src/modules/<area>.ts` |
| change how the agent runs a thread | `agent/src/runtime/runtime.ts` (the core) and its part for the job (`stream.ts`, `queue.ts`, `hold.ts`, `asks.ts`, `inserts.ts`, `title.ts`, `models.ts`); what Bench reads of it: `agent/src/runtime/journal.ts` |
| change how work is done on a server (a project's machine over SSH) | `agent/src/machine/ssh.ts` (Pi's `ExecutionEnv`; its tests are Pi's conformance suite, `test/ssh-env.test.ts`) |
| change the agents Bench talks to (this Mac's, servers') | `src/app/agents.rs` (links), the list of servers' agents in `agent/src/builtin/servers.ts`; adding/removing in `src/settings/agent_form.rs` |
| change where a project lives (agent, machine, folder) or how the sidebar's projects are put together | `src/app/places.rs` (`ProjectKey`, `Location` in `src/app/model.rs`); its rows in `src/env/place.rs` |
| keep a project's agent in its folder | `agent/src/machine/confined.ts` |
| change the presets (MCP, LSP, compaction, reasoning levels) | defaults in `agent/src/builtin/presets.ts`; Bench's side `src/settings/presets.rs`, the gear's tab in `src/settings/mod.rs` |
| the evaluator (a System One model asked at searches, on the way, at a turn's end) or a project's step limit | `agent/src/judge/` (a new point: a `Point` file and a line in `POINTS`, `index.ts`; `client.ts`, `point.ts`, `search.ts`, `plan.ts`, `drift.ts`, `done.ts`, `SPEC.md`), the turn they read `agent/src/builtin/turn.ts`, the limit `agent/src/builtin/steps.ts`; hooked in `agent/src/extensions/assistant.ts`; the gear's section `src/settings/providers.rs` (`Judge`), the limit's row `src/env/run.rs`, the counts in `src/thread/meters.rs` |
| what reaches the model fits its window (a tool's result cut, a request cut to fit) | `agent/src/builtin/window.ts`, hooked in `agent/src/extensions/assistant.ts` (`afterTool`, `beforeRequest`, `afterResponse`) |
| change an agent tool | `agent/src/builtin/<tool>.ts`, its prompt line in `agent/src/prompt.ts`; grep, glob, ls: `agent/src/builtin/search.ts` |
| change a project's language servers (diagnostics after every edit; the editor's problems, hover, definition via `agent/src/modules/lsp.ts`) | `agent/src/lsp/lsp.ts`, the presets in `agent/src/lsp/presets.ts`; the section `src/env/lsp.rs` |
| change when a project's conversations are compacted | `compactionPolicy` in `agent/src/config.ts`; the row in `src/env/run.rs` |
| change the demo (screenshots) | `src/demo/` (`mod.rs` what each `BENCH_DEMO` opens, `BENCH_DEMO_PROJECT=1|env` the new project's page (with an environment chosen), `threads.rs`, `env.rs` the blog project, `inserts.rs` the made-up servers, `journal.rs`) |

## The tree

```
crates/
  ui/            every element and size; nothing else has px values       (bench_ui)
  storybook/     all elements on one page; reference/ has the pictures
  protocol/      thread.open and thread.* types; writes agent/src/protocol.gen.ts
  thread/        the thread as rows from Pi's journal; rules of order as tests
  devtools/      the /tools utilities, pure functions
src/
  main.rs        start, the window
  app.rs         BenchApp: the window's state and its construction
  app/           the whole app: model (plain data), events, keys, connect, subscriptions, places, agents, projects
  client/        talking to the agent: rpc/ (WebSocket), rest/ (REST), connection, launch (here or over SSH)
  workspace/     the window: docks and center (mod.rs), their layout (layout.rs)
  titlebar/      the bar over the center
  sidebar/       the threads panel: projects, threads, their menu and marks
  thread/        a thread on screen, its own entity (ThreadView) on gpui::list: rows, what hangs around them, the composer, sending
  settings/      the gear's tabs, the new project's page
  inserts/       one folder per insert kind, the registry in mod.rs
  env/           a project's environment: one file per section, the registry in mod.rs, the Settings page page.rs
  explorer/      the Explorer: tree, documents, tabs, ⌘P, ⌘⇧F (docs/explorer.md)
  media/         an answer's media, attachments, audio and video
  system/        ssh, ~/.ssh/config, notifications
  demo/          the made-up session for screenshots and the web demo
agent/src/
  server.ts      starts the agent, collects modules/
  modules/       threads, commands, projects (environment), agents, presets, cron, files, audio, settings
  runtime.ts     the threads' core; its parts: models, stream, thread (Journal), hold, queue, asks, inserts, title
  thread.ts      Pi's journal → the items Bench shows
  builtin/       the agent's tools
docs/            architecture.md (why), map.md (where), thread-rewrite.md (the thread's design),
                 remote-projects.md (agents, machines, projects on servers)
```

## Names worth knowing

- **Row** (`crates/thread`): one line of the thread — User, Work, Answer, Insert, Fail, Writing, Queued.
- **Work**: the agent's steps between two messages; counted per message, never per run.
- **Insert** / **InsertKind** (`src/inserts/mod.rs`): a live tool in the thread and its kind; each method is told its thread (`ThreadRef`).
- **ThreadView** (`src/thread/view.rs`): a thread on screen, an entity; **ThreadRef**: which thread, as the app is told it.
- **Section** (`src/env/section.rs`): one part of a project's environment; a `SettingsForm` (fields on its Settings page) or `ExplorerParts` (parts in the Explorer's tree).
- **Journal**: Pi's entries of a conversation; Bench's thread is read from it, nothing else.
