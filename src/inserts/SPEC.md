---
id: inserts
type: submodule-design
status: draft
title: src/inserts — live tools put into a thread with "/"
parent: app
depends-on: [devtools, ui]
references: [thread-view, agent-runtime, agent-tools]
---

# Inserts

An insert is the person's: a small live tool put into a thread with `/` — `/terminal`,
`/browser`, each `/tools` utility by its own word. This module is the registry of insert kinds
(`InsertKind`, `KINDS`), the `/` list, the block every insert sits in (title, menu, controls,
fold, remove), the thread's inserts kept in step with the agent (`state.rs`), and one folder
per kind.

## The contract of a kind

Every kind does all four, or it is not an insert:

1. **Shows something live** — a shell, a utility, a browser tab.
2. **Tells the agent what it shows** with every message: each kind's `context()` is collected
   into the message's context; the agent appends it to the message as `<block-state>` for the model and
   the thread's history drops it again (`agent/src/runtime/runtime.ts`, `journal.ts`). The
   `inserts` prompt section names what is inserted, one line per kind (`agent/src/prompt.ts`
   `inserted()`).
3. **Has an agent tool that acts on it** — `ssh_run` for the terminal (without a host it takes
   the thread's terminal insert), the browser MCP's `mcp__browser__*` in the thread's own tab; a
   utility needs none, its input and output being in `<block-state>`.
4. **Is restored exactly on return** — its state is serde data kept by the agent, not by Bench.

## Where its truth is

The agent keeps inserts in Pi's terms (`thread-objects`, `agent-runtime`): an entry
`bench.insert {id, key}` written where it was put — a write the model does not see and that is
never queued, so it lands at once even mid-run — and its live state in the conversation's
document `bench.inserts` (`{items:{id:{key,state,folded,removed,after?}}}`), changed in commits.
Bench changes inserts only through the agent (`thread.insert`, `thread.insert.patch`,
`crates/protocol/src/rpc.rs`) and reads them back from the thread (`ThreadEvent::Inserts`). A removed insert keeps its entry and is marked removed.

## Invariants

- Every `/` adds a new insert where the conversation is; an insert put while a message waits in
  the composer goes after that message (`after`).
- A thread never switches mode or is renamed because of an insert.
- A kind that can be live only once (the utilities) shows "click to show it here" on its older
  inserts.
- A kind says how its body sits in the block (`inset`: `Padded`, `Rows`, `Flush`): the block adds
  its padding or not, and no body cancels it with negative margins.
- A kind says its own `/` word and choices (`word`, `choices`): adding a kind touches its folder,
  a line in `KINDS`, its state in `InsertsState`, and a line in the agent's `inserted()` — nothing
  else.

## Boundaries of the kinds

- `terminal/` runs on the thread's machine (`thread_machine()`); it is the only thing Bench opens
  over SSH itself, because it needs a live pty. Its shell also fills the bottom dock's terminal
  panel, one per project.
- `browser/` talks to the server browser's `/live` gateway, a project of its own
  (browser-server-mcp); Bench is one of its clients and sends no raw DevTools protocol.
- `tools/` is a thin view over `devtools`.

## Known limitations

A kind's methods take `&mut BenchApp` and its state is app-wide (`InsertsState`), so a kind is
not yet an entity or a crate of its own (`architecture` §6).
