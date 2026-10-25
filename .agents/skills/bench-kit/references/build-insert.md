# Adding a tool to Bench: a new kind of insert

Bench has no modes and no tool pages. A thread is a **document**: the person's messages, the
agent's answers, and the person's **inserts** — small live apps put into the conversation with
`/`, part of what the person says, right where they put them (2026-10-25, the person: "this is
the only approach in this app now; the only classic settings left are the gear"). An insert
never switches the thread or renames it; a thread can hold several; coming back to a thread
brings them back as they were. Bench has five kinds, and each is a worked example of this page.
**The browser is the centrepiece** — the thread's own tab, live, instead of a screenshot — so
read it first; the others are simpler cases of the same shape.

| Key (state) | `/` word | What's live | The agent's tool | What Bench sends with each message |
|---|---|---|---|---|
| `browser` {tab, url} | `/browser` | the thread's own tab of the shared server browser (`ui/inserts/browser.rs`; tab per thread, streaming: `app/sites.rs`, `remote_browser.rs`) | the browser MCP (Playwright on the server, `browser-server/`) | the tab's address and title, and "work only in it" |
| `tasks` {list} | `/tasks` | every list a click apart; in a project its tasks only (`ui/inserts/tasks.rs`) | `tasks` (`agent/src/builtin/tasks.ts`) | which list is shown |
| `ssh:<alias>` (`ssh:local` this Mac; `ssh` alone: pick one) | `/terminal`, `/ssh` | the terminal, as tall as its text (`ui/inserts/terminal.rs`) | `ssh_run` (`builtin/ssh.ts`) | the terminal's last 60 lines |
| `files` {host, path, file} | `/ftp` | the hosts (and a new one), the folder, the open file in an editor (`ui/inserts/files.rs`) | `remote_files` (`builtin/remote.ts`) | the folder, its entries, the open file |
| `tools` {tool} | `/tools` | every utility in its title's menu, the tool's input → output (`ui/inserts/tools.rs`) | none: `read`/`bash` on what it is sent | the tool's input and output |

What belongs to a **project** rather than to a thread (its soul, MCP servers, skills, prompts,
accesses, Telegram bot, scheduled jobs) is not an insert: it is the project's environment, the
block a project's pencil opens (`app/project_env.rs`, `ui/project_list.rs` `render_env_block`).

## How it is kept: Pi's terms

The agent keeps the inserts (`agent/src/inserts.ts`), so they are durable and every window sees
the same:
- an **entry** `bench.insert` {id, key} in the conversation's transcript, written where it was
  put (a Pi *write*: the model doesn't see it, it is never queued, it lands at once even mid-run);
- its state in the conversation's **document** `bench.inserts` ({items: {id: {key, state, folded,
  removed}}}), changed in commits. A removed insert keeps its entry and is marked removed.

