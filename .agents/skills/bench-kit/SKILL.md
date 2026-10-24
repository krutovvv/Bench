---
name: bench-kit
description: "Bench Kit: build a person their own Bench — one native, fast desktop app in Rust + GPUI (gpui-kit 0.8) that holds exactly the parts of their daily work they use — the Hermes AI agent built in by default, plus servers and terminals, files and an editor, everyday tools, their own records kept in native stores (Reminders, ssh config, folders) — all in one shared Zed-like look: small, square, precisely spaced, a sidebar, a header bar, a status bar. Everything is here: why and how to interview the person, a new app in one command, a UI catalog of every piece (buttons, rows, tabs, forms, panes, editor, Markdown, menus) with pictures and code, how to add a mode or a tool, a crate catalog, the look's rules, verification, packaging, and the gpui-kit/GPUI reference. Use for ANY desktop app, tool window or UI change in Rust/GPUI, for adding a mode or a tool to an existing Bench (including the user's own, ~/Projects/bench-kit), and when asked how such an app should look or be built."
version: 2.1.0
author: Hermes Agent and the user (from building Bench, ~/Projects/bench-kit)
license: MIT
platforms: [macos]
metadata:
  hermes:
    tags: [Rust, GPUI, gpui-kit, Desktop, Bench, Workbench, UI-Kit, Design-System, Zed, macOS]
    category: desktop-dev
---

# Bench Kit

## What this is, in one paragraph

A person installs this kit, tells the agent how they work — what they do all day, in which
programs, what exactly they do there, what they repeat — and gets **their own Bench**: one
native, fast window that holds exactly the parts of their work they use, with an AI agent
(Hermes) built in, in one shared look. Big tools (an IDE, an SSH client, an AI chat, a task
app, converter sites, Photoshop) serve everyone a little; each person uses a small part of each
and pays for the rest with clutter and window switching. Nobody else knows their processes and
nobody builds software for one person — the agent with this kit can. Later the person asks for
more and the Bench grows the same way. Like a 3D printer printing the same boat in different
sizes and colors: different jobs, one shape. The reference Bench is the user's own,
`~/Projects/bench-kit` (the app **Bench**: the Hermes chat, SSH servers with terminals, a mini
IDE, Tools, Tasks over Apple Reminders); every new Bench starts as a copy of it.

## Working on Bench from inside Bench (read first)

This kit lives in Bench's own repository, `~/Projects/bench-kit/.agents/skills/bench-kit` — Hermes
loads it for sessions started in that folder. Most often the person talks to you **through Bench
itself** while you change Bench. So:
- **Never quit, kill, restart or replace a running Bench** (`pkill`, `osascript … quit`, deleting
  its `.app`): it is the window the person works in and talks to you through.
- **A build is a commit's app.** Commit, then `scripts/ship.sh` (the kit's): it builds
  `dist/Bench.<commit>.app` — the commit's 7-character hash — **next to** the running one and
  opens it. The person tries it and moves the Dock icon over themselves; the newest 6 builds
  stay for rolling back. Uncommitted changes stop the build.
- Screenshots and tests only of **your own copy** (`scripts/shot.sh`), never the person's window.
- The code every page points to is this repository's `src/`.

## The five things that never change

1. **One look, and the look is the method.** Every Bench looks like the reference one: Zed-like,
   flat, square, small precise pieces, a sidebar, a header bar, a status bar, one accent, every
   bar 36, one inset per area, every rule that crosses the window on one pixel row. The design
   decides the code: a job placed into "a list on the left, a strip of small controls, panes, a
   status line" gets split into the right pieces. So **never invent a look or a control** —
   place the job into the catalog's pieces (`references/ui-catalog.md`). This is also why a
   modest model can build a Bench: it doesn't design, it assembles.
2. **The agent is in by default.** A Bench is built around the AI agent (the Hermes chat) — it
   starts from `scripts/new-bench.sh`. Only when the person explicitly asks for an app without
   one ("just a text editor") use `scripts/new-app.sh`; the agent can be added to it later (a
   chat panel for the open document).
3. **The Bench aggregates; the data stays native.** It shows and edits what lives where the
   person already keeps it — sessions in Hermes, servers in `~/.ssh/config`, tasks in Apple
   Reminders, files in folders — and keeps no database of its own. The agents reach the same
   data through their own tools (an MCP server in the agent, never inside the Bench), so they
   work with the Bench closed (`references/build-native-data.md`).
4. **Their subset, built exactly as asked.** Not a replacement for a big product: the part they
   use, done simply. "A random 16-character password" is not "an Argon2 hash". Extra ideas: one
   line, not built. Nothing that does nothing ("coming soon").
5. **The person grows it.** One mode per module, tools as tested pure functions, thin views —
   the next addition copies the shape of the last one.

