# A personal Bench: every professional's own workbench

## What we are doing, and why

Big professional tools already exist and serve everyone a little: an IDE, an SSH client, a chat
with an AI, a converter site, Photoshop, Blender. Each person uses a small, specific part of each
of them — the part their own work needs — and pays for the rest with clutter, switching windows and
copying between apps.

**A Bench is the opposite: one window holding exactly the parts one person uses, built for that
person.** Nobody else can build it for them: nobody else knows their processes, and no company
makes software for one person. An AI agent with these skills can. The person describes how they
work; the agent turns that into an app that looks and feels professional, because the look, the
stack and the building blocks are already decided here.

The reference Bench is the user's own (`~/Projects/bench-kit`, the app is called **Bench**):
a chat with a local AI agent (Hermes), SSH servers with terminals, a mini IDE (a file tree and an
editor — not Zed, just "open, look, fix"), and a Tools mode of everyday utilities (Markdown from
copied web pages, passwords, Base64, regex, cron, Unix time…). The user's whole working day now
happens in that one window. That is the result to aim for, for anyone.

What a Bench is **not**: a replacement for a big product, or a tool for everybody. It holds the
person's *subset* — "why do I open Photoshop? to crop and export screenshots" — implemented
simply, without anything they don't use, often with the agent automating the steps between.

## The look is the method

Every Bench looks like the reference one, whatever the person's job: **the same frame and the
same small, square, precisely spaced pieces**, after the Zed editor — a sidebar, a header bar on
top, a status bar at the bottom, flat panes split by 1px lines, small exact icons, dense rows,
one accent, nothing rounded or decorative. Like an IDE built for one person's work.

This is not a theme on top of the code; it decides the code. A job that has to fit into "a list
on the left, a strip of small controls, two panes, a status line" gets broken into the right
pieces: what the sidebar lists, which few actions earn a button, what input goes in and what
comes out. Designing the screen in these terms first is how the logic of the job becomes clear —
and why a small model can build it: it doesn't invent a UI, it places the job into a known one.

So:
- **Never invent a new look**, a new kind of control or a different layout for a new job. Take
  the frame from `scripts/new-bench.sh` (or `new-app.sh` for an app without the agent), the pieces from `references/ui-catalog.md` and `references/ui-content.md`, the rules
  from `references/design-look-rules.md`. If the job seems not to fit, find which existing piece it is
  (a mode, a tool, a pane, a card under a chat answer, a row action) — ask before making a new one.
- Two people's Benches do different things and still look like one family. That shared look is
  the collection's value: anyone who has used one Bench can use another.

## The agent is in by default

Almost every Bench is unthinkable without an AI agent: the work turns around it, and the agent
automates the steps between the person's tools. Hermes is the agent — local, profiles, skills,
cron, MCP — and the reference Bench already has a complete, tuned client for it (sessions,
steps, media, background processes, files it changed, voice, providers, install).
So a Bench starts with it (`scripts/new-bench.sh`). But build what was asked: if the person
wants "just a text editor", they get one without a chat (`scripts/new-app.sh`); when they later
want AI in it, the chat comes as a panel beside the open document ("edit this with the agent"),
from the same client code (`build-hermes-client.md`).

## A Bench aggregates; the sources stay native

A Bench shows and edits data that lives somewhere else, in the place the person already trusts
and could open without the Bench: the agent's sessions stay in Hermes, servers in `~/.ssh/config`,
code in the editor's folders, tasks in Apple Reminders. The Bench keeps no database of its own
(only its look and last place). If the person opened the native app instead, everything would be
there. What the native store has no field for is kept as plain lines it does have (comments in
the SSH config, `Tags:` / `Spent:` lines in a reminder's notes) — readable anywhere, parsed by
the Bench. The agents write to the same source directly (their own MCP, configured in the agent,
never inside the Bench), and the Bench shows it.

The user's tasks are the worked example: years in Things (a third-party app; the agents reached
it by reverse-engineering its database, time kept in titles as "- 2 часа") moved into Apple
Reminders (1318 tasks, back-dated completions, time moved out of the titles into `Spent:`); the
Bench got a Things-like Tasks mode on top of EventKit; the Hermes profiles got a small Node MCP
(`~/Projects/reminders-mcp`, EventKit via a background helper app) and rewritten skills. One window
for control, native storage, agents working through the same storage.

When choosing where a new kind of data lives: prefer what the OS or the person's long-lived
tools already keep (Reminders, Calendar, files, config files) over anything new; pick a
proprietary third-party app or an own database only when nothing native can hold it.

## The person grows their Bench themselves

Nobody can design anyone's whole system in advance, and no developer should stand between the
person and their tools. The person describes their work to the agent with these skills and gets
their Bench; later, when they need something new ("a tool that turns this report into a table",
"a mode for my clients"), they ask the agent again and it is added the same way, in the same look.
Write the code so that this stays easy: one mode per module, tools as pure functions with tests,
the views thin — the next addition copies the shape of the last one.

## How to work with the person

1. **Listen first, then interview** (`references/start-interview.md`): their job, a normal day, which
   programs they open and *what exactly they do there*, what they copy between apps, what they
   repeat, what annoys them. Write it back as a short list in their words and confirm it.
2. **Propose the shape** if they have no picture of it: which **modes** (the window's top-level
   places, like Sessions / SSH / Tools in Bench), what each mode's sidebar lists and what its main
   area shows, which actions the AI agent could take over. Small and concrete beats complete: the
   first version covers the core of each job, the rest comes later. They choose; you build.
3. **Build in order**: a copy of the reference Bench from `scripts/new-bench.sh` (same look, same
   icon; `new-app.sh` only for an app they asked to have without the agent) → switch off the
   modes they don't use (`bench-modes-remove.md`) →
   one mode at a time (`references/build-modes.md`) → each mode's parts from ready
   recipes and crates. After every mode: build, run, screenshot, bundle, show the person.
4. **Grow it when they ask.** New needs become new modes or tools, built the same way and in the
   same look; links between parts ("open this file in the editor", "send this to the agent",
   "this tool's output into that tool") come as the person discovers them. Never remove what
   works for them.

## How to build (the engineering rules)

Write code like a lazy senior developer (after the open-source "ponytail" skill,
github.com/DietrichGebert/ponytail): understand the task and the code it touches first, then take
the first rung that fully works:

1. **Does it need to exist?** Build what the person asked for; name extra ideas in one line.
2. **Is it already in the app?** Reuse Bench's code and the template's `kit.rs` the way nearby
   code does.
3. **Does a ready component cover it?** gpui-kit's editor, Markdown view, inputs, menus
   (`references/ui-content.md`).
4. **Does a proven crate solve it?** For real problems — HTML to Markdown, time zones, cron,
   regex, hashing, a terminal — use the crate from `references/build-crates.md`
   (already chosen, already compiling together) instead of writing your own. Not for a few lines.
5. **Otherwise** write the minimum that works, in the house style.

Always: logic with branches, parsers, time or money gets a small test; errors are shown to the
person, never swallowed; nothing that can lose their data runs without a confirmation; the report
says what was checked and what was not. Keep logic in plain Rust modules (pure functions, tested)
and the views thin — Bench's `src/devtools/` is the example.

Where everything is: the map in `SKILL.md`.
