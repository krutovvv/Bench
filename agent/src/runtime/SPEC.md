---
id: agent-runtime
type: submodule-design
status: draft
title: agent/src/runtime — the threads' core
parent: agent
depends-on: [agent-machine]
references: [thread-objects, thread-core]
---

# The runtime

`Runtime` (`runtime.ts`) makes, opens, sends to, stops and forks threads on Pi Durable, and is
where its parts meet. Each part is one file reached as `rt.<part>`, with one job: the model and
its fallback (`models.ts`, `fallback.ts`), a run's live events (`stream.ts`), the journal read for
Bench (`journal.ts`), the pause (`hold.ts`), the queue (`queue.ts`), questions and approvals
(`asks.ts`), inserts (`inserts.ts`), titles, usage, and the tools' host (`tools-host.ts`).
`run.ts` runs one prompt to its answer in a fresh thread (scheduled jobs, the tools' `run`).

## What Bench reads, and from where

- **The journal is the only record of a thread.** `Journal` reads a thread whole (`thread.open`:
  the items after a head, the queue, the inserts' state) and turns each Pi commit into
  `thread.*` events; it also feeds the search index. An item's key is built from its entry id and
  its place inside the entry, so the order is the journal's and never counted (`thread-core`
  draws it).
- **`stream.ts` is only what is live**: the answer being written, a run's start and end, its
  numbers, why it failed. Nothing Bench keeps comes from here.
- What Pi rebuilds after a restart (who waits for an answer, what is being written) is held in
  memory and given in `thread.open` and events; it never places a row (`thread-objects`).

## Decisions

- **What a turn changed is git's view, not the tools'** (`turns.ts`): the working tree written as a
  tree when a run starts and when it ends, through a copy of the index (the person's index,
  branch and stash untouched), the differing files a display-only `bench.changes` entry after the
  answer. It sees a formatter's or a script's changes as well as an edit's; outside a repository
  nothing is written and Bench falls back to the file tools' own diffs.

- **A message's id is Pi's `requestId`**, given by Bench: the row shown on Enter is matched to
  its entry by it, never by its words. A message sent while a run is live waits in Pi's inbox as a
  follow-up; "send now" steers it into the run after the current tool round.
- **The model is one for every thread** — the general agent's, followed at every turn with the
  thread's effort and extensions — so a change in the gear or a project's code mode takes effect
  from the next turn.
- **The fallback is a pi-ai provider of ours**: Pi retries only with the same model, so one
  provider stands for the pair and sends the same request to the fallback on an error or a
  silence. Pi sees one response; the thread's `info {fallback}` lets Bench show it.
- **The pause is a Pi document** (`bench.hold`) read by a gate in `beforeTool`; Pi calls
  `beforeTool` before it records a call's intent, so a held run stays held across a restart. A
  fork starts free.
- An insert's `<block-state>` is appended to the message text the model gets and stripped from
  what Bench shows (`BLOCK_OPEN`/`BLOCK_CLOSE`), so the model's transcript keeps each one
  (unconfirmed whether that growth is intended; `AGENTS.md` says "kept out of the history").
