# The window's three places, and the Explorer

Bench has two ways to look at a project: as **threads** (talking to the agent, the work it
did) and as **files** (what the project is made of). They share one window, built of three
places that every view fills in its own way:

```
┌──────────────┬───────────────────────────────┬────────────────┐
│ Sidebar      │ The middle                    │ Beside         │
│ (SidebarPanel) │ (Center)                        │ (optional)     │
│              │                               │                │
│ Threads      │ Chat — the open thread        │ a file kept    │
│ Explorer     │ Doc — the open document, tabs │ next to the    │
│              │ Providers — the gear          │ middle, for    │
│              │ Project — a project's Settings│ reading while  │
│              │ NewProject                    │ talking        │
└──────────────┴───────────────────────────────┴────────────────┘
```

- **The sidebar** lists one thing at a time: `SidebarPanel::Threads` (projects and their threads)
  or `SidebarPanel::Explorer` (one project's tree). The switch is at its top; ⌘E flips it. A new
  way to look at things (search across threads, git, tasks) is a new `SidebarPanel` with its own
  list, nothing else changes.
- **The center** is `Center` (`src/app/workspace.rs`): what the sidebar's click opened. A thread's click shows `Chat`; a
  file's click shows `Doc` with a tab per open document (the header holds the tabs instead of
  the thread's title). Leaving the Explorer does not close documents: their tabs come back.
- **Beside** holds a file next to whatever the middle shows (`Explorer::beside`): open one with
  the tree row's panel icon, ⌥-click, or a document's own "Open beside". It is how a file is
  read or fixed while the thread runs.

Sizes come from `bench_ui` (`crates/ui/src/ide.rs`: the tree rows, a document's head, the tabs,
⌘P's list).

In Zed's words, which GPUI code follows (Bench is built on Zed's framework): the window is the
**workspace**; the sidebar is its left **dock**, whose **panels** are Threads and Explorer (as
Zed's project panel); the middle is the center **pane**, whose tabs are **items** (a document);
Beside is the right dock. Shared pieces drawn the same everywhere are components in
`crates/ui`, as Zed's `ui` crate. Use these words when naming new code.

## The Explorer (`src/explorer/`)

One project at a time (the picker at the sidebar's top; it opens on the open thread's project).
Its tree:

| Group | What | Where it is |
|---|---|---|
| the folder | the project's code, listed as folders open | its machine (`/api/fs/*?machine=` when this Mac's agent works there over SSH) |
| Instructions | `SYSTEM.md` (its soul), prompts, skills by where they live | the agent's own files (`~/.bench/projects/<id>/`), a folder's `.agents/skills` |
| Tools | MCP servers, language servers | the environment's parts (`Section`s whose `home()` is `Explorer("Tools")`) |
| Schedule | scheduled jobs | the agent's cron (`Explorer("Schedule")`) |

Everything that is a file is opened as one: an editor of its own (`explorer/doc.rs`, `Doc`),
read and written through `/api/fs/read-text` and `write-text`; saving the agent's own text reads
the environment again (the composer's @ list follows). What is not a file — an MCP server with
its state and sign-in, a job with its schedule — is a **part**: a `Section`'s `items()` in the
tree and its `card()` in the middle (`DocKey::Part`). A new kind of thing in a project is one of
the two: files need nothing new; a part is a `Section` with `home() == Explorer(<group>)`.

The tree's work (`explorer/ops.rs`): folders are listed when they open (`/api/fs/list`), a name
is typed in place for a new file, folder, prompt, skill or a rename, a delete is asked once.
⌘P opens a file by name (`/api/fs/files`: ripgrep's list of the folder), ⌘⇧F finds a text in it
(`/api/fs/grep`), ⌘S saves, ⌘W closes the tab.

## Settings stay settings

A project's Settings (`Center::Project`, `src/env/page.rs`) are its fields: name, where it lives,
how it runs, its Telegram bot, its removal — `Section`s whose `home()` is `Settings` and that
draw a `form()`. What a project *contains* is in the Explorer; what it *is set to* is in Settings.

## Where the IDE goes

The Explorer is one of Bench's three cores, beside threads and projects: the person reads and
checks the code here, the agent writes it in threads. It stays far smaller than Zed (which is
one click away) and grows by putting the same parts in more places, not by new copies:

- **Docks hold panels, the center holds items** — as in Zed. The right dock shows one panel at
  a time (`RightPanel`): the open **thread** beside the code (the same `render_chat` the center
  shows, so the IDE costs no second chat), or a **file** kept beside. In the threads view the
  right dock shows only a file (the thread is the center).
- **Code goes to the thread as references.** ⌘L sends the selection (or the whole file) to the
  composer as a chip naming the file and lines; a tab dragged onto the thread, or its menu's
  "Add to thread", sends the file. A message carries them under its words as fenced blocks with
  `path:lines` — what the agent reads, what the thread shows. The same chips will carry a
  language server's problems and a terminal's output.

Phases, each a thing of its own:

1. **Tabs and the thread beside** (done): tabs drag to reorder and have a menu (close others,
   to the left, to the right, saved, all; copy the path; open beside; add to the thread); the
   right dock's panels; references from the code to the thread.
2. **Language servers through the agent.** The agent already runs a project's servers
   (`agent/src/lsp.ts`); the editor already takes LSP providers (gpui's `Lsp`: diagnostics,
   hover, go to definition, completion). A small REST bridge (`/api/lsp/{diagnostics,hover,
   definition}`) and providers in `explorer/lsp.rs` give underlined problems, hover and ⌘-click
   to the definition; a Problems list; "Send problems to the thread".
3. **A terminal in the bottom dock** on the project's machine — the `/terminal` insert's session
   (`src/inserts/terminal/session.rs`) in a dock instead of a thread; "Send output to the thread".
4. **Git at a glance**: changed lines in the gutter against HEAD, a changed file's diff.

## Growing it

- Another source of files in the tree (a second folder, a worktree): a group in `tree.rs` whose
  rows are `FileRef`s — opening, saving, renaming already work for any of them.
- A diff view, an image viewer, a Markdown preview: another body in `view.rs` `file_pane` for
  the kind of file (`Doc::binary` already says "not text").
- Tabs beside, a split middle: `beside` becomes a list; documents are already shared by key.
- A thread opening a file (a changed file in its answer) beside itself: `open_beside`.
