---
id: explorer
type: submodule-design
status: draft
title: The window's three places, and the Explorer
parent: app
references: [workspace, env, thread-view]
covers: [src/explorer/]
---

# The window's three places, and the Explorer

Bench has two ways to look at a project: as **threads** (talking to the agent, the work it
did) and as **files** (what the project is made of). They share one window, built of three
places that every view fills in its own way:

```
┌──────────────┬───────────────────────────────┬────────────────┐
│ Sidebar      │ The middle                    │ Beside         │
│ (left_dock)  │ (Center)                      │ (right_dock)   │
│              │                               │                │
│ Threads      │ Chat — the open thread        │ the thread     │
│ Explorer     │ Doc — the open document, tabs │ beside a       │
│              │ Providers — the gear          │ document       │
│              │ Project — a project's Settings│ (RightPanel::  │
│              │ NewProject                    │  Thread)       │
└──────────────┴───────────────────────────────┴────────────────┘
```

- **The docks** (`src/workspace/dock.rs`, as Zed's): a `Dock` holds a list of `Panel`s and shows
  one at a time, open or folded, at its own size. The window has three — `left_dock`,
  `right_dock`, `bottom_dock` (under the center: the project's terminal) — drawn
  around the center by `workspace/layout.rs`, each panel by its own module
  (`BenchApp::render_panel`).
- **The left dock** shows one panel at a time: `Panel::Threads` (projects and their threads)
  or `Panel::Explorer` (one project's tree). The Explorer is opened on the project of the thread
  open now — the window's top-right button (pressed while its tree is shown), ⌘E, a project's
  menu, a thread's context menu — and takes the window: tree, documents, the thread beside on
  demand. The same button, still in that corner and pressed, closes it and brings the thread back;
  the strip over the sidebar keeps its "+" and the fold under either panel.
  A new
  way to look at things (search across threads, git, tasks) is a new `Panel` in the left dock's
  list, nothing else changes.
- **The center** is `Center` (`src/workspace/mod.rs`): what the sidebar's click opened. A thread's click shows `Chat`; a
  file's click shows `Doc` with a tab per open document (the tabs are the center's own bar,
  under the window's title bar). Leaving the Explorer does not close documents: their tabs come back.
- **The right dock** shows the thread beside a document (`Panel::Thread` in `right_dock`, ⌘J or the
  button in the window's top-right corner) — the same thread the center shows in the threads
  view, so the IDE needs no chat of its own.

Sizes come from `bench_ui` (`crates/ui/src/ide.rs`: the tree rows, a document's head, the tabs,
⌘P's list).

In Zed's words, which GPUI code follows (Bench is built on Zed's framework): the window is the
**workspace**; the sidebar is its left **dock**, whose **panels** are Threads and Explorer (as
Zed's project panel); the middle is the center **pane**, whose tabs are **items** (a document);
the right dock holds the thread. Shared pieces drawn the same everywhere are components in
`crates/ui`, as Zed's `ui` crate. Use these words when naming new code.

## The Explorer (`src/explorer/`)

One project at a time (the picker at the sidebar's top; it opens on the open thread's project).
Its tree:

| Group | What | Where it is |
|---|---|---|
| the folder | the project's code, listed as folders open | its machine (`/api/fs/*?machine=` when this Mac's agent works there over SSH) |
| Agent | `SYSTEM.md` (its soul), prompts, skills by where they live; then its tools: MCP servers, language servers | the agent's own files (`~/.bench/projects/<id>/`), a folder's `.agents/skills`; the environment's parts whose `group()` is "Agent" |
| Project | scheduled jobs, then its Settings | the agent's cron (`group()` "Project"); the settings document |

Every tab holds an `Item` (`explorer/item.rs`, as Zed's): its tab, unsaved changes, what
opening it reads, how it is drawn, saved and closed; `DocKey` is the tab's identity and
`DocKey::item` the one place that tells the kinds apart. Everything that is a file is opened as
one: an editor of its own (`explorer/file.rs`, `Doc`),
read and written through `/api/fs/read-text` and `write-text`; saving the agent's own text reads
the environment again (the composer's @ list follows). What is not a file — an MCP server with
its state and sign-in, a job with its schedule — is a **part**: an `ExplorerParts` section's `items()` in
the tree and its `card()` in the middle (`DocKey::Part`, `PartItem`). A new kind of thing in a
project is one of the two: files need nothing new; a part is a `Section` and `ExplorerParts` in
`PARTS` (its `group()` the tree's group).

The tree's work (`explorer/ops.rs`; ⌘P `file_finder.rs`, ⌘⇧F `project_search.rs`): folders are listed when they open (`/api/fs/list`), a name
is typed in place for a new file, folder, prompt, skill or a rename, a delete is asked once.
⌘P opens a file by name (`/api/fs/files`: ripgrep's list of the folder), ⌘⇧F finds a text in it
(`/api/fs/grep`), ⌘S saves, ⌘W closes the tab.

What it was left with — the open tabs and the one shown, each project's open folders and groups —
is kept on this Mac (`explorer.json` in the state folder, `explorer/persistence.rs`) and comes
back the first time the Explorer opens after a start; another project's tree comes back as it
was left too.

## Settings stay settings

A project's Settings (`src/env/page.rs`) are its fields: name, where it lives,
how it runs, its Telegram bot, its removal — `SettingsForm`s (in `FORMS`), each
drawing a `form()`; the page is the tab `DocKey::Settings` (`SettingsItem`). What a project *contains* is in the Explorer; what it *is set to* is in Settings.

## Where the IDE goes

The Explorer is one of Bench's three cores, beside threads and projects: the person reads and
checks the code here, the agent writes it in threads. It stays far smaller than Zed (which is
one click away) and grows by putting the same parts in more places, not by new copies:

- **Docks hold panels, the center holds items** — as in Zed. The right dock shows one panel at
  a time (`right_dock`): the open **thread** beside the code (the same `ThreadView` entity the
  center shows, so the IDE costs no second chat). Files are never kept beside: they open as tabs. In
  the threads view the right dock holds a second thread, its own `ThreadView` with its own
  composer, run and queue, opened with a thread's "Open beside" (`src/thread/beside.rs`); ⌘J
  folds it.
- **Code goes to the thread as references.** ⌘L sends the selection (or the whole file) to the
  composer as a chip naming the file and lines; a tab dragged onto the thread, or its menu's
  "Add to thread", sends the file. A message carries them under its words as fenced blocks with
  `path:lines` — what the agent reads, what the thread shows. The same chips will carry a
  language server's problems and a terminal's output.
- **Specs.** The tree's "Specs" group (after the folder, before Agent) shows the project's specs by
  `parent`, not by folder: the module's name, what it is quieter, a draft dimmed
  (`src/explorer/specs/`, from `GET /api/specs`). One opens as a tab read as a document: the
  Markdown without its frontmatter in a reading column, over it the specs it is under, over,
  refers to and is referred to by, each a link; "Edit" opens the file.
- **Review comments.** ⌘⇧L (Thread → Comment on Selection…, the file head's speech bubble) opens
  a band under the editor naming the selected lines; Enter puts the comment in the composer as a
  chip (`Reference::Comment`, `src/explorer/pane/comment.rs`), the caret back in the code for the
  next one. The message carries each as `Comment <id> on \`path:lines\`: <note>` with the lines
  fenced; the agent addresses each and closes it with `resolve_comment` (`agent/src/builtin/review.ts`),
  a step of its work in the thread.

Phases, each a thing of its own:

1. **Tabs and the thread beside** (done): tabs drag to reorder and have a menu (close others,
   to the left, to the right, saved, all; copy the path; add to the thread); the
   right dock's panels; references from the code to the thread.
2. **Language servers through the agent** (in the editor: done). The servers the threads use
   (`agent/src/lsp/lsp.ts`) answer the editor too: `POST /api/lsp/{diagnostics,hover,definition}`
   (`agent/src/modules/lsp.ts`) with the file's text as typed; `explorer/lsp.rs` gives the
   editor its providers — problems underlined (checked on open, save and a pause in typing),
   the hover, ⌘-click or F12 to the definition (another file opens as a tab). The project's
   **Problems** (⌘⇧M, the folder's menu) is a tab, as Zed's project diagnostics are
   (`DocKey::Problems`, `explorer/problems.rs`): every error and warning the servers know, a
   click opens the file at its line; "Send to the thread" (and a file tab's "Send its problems
   to the thread") puts them in the composer as a reference (`thread/attachments.rs`
   `Reference::Problems`), beside ⌘L's code.
3. **A terminal in the bottom dock** (done): `Panel::Terminal` (`src/inserts/terminal/panel.rs`),
   ⌃` or the title bar's button — the `/terminal` insert's shell (`servers.rs` `Tab`,
   `session.rs`) on the project's machine, started in its folder, one per project (the
   Explorer's while it is shown, else the thread's). "Send to the thread" puts its selection,
   or its last lines, in the composer (`Reference::Terminal`).
4. **Git at a glance** (done, `explorer/git.rs`): a file's lines changed since HEAD marked in
   its gutter as it is typed (added green, changed yellow, a red rule where lines were removed:
   `LineMark`, added to the vendored editor, `vendor/gpui-base`), HEAD's text from the agent
   (`/api/git/head`, on the file's machine); its diff against HEAD a read-only tab
   (`DocKey::Diff`: the file head's button, its tab's and the tree's "Changes since HEAD").

## Growing it

- Another source of files in the tree (a second folder, a worktree): a group in `tree.rs` whose
  rows are `FileRef`s — opening, saving, renaming already work for any of them.
- A diff view, an image viewer, a Markdown preview: another body in `view.rs` `file_pane` for
  the kind of file (`Doc::binary` already says "not text").
- A panel in a dock (a second thread, a Problems panel): a
  `Panel` variant, its dock's list in `BenchApp::new`, its drawing in `render_panel`. A split
  middle: documents are already shared by key.
- A thread opening a file (a changed file in its answer) in the editor, itself beside it: done in
  `thread/changed_files.rs` (`open_doc` + `right_dock.activate(Panel::Thread)`).

## What the person sees, briefly

- The tree's work is in context menus (a file: open, add to the thread, rename, delete, copy the
  path, Finder (on this Mac); a folder: new file, new folder and the same; the project's folder: also find
  in it and read it again; Prompts and Skills: a new one). Rows carry no buttons.
- The center's tabs are its own bar under the window's title bar (which drags the window).
- The project's settings are the tree's last row, a document like the rest; MCP and language
  servers each show their state and open as their own JSON entry.
- The app's settings (the gear) and a new window are in the macOS menu: Bench → Settings… ⌘,,
  New Window ⌘⇧N (`src/app/menu.rs`).
