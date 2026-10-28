---
id: bench
type: goal-and-requirements
status: draft
title: Bench — a native workbench for working with an AI agent
---

# Bench

The root of Bench's spec graph. Every `SPEC.md` in this repository (and a few documents in
`docs/` that carry spec frontmatter) is a node of it; `parent` links mirror the code tree, and
`scripts/specs-check.mjs` keeps the links whole. A module's spec says what the code alone does
not: what the module is for, where its boundary runs, the decisions and invariants behind it.
Read the spec of the module you are about to change; a change that moves a boundary or
overturns a decision updates the spec in the same commit.

## Goal

A native macOS desktop app, written from scratch in Rust and GPUI, in which one person works
with an AI agent: talks to it in threads, watches and steers its work, and keeps the tools the
work needs (a terminal, a browser tab, small utilities, the project's files) live in the same
place. Bench is its own agent's client (`agent/`, a Node server on Pi Durable); it develops
itself, in itself.

It is made first for its author — a deep alpha, run on local models as well as hosted ones — so
the code must stay obvious enough for a weak local model to change it (small files, Zed's
words, one place for each change: `.agents/skills/bench-structure/SKILL.md`).

## The concepts

The exact vocabulary — thread, insert, card, agent, project, host, Explorer, the gear — is
defined once in `AGENTS.md` ("The concepts") and used unchanged in every spec. Three cores carry
the product, equal in weight:

- **Threads** — a conversation with the agent as a document of messages, the agent's work and
  the person's inserts (`thread-core`, `thread-view`, `inserts`, `thread-objects`).
- **Projects and agents** — where the work lives: which agent, which machine, which folder,
  and the environment the agent works with (`projects-and-agents`, `env`).
- **Explorer** — the project as an IDE shows it, built in (`explorer`).

## Capabilities

- Threads on Pi Durable: every step durable, a turn survives a restart of the agent or of
  Bench; a queue of messages while the agent works ("send now" steers); pause; fork from an
  answer; the agent's plan, questions and approvals as cards; background processes; subagents.
- Inserts put with `/`: `/terminal` on the thread's machine, `/browser` (a thread's own tab in
  a server browser), the `/tools` utilities. Each is live, tells the agent what it shows with
  every message, has an agent tool, and is restored exactly.
- Projects on this Mac or on servers: a server's own agent (installed and started over SSH by
  Bench), or this Mac's agent working on a server over SSH with nothing installed there; a
  project optionally kept in its folder.
- A project's environment: soul, MCP servers (with OAuth), language servers, skills, prompts,
  code mode, compaction, approvals, its Telegram bot, a schedule; exported to and imported from
  one file.
- The Explorer: the project's tree, documents in tabs, the thread beside, ⌘P / ⌘⇧F, language
  servers' problems, hover and definition, git changes against HEAD, a terminal in the bottom
  dock; code, problems and terminal output sent to the thread as references.
- Media in answers (images, video, audio, Mermaid diagrams, LaTeX), voice input, notifications.

## Principles

- **Pi is used unmodified, in Pi's terms.** What Pi lacks or gets wrong is worked around in our
  code and said so in a comment; Pi is never patched. Popular packages before our own.
- **One source of truth for what a thread shows**: Pi's journal and Pi's documents
  (`thread-objects`). The agent keeps no second copy; Bench polls nothing.
- **The look is a Mac app's**, settled as final on 2026-10-28: extend it with Apple's values,
  never a look of one's own (`ui`).
- **The structure is Zed's**: workspace, docks, panels, items; Rust modules and crates
  (`architecture`).

## Non-goals

- Not a kit for building other apps; not a general IDE (Zed is one click away — the Explorer
  stays narrow, and no external editor is referenced or needed).
- No profiles for the person: there are projects.
- No task manager in Bench (removed on 2026-10-28).
- Nothing of Hermes, the earlier agent: no compatibility on the wire.
- A project has no model of its own: one model, set in the gear, for every thread.
