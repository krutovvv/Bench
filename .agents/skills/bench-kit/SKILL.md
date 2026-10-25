---
name: bench-kit
description: "Bench Kit: how to understand and change Bench — an open, native workbench for working with an AI agent (Rust + GPUI, gpui-kit 0.8), a thread-centric editor in the spirit of Zed's Delta, on its own agent in agent/ (Pi Durable). A thread is a document of messages and the person's inserts: live tools put in with / (/terminal, /ssh, /files, /ftp, /tasks, /tools, /browser), each kept by the agent as a Pi entry and document, with an agent tool that acts on it and its state sent with every message. The kit: the concepts, the insert recipe with the six kinds as worked examples, a UI catalog of every piece with pictures and code, the look's rules and the user's preferences, the agent's side in Pi's terms, crates, the change loop with pixel checks, shipping, and the gpui-kit/GPUI reference. Use for any change to Bench (~/Projects/bench-kit), for adding an insert kind, and when asked how Bench should look or be built."
version: 4.0.0
author: The user and their agents (from building Bench, ~/Projects/bench-kit)
license: MIT
platforms: [macos]
metadata:
  tags: [Rust, GPUI, gpui-kit, Desktop, Bench, Pi, Agent-Client, Delta, Zed, macOS]
  category: desktop-dev
---

# Bench Kit

## What this is, in one paragraph

Bench is a native desktop **workbench for working with an AI agent**: an open, thread-centric
editor in the spirit of Zed's Delta, written from scratch in Rust and GPUI, on its own agent
(`agent/`, Pi Durable). **The thread with the agent is the unit of work.** A thread is a
document: messages and the person's **inserts**, small live tools put in with `/` where the
conversation is (`/terminal`, `/ssh`, `/files`, `/ftp`, `/tasks`, `/tools`, `/browser`; several per
thread, every `/` a new one, never switching or renaming the thread). The agent knows what is
inserted, sees what each shows with every message, and has a tool to act on it. Forms and
editors come as cards in the thread, under what asked for them. The sidebar holds only
**projects** (each with its environment: soul, MCP, skills, prompts, accesses, Telegram,
schedule; a folder and a server optional) and their threads. The repository's `AGENTS.md` is the
short version of all this; this kit is the detail: how to change Bench and how to add the next
insert kind, in its one look.

## Working on Bench from inside Bench (read first)

This kit lives in Bench's own repository, `~/Projects/bench-kit/.agents/skills/bench-kit`. Most
often the person talks to you **through Bench itself** while you change Bench. So:
- **Never quit, kill, restart or replace a running Bench** (`pkill`, `osascript … quit`, deleting
  its `.app`): it is the window the person works in and talks to you through.
- **A build is a commit's app.** Commit, then `scripts/ship.sh`: it builds
  `dist/Bench.<commit>.app` **next to** the running one and opens it. The person tries it and
  moves the Dock icon over themselves; the newest 6 builds stay for rolling back. Uncommitted
  changes stop the build.
- Screenshots and tests only of **your own copy** (`scripts/shot.sh`), never the person's window.
  `BENCH_DEMO=1|top|tasks|server|tool|env` opens a made-up session (`src/demo.rs`) to shoot;
  `BENCH_HOME=<a copy> BENCH_SUBJECT=tasks:upcoming` runs a real agent on a copy of the data and
  opens a new thread with that insert.
- The code every page points to is this repository's `src/` (Bench) and `agent/` (the agent).

## The five things that never change

1. **Projects, threads, inserts.** No modes, no profiles, no pages or panels for a job; the only
   classic settings are the gear (providers, look). The sidebar lists projects; a project's
   environment (soul, MCP, skills, prompts, accesses, Telegram, schedule) is what its threads get,
   edited in its environment block. A tool is an **insert** put into a thread with `/`, kept by the
   agent in Pi's terms (an entry where it was put, a document of its state), reliable and restored
   on return (`references/build-insert.md`). Something the agent produces during a turn shows in
   that turn's answer (a card, a chart).
2. **One look, and the look is the method.** Zed-like, flat, slightly round (the theme's radius;
   `rounded_md` for every bordered block), precisely spaced, one accent; every block's head is
   `block_head` (one height, one faint ground); copying is an icon; in the thread one font size,
   the messages'. A change to how blocks look goes to all of them at once. **Never invent a look or a control** — place
   the job into the catalog's pieces (`references/ui-catalog.md`). This is why a modest model can
   do it: it doesn't design, it assembles.