## Start here (in order)

1. **Understand the person** (`references/start-interview.md`): their job, a normal day, which
   programs and *what exactly they do there*, what they copy between apps, what they repeat.
   Write it back as a short list in their words; confirm it. No picture of their own? Propose
   one: which modes, what each sidebar lists, what the agent takes over.
2. **Create the project in one command** (`references/build-new-app.md`):
   `scripts/new-bench.sh <crate>` — the usual start, a copy of the reference Bench;
   `scripts/new-app.sh <crate> "<Title>"` — only for an app they asked to have without an agent.
   Build and run it untouched before changing anything (`cargo build && ./target/debug/<crate>`).
3. **Switch off what they don't use** (`references/bench-modes-remove.md`: one flag per mode in
   `src/bench.rs`), then **add theirs**: each job with its own list becomes a **mode**
   (`references/build-modes.md`); one-off jobs become **tools** in the Tools mode
   (`templates/examples/devtools/`, 19 ready ones).
4. **Build every view from the catalog** (`references/ui-catalog.md` → `ui-*.md`). Never style
   buttons, rows or bars yourself; never write your own text editor, renderer or highlighter.
5. **A library? From `references/build-crates.md`** (proven together). Nothing there → ask
   before adding a crate. Never a pre-release crate.
6. **Small steps**: one change (≤ ~50 lines) → `cargo build` → fix → run → next. Never several
   new files before the first build. Past 40 tool calls without a running app: stop, go back to
   the last version that ran, and say what is hard.
