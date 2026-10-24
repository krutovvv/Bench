# Adding a tool to Bench: a new kind of thread

Bench has no modes. Everything is a thread with the agent, and a tool is a **subject**: what a
thread is about, drawn live at its top, with the conversation under it (Zed's Delta, where a
thread is the unit of work, extended with blocks). Bench has five, and each is a worked example
of this page. **The browser is the centrepiece**: it shows the idea most clearly — the agent's
step is a live page right in its answer, and the person clicks and types in it — so read it
first; the others are simpler cases of the same shape.

| Subject key | Sidebar group | What's live | The agent's tool | What Bench sends with each message |
|---|---|---|---|---|
| `browser:<id>` | Browser (+) | the server browser's tab of each step, live in that turn's answer (`render_site_step`, `src/ui/sites.rs`; state and streaming `src/app/sites.rs`, `src/remote_browser.rs`) | the browser MCP (Playwright on the server, `browser-server/`) | the live tab's address and title |
| `tasks:<list>` | Tasks (Today, Upcoming, Anytime, Logbook) | the task list at the top (`render_tasks_block`, `src/ui/tasks.rs`) | `tasks` (`agent/src/builtin/tasks.ts`) | nothing: the agent reads the list with its tool |
| `ssh:<alias>` (`ssh:` alone is this Mac) | Servers | the server's terminal at the top (`render_terminal`, `src/ui/terminal.rs`) | `ssh_run` (`builtin/ssh.ts`) | the terminal's last 60 lines |
| `files:<host id>` | Files | an FTP/SFTP folder and file editor at the top (`render_files_block`, `src/ui/remote_files.rs`) | `remote_files` (`builtin/remote.ts`) | the folder, its entries, the open file |
| `tool:<name>` | Tools | a utility's input → output at the top (`render_tool_block`, `src/ui/tools.rs`) | none: `read`/`bash` on what it is sent | the tool's input and output |

**How the browser thread works**, end to end (the pattern for anything the agent makes live
during a turn):
1. The browser runs on a server (`browser-server/`: Chrome, the Playwright MCP the agent drives,
   and a `/live` gateway for people: a personal key, a narrow set of operations, a log). Set-up:
   `browser-server/README.md`.
2. The prompt section (`subject()` in `agent/src/prompt.ts`) tells the agent to open a new tab
   each turn and not to screenshot the page for the person.
3. Bench watches the gateway's tab list; a tab that appears (or moves to a new page) during a turn
   becomes that turn's **step** (`on_browser`, `steps_of`), drawn in the answer, streamed live
   while it is the latest (≤ 2 frames a second, only while someone looks), clickable and typable
   (`render_site_step` maps the pointer and keys to the page).
4. Each step's last frame is saved in the session's folder, so the thread still shows what the
   agent saw after a restart; a click reopens the page.
5. `block_context` sends the live tab's address and title with each message.

So a new tool for a person ("my clients", "the shop's orders", "the build queue") is the same
five pieces. Copy the subject closest to it.

## The five pieces

**1. A key.** `<kind>:<value>`, stored on the session (`SessionRow.subject`, sent with
`session.create`). Parse it in one place: `subject_<kind>(key) -> Option<…>`, like
`subject_server` / `subject_tool` (`src/app/subjects.rs`), `subject_list` (`app/tasks.rs`),
`subject_files` (`app/remote_files.rs`); add its branch to `open_subject_key` and
`subject_title` in `subjects.rs`.

