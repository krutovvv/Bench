---
name: bench-map
description: "Use first for any change in Bench: which file to open for it, and the whole source tree."
---

# Where is what

Find the row for your change, open that file, and look at its folder too.

## The common changes

| You are asked to… | Open |
|---|---|
| change a size, a gap, a font, a color, a button's look | `crates/ui/src/` (`metrics.rs`, `text.rs`, `button.rs`, `theme/`) — see it in `cargo run -p storybook` |
| change Bench's colours (its light and dark half) | `crates/ui/src/theme/palettes.rs` (`BENCH_LIGHT`, `BENCH_DARK`), the syntax JSON of each half in `assets/themes/bench-*.json` (written by `scripts/gen_themes.py`); a mark's colours: `crates/ui/src/mark.rs` |
| add an icon | `assets/icons/<name>.svg`, a line in `crates/ui/src/icons.rs` |
| change how a message, an answer or "N actions" looks | `src/thread/rows/message.rs`, `src/thread/rows/work.rs` (a call's line: `crates/thread/src/tool_labels.rs`) |
| change the opened work (its milestones, a step's card) | `src/thread/rows/steps.rs`; how a call is said `crates/thread/src/steps.rs` |
| change what the thread shows or in what order | `crates/thread/src/rows.rs` (the rules are tests in `tests.rs`) |
| change a queued message's row or its buttons | `src/thread/rows/queued.rs` (the row and its actions: `queued`, `queue_edit_*`) |
| switch the thread's look (the chat: bubbles, a pinned composer; the document: avatars, the composer as the last row) | `src/thread/look.rs` (`pub type Current`) |
| a review comment on code (⌘⇧L) | `src/explorer/pane/comment.rs` (the band), `Reference::Comment` in `src/thread/composer/attachments.rs`, `resolve_comment` in `agent/src/builtin/review.ts` |
| change the composer (Enter sends, it grows to `COMPOSER_MAX_LINES` and scrolls inside) | `src/thread/view.rs` (the field's state and Enter), `src/thread/composer/` (`input.rs` the field, its pop-up `slash.rs`, prompts `prompts.rs`, files and references (code, review comments, problems, a terminal) `attachments.rs`, drafts `drafts.rs`, sending `send.rs`) |
| the specs: which spec a file is under (the agent), the Explorer's "Specs" and a spec's tab | `agent/src/specs.ts` (+ the hook in `agent/src/extensions/assistant.ts`, `GET /api/specs`), `src/explorer/specs/` (`tree.rs`, `doc.rs`) |
| a turn's review (findings on its changes, sent as comments) | `agent/src/runtime/review.ts` (`thread.review`), drawn in `src/thread/rows/changed_files.rs` |
| what a turn changed (git trees at a run's start and end) | `agent/src/runtime/changes.ts`, drawn by `src/thread/rows/changed_files.rs`, a file's turn diff as a tab `src/explorer/code/turn_diff.rs` (one diff pane: `diff_pane`, `src/explorer/code/git.rs`) |
| change a kind of row in the thread (a question, the plan, processes, changed files, pages read) | `src/thread/rows/` (`asks.rs`, `clarify.rs`, `processes.rs`, `changed_files.rs`, `pages.rs`) |
| change the bottom dock's terminals (⌃`, ⌘T, the tabs) | `src/inserts/terminal/panel.rs` (the shell itself: `session.rs`, `view.rs`) |
| change an insert (`/terminal`, `/tools`, `/browser`) | `src/inserts/<kind>/` — `mod.rs` the block, `state.rs` its data and actions, `view.rs` a long body |
| add an insert | `.agents/skills/bench-kit/references/build-insert.md` |
| change a project's Settings (name, where, model, Telegram) or its parts (MCP, language servers, schedule) | `src/env/<section>.rs` (`mcp/`, `schedule.rs`, `telegram.rs`…); the page `src/env/page.rs`; a new section: `src/env/mod.rs` header |
| export a project's environment to a file or import one (backups, a move to another project) | `src/env/backup.rs` (the Settings rows, the save and open dialogs, the alert); the agent packs and unpacks it in `agent/src/modules/backup.ts` (test `agent/test/backup.test.ts`) |
| change the Explorer (the tree and its menus, documents, tabs, ⌘P, ⌘⇧F) | `src/explorer/`: `mod.rs` its state; `tree/` the tree (`view.rs` drawn, `parts.rs` the project's parts, `menus.rs` its context menus, `ops.rs` listing, new/rename/delete); `pane/` the middle as Zed's pane (`doc.rs` the tabs' keys and open/close/save, `item.rs` a document as Zed's `Item` — a new kind of document is a `DocKey` variant and its `Item` —, `file.rs` a file in an editor, `tabs.rs`, `view.rs`, `persistence.rs` kept between starts); `search/` (`file_finder.rs` ⌘P, `project_search.rs` ⌘⇧F); `code/` (`lsp.rs` problems, hover and definition, `problems.rs` the Problems tab ⌘⇧M, `git.rs` changed lines and a file's diff, `lang.rs` a file's language); its pieces `crates/ui/src/ide.rs`; the agent's `/api/fs/*` in `agent/src/modules/files.ts`; why: `src/explorer/SPEC.md` |
| change what the window's docks or center show, add a panel to a dock | `src/workspace/dock.rs` (`Dock`, `Panel`), `src/workspace/mod.rs` (`Center`, `render_panel`), the layout in `src/workspace/layout.rs` |
| change the bar over the center (a thread's title, the documents' project, a settings page's title) | `src/titlebar/mod.rs`; the documents' tab bar `src/explorer/pane/view.rs` (`render_doc`), the tabs themselves `src/explorer/pane/tabs.rs` |
| back and forward over the threads and documents shown (‹ › in the title bar, ⌘[ ⌘]) | `src/workspace/history.rs` (`NavHistory`, noted by `BenchApp::follow_shown`, `src/app/subscriptions.rs`); the buttons `src/titlebar/mod.rs` |
| a new thread's welcome (Bench's mark, the first steps, "no model yet") | `src/thread/rows/welcome.rs`; its pieces `crates/ui/src/welcome.rs`; `BENCH_DEMO=new` [`BENCH_DEMO_NOMODEL=1`] |
| a fork shown under the thread it came from (the sidebar's └) | the order `nest_forks` in `src/sidebar/items.rs`, the corner `bench_ui::nest_corner`; the parent is kept by the agent (`parent` in `agent/src/store.ts`, set by `Runtime.fork`) |
| a project head's marks (its Telegram bot, a schedule) | `src/sidebar/project_row.rs` (`telegram_mark`, `cron_mark`); what the agent says of them `GET /api/projects` in `agent/src/modules/projects.ts`, a schedule's change → `env_changed` (`onJobsChanged`, `agent/src/builtin/cron.ts`) |
| change what happens on an agent event | `src/app/events.rs` |
| what follows a change in the window (the prompts `@` offers, the browser's stream, the place shown): `Render` has no side effects | `BenchApp::observe_changes`, `follow_shown` in `src/app/subscriptions.rs` |
| a file carried in from the machine (one layer over the window) | `file_layer` in `src/workspace/layout.rs`, `BenchApp::drop_paths` in `src/app.rs`, `src/thread/composer/attachments.rs` |
| how a click inside the thread gives the composer the focus | `ThreadView::press` in `src/thread/mod.rs`; a clickable element stops its own press: `bench_ui::own_press` |
| open, delete or search threads | `src/thread/open.rs` |
| make anything wait for a second press (delete, remove, close unsaved) | `src/app/armed.rs` (`self.armed.confirm(key)`), the control `bench_ui::confirm_buttons` |
| change what a thread on screen keeps (its composer, its run, its events) | `src/thread/view.rs` (`ThreadView`), its state `src/thread/active.rs` (`ActiveThread`, `ThreadRef`) |
| change the second thread beside the open one (kept in code, not offered in the menu) or the right dock's thread head | `src/thread/beside.rs`, `src/thread/panel.rs` |
| change an answer's media (images, video, audio, diagrams, formulas, code blocks) | `src/media/` (`view.rs`, `text.rs`, `playback.rs`, `chip.rs`) |
| change Bench's sounds (the mic, a run's end, a question) | `src/media/sound.rs` (what plays, its switch `sounds.json`), the files `assets/sounds/` written by `scripts/make-sounds.py`; when they play: `src/media/voice.rs`, `src/app/events.rs`; the switch in the gear's Appearance `src/settings/appearance.rs` |
| change dictation (the mic, text while speaking: pieces sent at pauses) | `src/media/voice.rs`, the pauses `src/media/pauses.rs` (tests), the recorder `src/media/audio.rs`; its switch in the gear's Voice (`src/settings/voice.rs`), kept in `dictation.json` |
| change the sidebar | `src/sidebar/` (`mod.rs` the panel, `project_row.rs` a project, `thread_row.rs` a thread, `menu.rs` the ⋯ menu; loading projects: `project_list.rs`; which Telegram threads show: `telegram.rs`); what is done to a thread or project there — pin (a thread's), color, rename, delete, ↑ ↓ ⌫ — `src/sidebar/items.rs`, marks in `src/sidebar/marks.rs` |
| the projects' order in the sidebar: a project's group dragged above or below another | `src/sidebar/order.rs` (the order, `project_rank`, `drop_zone`, `drag_list`, kept in `project-order.json`), its handle the head and its place the whole group in `project_row.rs`, the sort in `items.rs`, the end of the list in `mod.rs` (a project has no pin; a thread does, `marks.rs`); the line and the half-of-a-block arithmetic `bench_ui::drop_line`, `drop_edge` |
| the agent's item, the gear (the left dock's foot) | `src/sidebar/agent.rs` |
| the Code view's status bar (caret, thread beside, terminal) | `src/explorer/statusbar.rs` |
| the line under the composer (project, repo, meters) | `src/thread/composer/foot.rs` |
| ⌘K, the search over threads and projects | `src/sidebar/search.rs` |
| change a thread's meters (context, speed, fallback, by its repo) | `src/thread/meters.rs`, the speed meter `src/thread/speed.rs` |
| change the gear (tabs Agents, Presets, Appearance) | `src/settings/mod.rs` (the tabs, built from `bench_ui::settings_*`), `providers.rs` (a provider's card: Make main, its fallback), `voice.rs`, `evaluator.rs`, `server_form.rs` (`ServerForm`, what the server cards share), `agent_form.rs` (add/remove a server's agent), `presets.rs`, `appearance.rs`, `state.rs` (`Gear`); `src/settings/SPEC.md` |
| change a keyboard shortcut | `src/app/keys.rs` (the built-in inserts' shortcuts on or off: `src/app/shortcuts.rs`); the macOS menu (Settings… ⌘,, New Window ⌘⇧N) `src/app/menu.rs` |
| change a /tools utility | `crates/devtools/src/` (pure functions with tests) |
| change what Bench sends or receives | a method or an event: `crates/protocol` (`rpc.rs` one type per method with its answer, `event.rs`, `thread.rs`), its handler in `agent/src/modules/`, Bench's side `app.request`/`send`/`send_at` (`src/app/agents.rs`), events in `src/app/events.rs`; the socket `src/client/rpc/`; REST `src/client/rest/` |
| change the agent's answer to an RPC or a REST route | `agent/src/modules/<area>.ts` |
| change how many requests the model's server is sent at once, the line threads wait in | `agent/src/runtime/streams.ts`, its gate in `agent/src/extensions/assistant.ts`, the field on the provider's card `src/settings/providers.rs` |
| change how the agent runs a thread | `agent/src/runtime/runtime.ts` (the core) and its part for the job (`stream.ts`, `queue.ts`, `hold.ts`, `asks.ts`, `inserts.ts`, `title.ts`, `models.ts`); what Bench reads of it: `agent/src/runtime/journal.ts` |
| change how work is done on a server (a project's machine over SSH) | `agent/src/machine/ssh.ts` (Pi's `ExecutionEnv`; its tests are Pi's conformance suite, `test/ssh-env.test.ts`) |
| change the agents Bench talks to (this Mac's, servers') | `src/app/agents.rs` (links), the list of servers' agents in `agent/src/builtin/servers.ts`; adding/removing in `src/settings/agent_form.rs` |
| change where a project lives (agent, machine, folder) or how the sidebar's projects are put together | `src/app/places.rs` (`ProjectKey`, `Location` in `src/app/model.rs`); its rows in `src/env/place.rs` |
| keep a project's agent in its folder | `agent/src/machine/confined.ts` |
| change the presets (MCP, LSP, compaction, reasoning levels) | defaults in `agent/src/builtin/presets.ts`; Bench's side `src/settings/presets.rs`, the gear's tab in `src/settings/mod.rs` |
| the evaluator (a System One model asked at searches, on the way, at a turn's end) | `agent/src/judge/` (`SPEC.md`; a new point: a `Point` file and a line in `POINTS`, `index.ts`; `models.ts` pi-ai's classifier `bench/evaluator` and its servers; `point.ts`, `search.ts`, `plan.ts`, `drift.ts`, `done.ts`…), the turn they read `agent/src/turn/turn.ts`; hooked in `agent/src/extensions/assistant.ts`; REST `/api/judge` in `agent/src/modules/settings.ts`; the gear's section `src/settings/evaluator.rs`; a project's switch `src/env/run.rs`; the counts `src/thread/meters.rs` |
| a project's step limit | `agent/src/turn/steps.ts`; its row `src/env/run.rs` |
| what reaches the model fits its window (a tool's result cut, a request cut to fit) | `agent/src/turn/window.ts`, hooked in `agent/src/extensions/assistant.ts` (`afterTool`, `beforeRequest`, `afterResponse`) |
| how an MCP server's tools reach the model and the scripts (`exposure`: `codemode`, `direct`, `deferred`, `hidden`) | `agent/src/mcp/mcp.ts`, `agent/src/mcp/SPEC.md`; set in the server's JSON entry, its document `src/env/mcp/` |
| code mode (a script calls the thread's tools, `models.*`, MCP namespaces) | `agent/src/extensions.ts` (`loadExtensions`, `forAgent`); a script's evaluator questions `agent/src/extensions/assistant.ts` |
| change an agent tool | `agent/src/builtin/<tool>.ts`, its prompt line in `agent/src/prompt.ts`; grep, glob, ls: `agent/src/builtin/search.ts` |
| change a project's language servers (diagnostics after every edit `agent/src/builtin/diagnostics.ts`; the editor's problems, hover, definition via `agent/src/modules/lsp.ts`) | `agent/src/lsp/lsp.ts`, the presets in `agent/src/lsp/presets.ts`; the section `src/env/lsp.rs` |
| change when a project's conversations are compacted | `compactionPolicy` in `agent/src/config.ts`; the row in `src/env/run.rs` |
| change the demo (screenshots) | `src/demo/`: `hooks.rs` (`DemoHooks`: a hook's variable and doc comment, read once at start), `script.rs` (what the hooks ask of the window), `agent.rs` (the fake agent's answers), `env.rs`, `threads.rs`, `journal.rs` (fixtures), `probes.rs` (real input); the seam `src/client/fake.rs`; the table of hooks `.agents/skills/bench-kit/references/build-verification.md`; every state against a reference `.agents/skills/bench-kit/scripts/pixels/` |

## The tree

The one description of the source layout. What each module is for: its `SPEC.md`.

```
crates/
  ui/            bench_ui: every element and size (theme, icons, Gap, text, buttons, fields, blocks,
                 rows, forms, lists, bars); the only place with pixel values
  storybook/     all elements on one page (cargo run -p storybook); reference/ has the pictures
  protocol/      the wire: rpc.rs (Request: one type per method with its answer), event.rs (Event: what
                 the agent says unasked), thread.rs (ThreadOpen, ThreadEvent); cargo test -p protocol
                 writes agent/src/protocol.gen.ts
  thread/        the thread as rows from Pi's journal: rows.rs (rules of order, tests.rs), steps.rs
                 (the opened work), tool_labels.rs (a call in one line), diff.rs
  devtools/      the /tools utilities, pure functions with tests
src/
  main.rs        start: demo::install(), the window, demo::attach()
  app.rs         BenchApp: the window's state, one field per area, and its construction
  app/           the whole app: model.rs (ProjectKey, Location, ProjectEntry, Target), places.rs (where a
                 project lives), agents.rs (the links to agents), events.rs (protocol::Event applied),
                 keys.rs, shortcuts.rs, menu.rs (the macOS menu), armed.rs (what waits for a second
                 press), defaults.rs, subscriptions.rs (what new() listens to, observe_changes),
                 backend.rs (the agent's restart, its log), repo.rs (a thread's git repo)
  client/        talking to the agent, as Zed's client: rpc/ (Client, Incoming), rest/ (with_settings),
                 connection.rs (where it runs, the tunnel), launch.rs (installing, here or over SSH),
                 service.rs (launchd, systemd), probe.rs (bench --probe), fake.rs (FakeAgent, the seam
                 the demo plugs into)
  workspace/     the window as Zed's: dock.rs (Dock, Panel), mod.rs (Center, render_panel), layout.rs
                 (the docks around the center, the file drop's layer), history.rs (back and forward)
  titlebar/      the bar over the center: mod.rs, tail.rs (Chat | Code)
  sidebar/       the threads panel: mod.rs (New thread, New project, Search), search.rs (⌘K),
                 project_row.rs, project_list.rs, order.rs (a project dragged), thread_row.rs, menu.rs,
                 items.rs (a thread's pin, color, rename, ⌫⌫, ↑↓), marks.rs, telegram.rs, agent.rs (the left
                 dock's foot: the agent, its popover, the gear), state.rs
  thread/        a thread on screen: view.rs (ThreadView), active.rs (ActiveThread, ThreadRef), mod.rs,
                 list.rs (gpui::list), look.rs (chat or document), meters.rs, speed.rs, open.rs, store.rs,
                 panel.rs, beside.rs
    rows/        one file per kind of row: message, work, steps, queued, asks, clarify, processes,
                 changed_files, diff_stats, pages, welcome
    composer/    the next message: input, slash, prompts, attachments, drafts, send, foot
  settings/      the gear: mod.rs (tabs Agents, Presets, Appearance), providers.rs, voice.rs, evaluator.rs,
                 server_form.rs, agent_form.rs, presets.rs, appearance.rs, state.rs; new_project.rs
  inserts/       mod.rs (InsertKind, KINDS, Inset), slash.rs (the / list), state.rs (each thread's
                 inserts); one folder per kind: terminal/, tools/, browser/ (mod.rs, state.rs, view.rs)
  env/           a project's environment: section.rs (Section), mod.rs (FORMS, PARTS), page.rs (Settings),
                 one file per section (name, place, run, telegram, instructions, lsp, schedule, mcp/, backup)
  explorer/      the Explorer: tree/, pane/ (doc.rs DocKey, item.rs Item, file.rs, tabs.rs, comment.rs,
                 persistence.rs), search/ (⌘P, ⌘⇧F), code/ (lsp, problems, git and turn_diff, lang),
                 specs/, statusbar.rs (SPEC.md)
  media/         what an answer carries besides text and how it shows: view.rs, text.rs, playback.rs,
                 chip.rs, fetch.rs, diagram.rs, latex.rs; attachments and sound: attach.rs, audio.rs,
                 video.rs, voice.rs, pauses.rs, sound.rs
  system/        what Bench asks of the machine: ssh.rs, sshconfig.rs, notify.rs
  demo/          a fake agent for screenshots and the web demo: hooks.rs, script.rs, agent.rs, env.rs,
                 threads.rs, journal.rs, probes.rs
agent/src/
  server.ts      starts the agent, collects modules/
  modules/       each area's RPC and REST: threads, commands, projects, agents, presets, cron, files, lsp,
                 audio, settings, backup, specs
  runtime/       the threads' core: runtime.ts (make, open, send, stop, fork); parts reached as rt.<part>:
                 models, fallback, stream, journal (Journal), hold, streams, queue, asks, inserts, title, usage,
                 changes, review, steer, thinking; tools-host.ts (the tools' host, envFor), run.ts
  prompt.ts      the prompt sections
  extensions.ts, extensions/   Pi's extensions: loaded, chosen per agent (forAgent); assistant.ts (the
                 default: tools, sections, hooks), spec-note.ts
  builtin/       the agent's own tools and stores: ssh, process, cron, memory, skills, ask, projects,
                 servers, presets, search, navigate, hashline, diagnostics, approvals, review
  turn/          what the hooks keep of a turn: turn, steps, window, frozen, compaction, kept
  judge/         the evaluator: models.ts (bench/evaluator), index.ts (POINTS), a file per point
  machine/       where tools work: ssh.ts, confined.ts, settings.ts, sh.ts
  mcp/, lsp/     MCP servers (mcp.ts, oauth.ts); language servers (lsp.ts, presets.ts)
  telegram.ts, config.ts, store.ts, specs.ts, host.ts, http.ts, instance.ts, repo.ts, audio.ts
.agents/skills/  what the agent reads: bench-map (this), bench-concepts, bench-kit, bench-structure, pi-durable-dev-rules, gpui-kit
assets/shots/    the README's pictures, made by scripts/readme-shots.sh
.agents/skills/  bench-kit (how to change Bench), bench-structure (how to grow it), bench-repo (the README,
                 the About box, the web demo, a push), pi-durable-dev-rules, gpui-kit and its guides,
                 rust-best-practices
scripts/         bundle-macos.sh, build-web.sh, readme-shots.sh, gen_themes.py, specs-check.mjs
```

## Names worth knowing

- **Row** (`crates/thread`): one line of the thread — User, Work, Answer, Insert, Fail, Writing, Queued.
- **Work**: the agent's steps between two messages; counted per message, never per run.
- **Insert** / **InsertKind** (`src/inserts/mod.rs`): a live tool in the thread and its kind; each method is told its thread (`ThreadRef`).
- **ThreadView** (`src/thread/view.rs`): a thread on screen, an entity; **ThreadRef**: which thread, as the app is told it.
- **FakeAgent** (`src/client/fake.rs`): what answers instead of a real agent in the demo, as Zed's `FakeFs`.
- **Section** (`src/env/section.rs`): one part of a project's environment; a `SettingsForm` (fields on its Settings page) or `ExplorerParts` (parts in the Explorer's tree).
- **Journal**: Pi's entries of a conversation; Bench's thread is read from it, nothing else.
