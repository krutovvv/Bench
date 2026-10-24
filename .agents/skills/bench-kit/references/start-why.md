# What Bench is, and what this kit is for

## Bench: a client for Pi, shaped like Delta, grown around one person's work

Bench is a native desktop client for an AI agent built on [Pi Durable](https://github.com/earendil-works/pi),
written from scratch in Rust with GPUI. Its shape is taken from Zed's Delta: **the thread with
the agent is the unit of work**. One chat app, no modes, no separate windows for separate jobs.

What it adds to that is the person's own tools, **each as a kind of thread**. The sidebar lists
what the person works on — task lists, servers, file hosts, browser sessions, small utilities —
and each opens a thread about it: the thing itself live at the top (the task list, the server's
terminal, the folder on the FTP host, the page in the browser, the tool's input and output), the
conversation with the agent under it. The agent knows what the thread is about, sees what the
block shows with every message, and has a tool to act on it. The person can do the work by hand
in the block, ask the agent, or both, in the same place.

The user's Bench (this repository) has five such tools: **Tasks** (their task manager), **Servers**
(SSH terminals), **Files** (FTP/SFTP hosts), **Browser** (a shared server browser the agent drives,
the person watches and steps into) and **Tools** (everyday utilities: Base64, regex, cron, JSON…).
Plus ordinary project threads, cron jobs and Telegram — all threads too.

It did not start this way. The first plan was a collection of skills that would build anyone a
different app from a description of their work; along the way the modes, panels and separate
apps fell away one by one, and what stayed is this: a Delta-like agent client where a tool is a
thread. That is the result to keep, not to rebuild differently each time.

## What the kit is for

**Growing this Bench.** A person's needs differ from the user's: one has clients, another the
shop's orders, a third a build queue. Each of those becomes one more kind of thread, made the
same way as the five that exist (`build-subject.md`): a subject key, a live block, a sidebar
group, an agent tool, a short prompt section, the block's state sent with each message. The kit
holds the look, the pieces, the recipes and the mistakes already paid for, so an agent — even a
modest model — adds a tool that looks and works like the others.

What the kit is **not** for: designing a different app, a new layout, a mode, a window of its own
for one job. If a job seems not to fit a thread, find which thread it is (a block, a step in the
answer, a card under it, a tool in Tools) and ask before inventing anything.

## The look is the method

Every part of Bench has the same look: Zed-like, flat, square, precisely spaced, one accent; a
sidebar, a header bar, a status bar; in the thread one font size, the messages' (secondary things
muted, never smaller). A new tool that has to fit into "a block at the top of a thread, a group in
the sidebar, an answer under it" gets split into the right pieces by itself: what the sidebar
lists, what the block shows, which few actions earn a button, what the agent does. So:
- **Never invent a look or a control.** Pieces from `ui-catalog.md`, rules from
  `design-look-rules.md`, the person's preferences in `design-user-preferences.md`.
- The block is built from the same pieces as the rest; the agent's answer under it is the same
  Markdown as everywhere.

## The agent is the core

Bench is a client: the agent (`agent/`, Pi Durable, `build-pi-backend.md`) does the work, keeps
the sessions, runs on this Mac or on a server, answers in Telegram and on schedules with Bench
closed. Build on Pi the way its authors describe it, extend it with their packages or popular
community ones, write as little of our own as possible. A tool's logic that the agent needs lives
in the agent (a tool or an MCP server), not in Bench; Bench shows it and lets the person act.

## Where the data lives

Bench's own records live in one home, `~/.bench` (`bench.sqlite`: tasks, hosts, settings,
providers, sessions), reached through the agent, so the agent works with them with Bench closed.
What already has a home stays there: servers in `~/.ssh/config`, files in their folders and on
their hosts, sign-ins in the browser's profile. `build-native-data.md`.

## The person grows it

Nobody designs anyone's whole system in advance. When the person needs something new, they ask
the agent with this kit, and it is added the same way, in the same look. Keep the code so that
stays easy: one module per tool on each side (`src/app/<kind>.rs`, `src/ui/<kind>.rs`,
`agent/src/builtin/<kind>.ts`), logic as tested pure functions, views thin — the next tool copies
the shape of the last one.

## How to work with the person

1. **Listen, then ask** (`start-interview.md`): what they work on all day, in which programs,
   what exactly they do there, what they'd like the agent to take over.
2. **Propose threads**: for each job, which kind of thread it is — one of the five, or a new one
   (what its sidebar lists, what its block shows, what the agent's tool does). Small and concrete:
   the core of each job first.
3. **Build one tool at a time** (`build-subject.md`), each through the change loop
   (`build-change-loop.md`): build, run, screenshot, commit, a new app next to the running one.
4. **Grow it when they ask.** Never remove what works for them.

## How to build (the engineering rules)

Like a lazy senior developer: understand the task and the code it touches, then take the first
rung that fully works:
1. **Does it need to exist?** Build what was asked; name extra ideas in one line.
2. **Is it already in Bench?** Reuse the nearest subject's code the way it is written.
3. **Does a ready component cover it?** gpui-kit's editor, Markdown view, inputs, menus
   (`ui-content.md`).
4. **Does a proven crate or package solve it?** `build-crates.md` on Bench's side; Pi's packages
   and popular ones on the agent's.
5. **Otherwise** the minimum that works, in the house style.

Always: logic with branches, parsers, time or money gets a small test; errors are shown, never
swallowed; nothing that can lose data runs without a confirmation; the report says what was
checked and what was not.

Where everything is: the map in `SKILL.md`.