**2. Opening it.** A subject has **many threads, one job each** (the user: "first we did one
thing on the server, then another, then a third by hand" — each its own thread, never one
endless thread, and never split automatically). `open_<kind>_thread(value, window, cx)`:
prepare the block's state (connect, load, start a terminal), then `self.open_subject(key, cx)`.
That opens the latest thread about the subject, or a new chat that will carry it
(`new_subject`); `new_subject_thread(key)` (the row's "+") always starts a new one, and
`open_subject_thread(key, id)` opens a given one — both go through the same
`open_<kind>_thread` path via `subject_target`, so the block is ready either way. The block
belongs to the subject, not the thread: every thread about `web-1` shows the same terminal. Bench's: `open_server_thread`,
`open_tool_thread` (`subjects.rs`), `open_task_list` (`app/tasks.rs`), `open_files_thread`
(`app/remote_files.rs`), `open_browser_thread` / `new_browser_thread` (`app/sites.rs`).

**3. The block.** One branch in `render_chat` (`src/ui/chat.rs`, the `let block = …` chain)
returning the view. The chat puts it in an agent row (the agent's avatar in the gutter), on top
of the conversation, and its clicks are its own (set `this.click_claimed = true` in its
handlers; a click anywhere else in the thread focuses the composer). Rules:
- Built from the catalog's pieces (`ui-catalog.md`), the thread's text size (`text_base`; code
  and terminals monospaced), muted color for what is secondary. No bars or frames of its own
  beyond a 1px border where a surface needs one.
- Its height fits what it shows, up to a cap (a terminal fits its text up to 280; a tool's block
  is 440): the conversation must stay in view.
- State the block needs lives in the app, not in the view: `self.<kind>` (a struct in
  `src/app/<kind>.rs`, its field on `HermesApp`, its `new()` in `HermesApp::new`).

**4. The sidebar group.** A head with an icon in a fixed square, a name, a chevron to fold and an
optional "+" that adds a new thing to work on — a server, a host (`subject_head(id, icon, name,
plus, cx)`, `src/ui/sidebar.rs`); then rows that call `open_<kind>_thread` (`subject_row`, with
`Some(key)` for the hover "+" that starts a new thread about it); under the open subject's row,
its threads (`thread_rows`, the latest first). One row per thing the person works on (a list, a server, a
host); a single row when there is one block with a picker inside it (Tools). Groups are listed in
`render_sidebar`.

**5. The agent's side.** Three parts, in `agent/`:
- **A tool** that does the subject's work with the thread's value: `defineTool` in
  `agent/src/builtin/<kind>.ts`, added to the assistant's `tools` (`agent/src/extensions/assistant.ts`).
  It reads the thread's subject from `sessionOf(api.conversationId).subject` (see `ssh.ts`), so
  the agent never has to name the server or list.
  Watch the first message of a new thread: the thread is created first and the sessions list
  arrives later, so the new row goes into `sessions` at once with its subject (`Evt::Created`),
  or `block_context` finds no subject and the block's state is lost. When an MCP server already does the job, use
  it instead and write no tool (the browser).
- **A prompt section** for the subject: a case in `subject()` (`agent/src/prompt.ts`): what the
  block is, which tool works on it, what "this" and "here" mean, what not to do (tasks: "never look
  for them in files"; browser: "don't take screenshots to show the page"). Two or three sentences.
- **The block's state**: `block_context` (`subjects.rs`) returns what the block shows right now
  as text; Bench sends it as `context` with `prompt.submit`, the agent appends it to the message
  inside `<block-state>` for the model and strips it from the history (`runtime.ts` `submit`,
  `stripBlock`). Send what the person sees and the model cannot get with a tool; nothing when
  the tool reads it anyway. Cut long text (6000 characters).

Then: `cargo build`; open the thread; ask the agent something about "this" and watch which tool
it calls; screenshot (`build-verification.md`). Add a demo thread for it in `src/demo.rs`
(`subject_threads`), so the web demo and the README show it.

## Where the data lives

The tool's records live where the agent can reach them with Bench closed: in `~/.bench`
(`bench.sqlite`, through the agent's REST API: Bench's Tasks and hosts, `/api/tasks`,
`/api/hosts`) or where they already live (`~/.ssh/config`, a server's files, a website behind
the browser). Never only in Bench's memory. `build-native-data.md`.

## Patterns for a block

- **Input → output** (a tool): two panes side by side, `h_flex().size_full().items_stretch()`
  (without `items_stretch` the panes shrink to their content); a 36px strip over each pane with
  its title and small square buttons; `Editor::new(&state).appearance(false)` in a
  `div().flex_1().min_h_0()`. Bench `src/ui/tools.rs`.
- **A list re-read from its source** (tasks, a folder): the agent and other devices change it
  too, so re-read it now and then (Tasks: every 4 s while the thread is open) and replace the
  list, with three rules, each from a real bug:
  - keep what exists only here (a new item not yet saved has a temporary id `new:<token>`;
    carry it over the re-read, or its card closes "by itself");
  - don't overwrite a card being typed into: keep the dirty copy, save 700 ms after the last key,
    then re-read;
  - every row in a scrolling list gets `.flex_none()`, or GPUI squeezes the rows to fit.
  Bench `src/app/tasks.rs` (`tick_tasks`, `drop_blank_new`), `src/ui/tasks.rs`.
- **Something live from elsewhere** (a terminal, a browser tab): a worker thread owns the
  connection, sends events over an `async_channel` that a `cx.spawn` loop drains into
  `cx.notify()`. Keep it light: stream only while the thread is open and someone looks (the
  browser pauses after 60 s idle and frames are capped at 2 a second), and keep the last picture
  when the source goes away (`sites.rs` saves each step's frame in the session's folder).
- **Something the agent made during a turn belongs to that turn**: the browser gives each tab
  opened during a turn to that turn and shows it in the answer, not at the top
  (`on_browser`, `steps_of`).
- **Work that takes time**: `cx.spawn_in(window, async move |this, cx| { let r =
  cx.background_spawn(async move { … }).await; this.update_in(cx, |this, window, cx| …) })`.
  Clipboard: `cx.read_from_clipboard()`, `cx.write_to_clipboard(ClipboardItem::new_string(…))`.
- **"!" from any thread**: `!command` runs in a terminal opened under that thread (a server
  thread types it into its own terminal): `run_bang`, `subjects.rs`.

## Checklist: every file a new tool touches

From adding Files (FTP/SFTP), in order, `cargo build` after each line:
1. `src/<logic>.rs` — the data type and its pure functions, tested; `mod` in `main.rs`.
2. `agent/src/builtin/<kind>.ts` — the agent's tool (or an MCP server in the agent's `mcp.json`),
   and the REST routes Bench uses to read the same data (`agent/src/server.ts`).
3. `src/app/<kind>.rs` — state + `open_<kind>_thread` and the actions; `pub(crate) mod` in
   `src/app.rs`, the field on `HermesApp`.
4. `src/ui/<kind>.rs` — `render_<kind>_block`; `mod` in `src/ui/mod.rs`.
5. `src/app/subjects.rs` — `open_subject_key`, `subject_title`, `block_context` (and
   `subject_<kind>`, here or in `app/<kind>.rs`).
6. `src/ui/chat.rs` — the block's branch in `render_chat`.
7. `src/ui/sidebar.rs` — the group (`subject_head` + rows) in `render_sidebar`.
8. `agent/src/prompt.ts` — the subject's case in `subject()`.
9. Icons: SVGs in `assets/icons/`, lines in `src/icons.rs` (`design-icons.md`).
10. `src/demo.rs` — a demo thread; the web build still compiles
    (`cargo check --target wasm32-unknown-unknown`: desktop-only crates go under
    `[target.'cfg(not(target_family = "wasm"))'.dependencies]`, with a stub for the browser build).
