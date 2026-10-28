# Adding a tool to Bench: a new kind of insert

Bench has no modes and no tool pages. A thread is a **document**: the person's messages, the
agent's answers, and the person's **inserts** — small live apps put into the conversation with
`/`, part of what the person says, right where they put them (2026-10-25, the person: "this is
the only approach in this app now; the only classic settings left are the gear"). An insert
never switches the thread or renames it; a thread can hold several; coming back to a thread
brings them back as they were. Bench has four kinds, and each is a worked example of this page.
**The browser is the centrepiece** — the thread's own tab, live, instead of a screenshot — so
read it first; the others are simpler cases of the same shape.

| Key (state) | `/` word | What's live | The agent's tool | What Bench sends with each message |
|---|---|---|---|---|
| `browser` {tab, url} | `/browser` | the thread's own tab of the shared server browser (`src/inserts/browser/`: `mod.rs` the kind, `state.rs` a tab per thread, `gateway.rs` the stream) | the browser MCP (Playwright on the server: browser-server-mcp, its own project) | the tab's address and title, and "work only in it" |
| `tasks` {list} | `/tasks` | every list a click apart; in a project its tasks only (`src/inserts/tasks/`) | `tasks` (`agent/src/builtin/tasks.ts`) | which list is shown |
| `ssh:local` (on the thread's machine, `thread_machine(at)`: this Mac or the project's server) | `/terminal` | the terminal, as tall as its text (`src/inserts/terminal/`) | `ssh_run` (`builtin/ssh.ts`) | the terminal's last 60 lines |
| `tools` {tool} | `/tools` | every utility in its title's menu, the tool's input → output (`src/inserts/tools/`, the utilities themselves in `crates/devtools`) | none: `read`/`bash` on what it is sent | the tool's input and output |

What belongs to a **project** rather than to a thread (its soul, MCP servers, skills, prompts,
Telegram bot, scheduled jobs) is not an insert: it is the project's Settings page
(`src/env/`, one file per section).

## How it is kept: Pi's terms

The agent keeps the inserts (`agent/src/runtime/inserts.ts`), so they are durable and every window sees
the same:
- an **entry** `bench.insert` {id, key} in the conversation's transcript, written where it was
  put (a Pi *write*: the model doesn't see it, it is never queued, it lands at once even mid-run);
- its state in the conversation's **document** `bench.inserts` ({items: {id: {key, state, folded,
  removed, after?}}}), changed in commits. A removed insert keeps its entry and is marked removed.

RPC: `session.insert` {id, key, state}, `session.insert.patch` {id, patch}; every change is
broadcast as `session.inserts` and `thread.inserts`. Where an insert sits in the thread is its
entry's place in the journal (`crates/thread`); `after` in its state moves one that was put while
a message still waited in the composer to just after that message. Bench changes an insert on screen at once and sends the
change; the agent's answer replaces Bench's list (the truth). A new chat's session is created when
its first insert comes, not with the first message. Tests: `agent/test/inserts.test.ts`.

## The pieces of a new kind

**1. A key and its state.** A key names the kind (several inserts of a kind can sit in a
thread); everything that changes as the person works is `state` (a JSON object). Map the `/`
row's key to (key, state) in `insert_spec` (`src/inserts/state.rs`): "tasks:<list>" → tasks {list}.

**2. An `InsertKind`** — a folder `src/inserts/<kind>/`: `mod.rs` holds the unit struct, a line in
`KINDS` (`src/inserts/mod.rs`) registers it:
Every method but `owns` and `shared` is told the thread the insert sits in, `at: &ThreadRef`
(its session, project, run): a thread on screen is its own `ThreadView` (the open one, or the
one beside it), so the kind never asks "the open thread" — it reads `at` (`app.thread_machine(at)`,
`app.inserts_of(at)`, `app.patch_insert(at, …)`). Its state is the app's (`&mut BenchApp`,
`Context<BenchApp>`), and so are its listeners.
- `owns(key)`; `shared` — a name when its live state is one for the whole app (`tasks`,
  `tools`): of several such inserts one is live (the last clicked, else the
  newest), the others say "click to show it here". The browser has none: its tab is the thread's;
- `title`; `switches` — the title's menu (another list, another tool): each a patch of the insert;
- `controls` — the head's own buttons; the frame adds fold and remove;
- `body` — what it shows, under the head (None: the head alone). Its height fits what it shows
  (editors at `editor_height`, never scrolling inside; no scroll inside the thread's scroll except
  the terminal). While it renders, it restores its state (the tools insert shows its utility
  again); what the person changes goes back with `patch_insert`;
- `follow` — what it opened (the open file, the open task) as the next row of the thread, while
  it is live and unfolded;
- `context` — what it shows now, as text for the agent (cut long text at 6000 characters);
- `opened` (put in, or brought back: start a terminal), `removed`.
The frame is the same for every kind (`ThreadView::render_insert`, drawn by the thread it is in): the person's avatar, a light ground, a
thin border, round corners, `block_head` on top (one height and look for every block's head).
State the kind needs while it lives is a struct in `src/inserts/<kind>/state.rs`, a field of
`InsertsState` (`src/inserts/state.rs`; `app.inserts.ssh`, `.tools`, `.tasks`, `.sites`); a long body goes to `view.rs` beside it. Build it from `bench_ui`
elements (`crates/ui`, shown in the storybook). Its clicks set `this.click_claimed = true`. What
it calls on the app while it draws must never read a `ThreadView` (`self.thread(cx)`,
`active_thread.read`): the view drawing it is borrowed then — use `at`.

**3. Its `/` word.** The trait's `word()` (the word, its aliases, what it is, the key it puts
in); `choices()` for the rows of `/word <query>` when picking straight away helps (`/tasks
today`). The "/" list reads them from `KINDS` (`inserts/slash.rs`).