3. **The agent is the core, on Pi as its authors intended.** Bench is a client of `agent/` (Pi
   Durable, `references/build-pi-backend.md`). Extend with Pi's packages and popular ones; write
   as little of our own as possible. A tool's work the agent needs lives in the agent (a tool, an
   MCP server), so it works with Bench closed, in Telegram, on a schedule.
4. **One home for Bench's own data: `~/.bench`.** `bench.sqlite` (tasks, hosts, settings,
   providers, sessions), agents in Pi's agent directories, Bench's view state in `ui/`. What
   already has a home stays there (`~/.ssh/config`, files, the browser's profile)
   (`references/build-native-data.md`).
5. **Built exactly as asked.** The part the person uses, done simply; extra ideas in one line,
   not built; nothing that does nothing. One module per tool on each
   side, logic as tested pure functions, thin views — the next tool copies the last one.

## Start here (in order)

1. **Read `AGENTS.md`** at the repository's root: the concepts, the code map, how the agent uses
   Pi, the working rules.
2. **Find the nearest existing piece in `src/`** and the page that describes it. Bench is its own
   example: a new thing copies the closest one.
3. **Add a tool = a new kind of insert** (`references/build-insert.md`): key and state → its
   `InsertKind` (one file in `src/ui/inserts/`) → its `/` word → the agent's tool, prompt line and
   live state → a demo thread. Copy the nearest of the six.
4. **Build every view from the catalog** (`references/ui-catalog.md` → `ui-*.md`). Never style
   buttons, rows or bars yourself; never write your own text editor, renderer or highlighter.
5. **A library? From `references/build-crates.md`** (proven together). Nothing there → ask before
   adding a crate. Never a pre-release crate. On the agent's side: Pi's packages, then popular
   ones, versions at least two weeks old.
6. **Small steps**: one change (≤ ~50 lines) → `cargo build` → fix → run → next. Past 40 tool
   calls without a running app: stop, go back to the last version that ran, say what is hard.
7. **Every change goes through the loop** (`references/build-change-loop.md`): the target in
   numbers → the code and the reference → shot and numbers before → one exact edit → build with
   zero warnings → shot and numbers after (`scripts/rules.py`, `textx.py`) → commit → `scripts/ship.sh` → report with before/after.
   "Looks fine" is not a check.
8. **Where a thing goes, how to move it**: `references/ui-areas.md`.

Rules that keep it right:
- **One UI language: English**, short labels, tooltips on icons with their shortcut.
- **Engineering ladder**: need it at all? → already in Bench? → a ready component? → a proven
  crate or package? → otherwise the minimum, in the house style. Branches, parsers, time or money
  get a small test; errors are shown, never swallowed; nothing that can lose data runs without a
  confirmation.
- **When data moves** (a new store, a new tool), everything that used the old place moves too:
  the agent's skills, memory, scheduled jobs (`references/build-native-data.md`).
- **The web demo keeps compiling** (`cargo check --target wasm32-unknown-unknown`; desktop-only
  crates under the non-wasm target, a stub for the browser build).
- **Only working results go into this kit.** When Bench changes, update the pages that describe
  what changed: the kit's examples are Bench itself.

## Map of the kit

**Build** — how Bench is put together and grown
| Page | What |
|---|---|
| `references/build-insert.md` | **Adding a tool: a new kind of insert** — how inserts are kept in Pi, the `InsertKind` trait, the six existing kinds as examples, body patterns, the checklist of files |
| `references/build-pi-backend.md` | The agent: Pi Durable server in `agent/`, installed and started by Bench (also over SSH); extensions, tools, prompt sections, how each tool uses Pi Durable, the server browser |
| `references/build-agent-client.md` | Bench's side of the protocol: sessions, events, Stop, questions, media |
| `references/build-architecture.md` | Crate layout, modules, state, threads, windows; where Bench keeps each piece |
| `references/build-native-data.md` | Where a tool's data lives (`~/.bench`, native places), moving data and the skills/cron jobs that used it |
| `references/build-crates.md` | Which crate for which job (HTML→Markdown, dates, cron, regex, hashes, terminal…) |
| `references/build-media.md` | Images, video, audio, Markdown files, Mermaid, LaTeX, attachments, drag and drop |
| `references/build-custom-render.md` | Custom drawing (a terminal grid, a chart) — only when asked |
| `references/build-change-loop.md` | **One change, start to finish**: target → code → measure → edit → build → measure → ship → report |
| `references/build-workflow.md` | The order of work; a change: look at the reference first |
| `references/build-verification.md` | Screenshots of your own window, measuring paddings |
| `references/build-packaging.md` | `.app`, icon, Info.plist, signing, permissions, logs |
| `references/build-hard-won-rules.md` | Rules each paid for with a round of feedback |

**UI catalog** — every piece, with a picture and the call (pictures in `assets/ui/`)
| Page | Pieces |
|---|---|
| `references/ui-catalog.md` | **The index**: a table of all pieces, how handlers are passed, the rules every piece follows |
| `references/ui-areas.md` | The window's areas with their sizes and insets; what goes where; procedures to put, move and fit a piece |
| `references/ui-frame.md` | Window, sidebar, header bar, status bar, `bar_h` |
| `references/ui-buttons.md` | Primary, icon, status, small text button, arm-then-delete |
| `references/ui-lists.md` | Group header, list row (meta, nested, hover actions), compact rows, tabs, row menus |
| `references/ui-forms.md` | Settings row, segmented, checkbox, text field, form, textarea / composer, picker |
| `references/ui-content.md` | Empty state, two panes with strips, editor (5 recipes), Markdown, HTML, plain text, images, diff, terminal |
| `references/ui-overlays.md` | Menus, right-click, popover, tooltip, toast, attached lists, loaders, cards |
| `references/ui-gotchas.md` | Compile errors that happened, layout, scrolling, keyboard, theme, platform |

**Design** — the look in detail
| Page | What |
|---|---|
| `references/design-look-rules.md` | The rules behind every view |
| `references/design-system.md` | Colors, fonts, sizes, accents, dark mode (code: `src/theme.rs`) |
| `references/design-layout.md` | Exact layouts: sidebar groups, header, status bar, composer, the thread's column, blocks |
| `references/design-icons.md` | Lucide icons, adding one (code: `src/icons.rs`) |
| `references/design-user-preferences.md` | What the user liked and rejected, how to talk about UI changes |

**gpui-kit and GPUI** — the framework's own reference (vendor docs; long — only for what the catalog lacks)
| Page | What |
|---|---|
| `references/gpui-kit.md` | Where the sources and docs are, how to look up a component |
| `references/gpui-kit-usage.md` | All components: Button, Input, Select, Checkbox, Dialog, Notification, Tabs, Tooltip, Form, List, theming |
| `references/gpui-kit-recipes.md`, `gpui-kit-conventions.md` | Short recipes, naming conventions |
| `references/gpui-kit-coding-guides.md`, `gpui-kit-design-guides.md` | The vendor's normative guides (our look rules win where they differ) |
| `references/gpui-*.md` | GPUI mechanics: action, async, context, element, entity, event, focus-handle, global, layout-style, test |

**Files**
| Path | What |
|---|---|
| `~/Projects/bench-kit` (this repository) | Bench itself: `src/` the app, `agent/` the agent, `src/demo.rs` the made-up session |
| `scripts/ship.sh` | Build this commit's `dist/Bench.<commit>.app` and open it next to the running one (never quits anything) |
| `scripts/shot.sh` | Screenshot YOUR copy (a binary, or the .app, with `KEY=VALUE` hooks), never the person's |
| `scripts/rules.py`, `spans.py`, `textx.py`, `crop.py`, `measure.py` | Rules and edges down a column; icons and gaps along a band; where text starts in a band; an area cut out and zoomed (pt); raw pixel runs (px) |
| `templates/app/src/ui/kit.rs` | The piece library the catalog documents (`kit::…`); `templates/examples/gallery.rs` uses every piece (`assets/gallery.png`) |
| `templates/examples/devtools/` | The 19 tools of Tools as tested pure functions (Bench's `src/devtools/`) |
| `assets/ui/*.png` | Pictures of each piece for the catalog |