RPC: `session.insert` {id, key, state}, `session.insert.patch` {id, patch}; `session.resume`
returns `inserts` (each with `after`: how many of the person's messages come before it); every
change is broadcast as `session.inserts`. Bench changes an insert on screen at once and sends the
change; the agent's answer replaces Bench's list (the truth). A new chat's session is created when
its first insert comes, not with the first message. Tests: `agent/test/inserts.test.ts`.

## The pieces of a new kind

**1. A key and its state.** A key names the kind (and, for terminals, which one: several can sit
in a thread); everything that changes as the person works is `state` (a JSON object). Map the `/`
row's key to (key, state) in `insert_spec` (`app/inserts.rs`): "files:<id>" → files {host}.

**2. An `InsertKind`** — one file in `src/ui/inserts/<kind>.rs`, a unit struct, a line in `KINDS`
(`src/ui/inserts/mod.rs`):
- `owns(key)`; `same(there, new)`: whether putting it again brings back the one there (default:
  one per thread);
- `title`; `switches` — the title's menu (another server, host, tool; "New …" runs a form);
- `controls` — the head's own buttons; the frame adds fold and remove;
- `body` — what it shows, under the head. Its height fits what it shows, up to a cap; the
  conversation must stay in view. While it renders, it restores its state (the files insert shows
  its host and folder again) and syncs what changed back with `sync_insert_state`;
- `context` — what it shows now, as text for the agent (cut long text at 6000 characters);
- `opened` (put in, or brought back: start a terminal, load the hosts), `removed`.
The frame is the same for every kind (`render_insert`): the person's avatar, a light ground, a
thin border, round corners, `block_head` on top (one height and look for every block's head).
State the kind needs while it lives stays in the app (`self.<kind>`, a struct in
`src/app/<kind>.rs`); its clicks set `this.click_claimed = true`.

**3. Its `/` word.** A line in `words()` (`ui/inserts/mod.rs`); rows for `/word <query>` in
`tool_slash` (`app/subjects.rs`) when picking straight away helps (`/ssh web-1`).

**4. The agent's side**, in `agent/`:
- **A tool** for its work: `defineTool` in `agent/src/builtin/<kind>.ts`, in the assistant's
  `tools` (`agent/src/extensions/assistant.ts`). It finds what the thread has inserted in
  `sessionOf(api.conversationId).inserts` (see `ssh.ts`, `remote.ts` `hostFor`), so the agent never
  has to name the server or host. When an MCP server already does the job, use it (the browser).
- **A prompt line**: its case in `inserted()` (`agent/src/prompt.ts`): what it is, which tool works
  on it, what not to do ("never look for the tasks in files"; "don't take screenshots to show the
  page"). The `inserts` section is built from the document, so it changes only when an insert
  comes, goes or switches, and the prompt cache keeps.
- **Its live state**: Bench sends every insert's `context` with each message; the agent appends it
  inside `<block-state>` for the model and strips it from the history (`runtime.ts` `submit`,
  `stripBlock`).

Then: `cargo build`; put it into a thread; ask the agent something about "this" and watch which
tool it calls; screenshot (`build-verification.md`). Add a demo thread with it in `src/demo.rs`
(`thread(…, "<subject>", …)` makes the insert), so the web demo and the README show it.

## How the browser insert works, end to end

The pattern for anything live from elsewhere:
1. The browser runs on a server (`browser-server/`: Chrome, the Playwright MCP the agent drives,
   and a `/live` gateway for people: a personal key, a narrow set of operations, a log).
2. Each thread has its own tab (2026-10-25, the person: "one thread works with one site, another
   with another; if the tab falls off, the thread opens a new one through MCP"). The insert's
   state keeps {tab, url}. The tab the agent opens or moves while it answers in the open thread
   becomes that thread's (`on_browser`, `app/sites.rs`); a tab woken under a new id keeps its
   thread; a closed one is opened again at its address ("Open again", `reopen_thread_tab`).
3. The insert streams that tab (≤ 2 frames a second, only while the thread is open and someone
   looks; paused, it keeps the last frame) and maps the pointer and keys to the page
   (`render_tab`, `ui/sites.rs`).
4. `context` tells the agent the tab's address and to work only in it (select it with
   `browser_tabs`, or open a new one when it is gone); the prompt line says the same.

## Where the data lives

The tool's records live where the agent can reach them with Bench closed: in `~/.bench`
(`bench.sqlite`, through the agent's REST API: tasks and hosts, `/api/tasks`, `/api/hosts`) or
where they already live (`~/.ssh/config`, a server's files, a website behind the browser). A
project's accesses are its own (`project` on the row). Never only in Bench's memory.
`build-native-data.md`.

## Patterns for a body

- **Input → output** (a tool): two panes side by side, `h_flex().size_full().items_stretch()`;
  `block_head` over each pane with its title and small buttons (copying is an icon);
  `Editor::new(&state).appearance(false)` in a `div().flex_1().min_h_0()`. `src/ui/tools.rs`.
- **A list re-read from its source** (tasks, a folder): re-read it now and then and replace the
  list, with three rules, each from a real bug: keep what exists only here (a new item's
  temporary id `new:<token>`); don't overwrite a card being typed into (save 700 ms after the last
  key, then re-read); every row in a scrolling list gets `.flex_none()`. `src/app/tasks.rs`.
- **A terminal**: as tall as its text, while its program sees a whole screen
  (`Session::set_min_rows`, `term.rs`): `ls` must not shrink it to one line.
- **Something live from elsewhere** (a terminal, a browser tab): a worker thread owns the
  connection and sends events over an `async_channel` that a `cx.spawn` loop drains into
  `cx.notify()`; stream only while someone looks.
- **Work that takes time**: `cx.spawn_in(window, async move |this, cx| { let r =
  cx.background_spawn(async move { … }).await; this.update_in(cx, |this, window, cx| …) })`.
- **"!" from any thread**: `!command` types into the thread's terminal insert, or puts this Mac's
  in (in the thread's folder): `run_bang`, `subjects.rs`.

## Checklist: every file a new kind touches

In order, `cargo build` after each line:
1. `src/<logic>.rs` — the data type and its pure functions, tested; `mod` in `main.rs`.
2. `agent/src/builtin/<kind>.ts` — the agent's tool (or an MCP server in the project's
   `mcp.json`), and the REST routes Bench uses to read the same data (`agent/src/server.ts`).
3. `src/app/<kind>.rs` — the state it needs while it lives and its actions; `pub(crate) mod` in
   `src/app.rs`, the field on `BenchApp`.
4. `src/ui/inserts/<kind>.rs` — its `InsertKind`; `mod` and a line in `KINDS` (`ui/inserts/mod.rs`).
5. `src/ui/inserts/mod.rs` `words()` — its `/` word; `app/inserts.rs` `insert_spec` when a row
   carries state; `app/subjects.rs` `tool_slash` for its rows.
6. `agent/src/prompt.ts` — its line in `inserted()`.
7. Icons: SVGs in `assets/icons/`, lines in `src/icons.rs` (`design-icons.md`).
8. `src/demo.rs` — a demo thread with it; the web build still compiles
   (`cargo check --release --target wasm32-unknown-unknown`: desktop-only crates go under
   `[target.'cfg(not(target_family = "wasm"))'.dependencies]`, with a stub for the browser build).