**4. The agent's side**, in `agent/`:
- **A tool** for its work: `defineTool` in `agent/src/builtin/<kind>.ts`, in the assistant's
  `tools` (`agent/src/extensions/assistant.ts`). It finds what the thread has inserted in
  `sessionOf(api.conversationId).inserts` (see `ssh.ts`), so the agent never has to name what
  the thread shows. When an MCP server already does the job, use it (the browser).
- **A prompt line**: its case in `inserted()` (`agent/src/prompt.ts`): what it is, which tool works
  on it, what not to do ("never look for the tasks in files"; "don't take screenshots to show the
  page"). The `inserts` section is built from the document, so it changes only when an insert
  comes, goes or switches, and the prompt cache keeps.
- **Its live state**: Bench sends every insert's `context` with each message; the agent appends it
  inside `<block-state>` for the model and strips it from the history (`runtime.ts` `submit`,
  `stripBlock`).

Then: `cargo build`; put it into a thread; ask the agent something about "this" and watch which
tool it calls; screenshot (`build-verification.md`). Add a demo thread with it in `src/demo/mod.rs`
(`thread(…, "<key>", …)` makes the insert), so the web demo and the README show it.

## How the browser insert works, end to end

The pattern for anything live from elsewhere:
1. The browser runs on a server (browser-server-mcp, its own project at `~/Projects/browser-server-mcp`:
   Chrome, the Playwright MCP the agent drives, and a `/live` gateway for people: a personal key, a narrow set of operations, a log).
2. Each thread has its own tab (2026-10-25, the person: "one thread works with one site, another
   with another; if the tab falls off, the thread opens a new one through MCP"). The insert's
   state keeps {tab, url}. The tab the agent opens or moves while it answers in the open thread
   becomes that thread's (`on_browser`, `src/inserts/browser/state.rs`); a tab woken under a new id keeps its
   thread; a closed one is opened again at its address ("Open again", `reopen_thread_tab`).
3. The insert streams that tab (≤ 2 frames a second, only while the thread is open and someone
   looks; paused, it keeps the last frame) and maps the pointer and keys to the page
   (`render_tab`, `src/inserts/browser/view.rs`).
4. `context` tells the agent the tab's address and to work only in it (select it with
   `browser_tabs`, or open a new one when it is gone); the prompt line says the same.

## Where the data lives

The tool's records live where the agent can reach them with Bench closed: in `~/.bench`
(`bench.sqlite`, through the agent's REST API: the tasks, `/api/tasks`) or where they already
live (`~/.ssh/config`, a server's files, a website behind the browser). Never only in Bench's
memory.
`build-native-data.md`.

## Patterns for a body

- **Input → output** (a tool): two panes side by side, `h_flex().size_full().items_stretch()`;
  `block_head` over each pane with its title and small buttons (copying is an icon);
  `Editor::new(&state).appearance(false)` in a `div().flex_1().min_h_0()`. `src/inserts/tools/view.rs`.
- **A list re-read from its source** (tasks, a folder): re-read it now and then and replace the
  list, with three rules, each from a real bug: keep what exists only here (a new item's
  temporary id `new:<token>`); don't overwrite a card being typed into (save 700 ms after the last
  key, then re-read); every row in a scrolling list gets `.flex_none()`. `src/inserts/tasks/state.rs`.
- **A terminal**: as tall as its text, while its program sees a whole screen
  (`Session::set_min_rows`, `src/inserts/terminal/session.rs`): `ls` must not shrink it to one line.
- **Something live from elsewhere** (a terminal, a browser tab): a worker thread owns the
  connection and sends events over an `async_channel` that a `cx.spawn` loop drains into
  `cx.notify()`; stream only while someone looks.
- **Work that takes time**: `cx.spawn_in(window, async move |this, cx| { let r =
  cx.background_spawn(async move { … }).await; this.update_in(cx, |this, window, cx| …) })`.
- **"!" from any thread**: `!command` types into the thread's terminal insert, or puts this Mac's
  in (in the thread's folder): `run_bang`, `inserts/slash.rs`.

## Checklist: every file a new kind touches

In order, `cargo build` after each line:
1. Pure logic (parsing, formats, the data type) — a crate in `crates/<name>/` with tests, like
   `crates/devtools`, when it needs no GPUI.
2. `agent/src/builtin/<kind>.ts` — the agent's tool (or an MCP server in the project's
   `mcp.json`); the REST routes Bench reads the same data with in a module `agent/src/modules/<kind>.ts`,
   a line in the list in `agent/src/server.ts`.
3. `src/inserts/<kind>/state.rs` — what it keeps while it lives and its actions; its field in
   `InsertsState` (`src/inserts/state.rs`).
4. `src/inserts/<kind>/mod.rs` — its `InsertKind`; `pub mod <kind>;` and a line in `KINDS`
   (`src/inserts/mod.rs`). A long body in `view.rs`.
5. Its `word()` and `choices()` in the `InsertKind`; `src/inserts/state.rs` `insert_spec` when a
   row carries state.
6. `agent/src/prompt.ts` — its line in `inserted()`.
7. Icons: SVGs in `assets/icons/`, lines in `crates/ui/src/icons.rs` (`design-icons.md`).
8. `src/demo/mod.rs` — a demo thread with it (`Journal` builds made-up journal items).