7. **Every change goes through the loop** (`references/build-change-loop.md`): the target in
   numbers → find the code and the reference → shot and numbers before → one exact edit →
   build with zero warnings → shot and numbers after → hooks out → commit → `scripts/ship.sh`
   (a new app for that commit, beside the running one — never quit the person's) → report
   with before/after numbers. Screenshots only of your own copy (`scripts/shot.sh`), never the
   person's window; measure with `scripts/rules.py` and `scripts/textx.py`. "Looks fine" is
   not a check.
8. **Where a thing goes, how to move it, how to fit it to another piece**:
   `references/ui-areas.md` — the window's areas with their numbers and a procedure for each
   kind of request.

Rules that keep it right:
- **One UI language: English**, short labels, tooltips on icons with their shortcut.
- **Engineering ladder**: need it at all? → already in
  the app? → a ready component? → a proven crate? → otherwise the minimum, in the house style.
  Logic with branches, parsers, time or money gets a small test; errors are shown, never
  swallowed; nothing that can lose data runs without a confirmation.
- **When their data moves** (a new store, a new tool), everything that used the old place moves
  too: the agent's skills, memory, and scheduled jobs (`references/build-native-data.md`).
- **Only working results go into this kit.** If the person wasn't happy with something, it is
  not a lesson to write here. When Bench changes, update the pages that describe what changed
  (the kit lives in Bench's own repository, so its code examples are Bench itself).
- As the Hermes agent: how to use your tools (builds, `patch`, LSP, `rust_lsp`) —
  `references/build-hermes-tools.md`. Read it once per session.

## Map of the kit

**Start** — why, and with whom
| Page | What |
|---|---|
| `references/start-why.md` | What a Bench is and isn't, the look as the method, the person grows it, how to work with them, the engineering rules |
| `references/start-interview.md` | Interview questions, writing it back, turning answers into modes and tools, the user's Bench as the example |

**UI catalog** — every piece, with a picture and the call (pictures in `assets/ui/`)
| Page | Pieces |
|---|---|
| `references/ui-catalog.md` | **The index**: a table of all pieces, how handlers are passed, the rules every piece follows |
| `references/ui-areas.md` | **The window's areas** with their sizes and insets; what goes where; procedures to put, move and fit a piece |
| `references/ui-frame.md` | Window, sidebar (top strip with mode toggles, search band, list, bottom button), header bar, status bar, `bar_h`, modes |
| `references/ui-buttons.md` | Primary, icon, status, small text button, arm-then-delete, gpui-kit `Button` |
| `references/ui-lists.md` | Group header, list row (meta, nested, hover actions), compact rows, tabs, row menus |
| `references/ui-forms.md` | Settings row, segmented, checkbox, text field, form, textarea / composer, picker |
| `references/ui-content.md` | Empty state, two panes with strips, editor (5 recipes), Markdown, HTML, plain text, images, diff, terminal |
| `references/ui-overlays.md` | Menus, right-click, popover, tooltip, toast, attached lists, loaders, cards |
| `references/ui-gotchas.md` | Compile errors that happened, layout, scrolling, keyboard, theme, platform, mistakes |

**Build** — how a Bench is put together
| Page | What |
|---|---|
| `references/build-new-app.md` | The two starts (`new-bench.sh` with the agent, `new-app.sh` without), the shared build folder |
| `references/bench-modes-remove.md` | Switching off the reference Bench's modes a person doesn't use (`src/bench.rs`) |
| `references/build-architecture.md` | Crate layout, modules, state, threads, windows; where Bench keeps each piece |
| `references/build-modes.md` | Add a mode: state, toggle over the sidebar, sidebar/header/body, shortcut, panes, background work, clipboard, files, polling, lists re-read from their source |
| `references/build-native-data.md` | Data in native stores (Reminders via EventKit, notes lines, `~/.ssh/config`), the agents' MCP, moving data and the skills/cron jobs that used it, groups of chats |
| `references/build-crates.md` | Which crate for which job (HTML→Markdown, dates, cron, regex, hashes, terminal…), macOS without crates |
| `references/build-media.md` | Images, video, audio, Markdown files, Mermaid, LaTeX, attachments, drag and drop |
| `references/build-custom-render.md` | Custom drawing (a terminal grid, a chart) — only when asked |
| `references/build-hermes-client.md` | A chat with the Hermes agent: gateway, tickets, profiles, Stop, media |
| `references/build-change-loop.md` | **One change, start to finish**: target → code → measure → edit → build → measure → clean → ship → report, with the commands |
| `references/build-workflow.md` | The order of work after the app runs; a change to an existing Bench: look at the reference first |
| `references/build-verification.md` | Screenshots of your own window, measuring paddings |
| `references/build-packaging.md` | `.app`, icon, Info.plist, signing, microphone/LAN permissions, logs |
| `references/build-hard-won-rules.md` | Rules each paid for with a round of feedback |
| `references/build-hermes-tools.md` | Hermes agent: builds, edits, LSP, `rust_lsp`, tool-call formats |

**Design** — the look in detail
| Page | What |
|---|---|
| `references/design-look-rules.md` | The 13 rules behind every view |
| `references/design-system.md` | Colors, fonts, sizes, accents, dark mode (code: `templates/app/src/theme.rs`) |
| `references/design-layout.md` | Exact layouts: sidebar groups, header, status bar, composer, settings tabs, chat column, side panel |
| `references/design-icons.md` | Lucide icons, adding one (code: `templates/app/src/icons.rs`) |
| `references/design-user-preferences.md` | What the user liked and rejected, how to talk about UI changes |

**gpui-kit and GPUI** — the framework's own reference (vendor docs; long — only for what the catalog lacks)
| Page | What |
|---|---|
| `references/gpui-kit.md` | Where the sources and docs are, how to look up a component |
| `references/gpui-kit-usage.md` | All components: Button, Input, Select, Checkbox, Dialog, Notification, Tabs, Tooltip, Form, List, theming |
| `references/gpui-kit-recipes.md`, `gpui-kit-conventions.md` | Short recipes, naming conventions |
| `references/gpui-kit-coding-guides.md`, `gpui-kit-design-guides.md` | The vendor's normative coding and design guides (our look rules win where they differ) |
| `references/gpui-*.md` | GPUI mechanics: action, async, context, element, entity, event, focus-handle, global, layout-style, test |

**Files**
| Path | What |
|---|---|
| `scripts/new-bench.sh` | A new Bench: a copy of the reference one (the usual start) |
| `scripts/new-app.sh` | A new app without the agent, from `templates/app/` |
| `~/Projects/bench-kit` (this repository) | The reference Bench itself: `src/` is the code every page points to; `new-bench.sh` copies it |
| `scripts/shot.sh` | Screenshot YOUR copy (a binary, or the .app with its permissions and `KEY=VALUE` hooks), never the person's |
| `scripts/rules.py`, `scripts/spans.py`, `scripts/textx.py`, `scripts/crop.py`, `scripts/measure.py` | Rules and edges down a column; icons and gaps along a band; where text starts; one area cut out and zoomed (all in pt); raw pixel runs (px) |
| `scripts/ship.sh` | Build this commit's `dist/Bench.<commit>.app` and open it next to the running one (never quits anything) |
| `templates/app/` | The template app (frame, `kit.rs`, theme, icons, settings, bundle script) |
| `templates/examples/gallery.rs` | A screen using every kit piece (`assets/gallery.png`) |
| `templates/examples/devtools/` | 19 everyday tools as tested pure functions (copy into `src/devtools/`) |
| `templates/examples/tools-mode/` | Bench's Tools mode: state and views, to copy |
| `templates/examples/text-viewer/` | "Just a text editor" from `new-app.sh`, finished and screenshotted |
| `templates/examples/reminders-mcp/` | The agents' MCP over Apple Reminders (Node + a Swift helper .app) |
| `assets/ui/*.png` | Pictures of each piece for the catalog |
