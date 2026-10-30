---
id: thread-core
type: module-design
status: draft
title: crates/thread — a thread as rows, without GPUI
parent: architecture
depends-on: [protocol]
references: [thread-objects]
---

# thread

Turns what the agent says about a thread (`ThreadOpen`, then `ThreadEvent`s) into the ordered
`Row`s Bench draws: the person's messages, the agent's work, answers, inserts, failures, the
answer being written, the queue. It is the only place that decides **what the thread shows and
in what order**; `thread-view` only draws its rows.

## Boundary

Depends on `protocol` and serde_json, nothing else: no GPUI, no I/O, no Bench types. That keeps
the rules of order testable in seconds (`cargo test -p thread`) and the crate usable by any view.

## Invariants

Each rule is a test in `src/tests.rs`; the list in `lib.rs` is their index. The model behind
them — every row stands on a journal entry, state that is not in the journal is a Pi document,
the agent never writes a second record "for Bench" — is `thread-objects`. In short:

- The journal comes first, ordered by item key and nothing else; then the answer being written;
  then the queue, in the agent's order.
- A queued message the agent takes becomes a journal row in the same place on screen; one
  just sent shows at once and is matched to its journal entry by the `requestId` Bench gave it,
  never by its text.
- Steps and time are counted per piece of work between two messages, never per run.
- Updating a row's state (a plan step done, a process ended, a question answered) moves neither
  it nor its neighbours.
- An insert sits where its entry is; a removed one is not shown.

`steps.rs` lays an opened piece of work out for a person, from the journal alone: as
milestones in order — the agent's own words between its calls, or, where it wrote none, a
sentence of the calls themselves ("Read things-report, edited it twice and ran things-do") — and
inside each its calls in order, each what it did — in the agent's own words where it wrote some
(the label it echoed, what its AppleScript asks, the file its script works on) — beside the first
line of what came back. The reasoning that led to a call (`reasoning_before`) is on the call's card, all
of it behind "Show all".
A call is said by what it does and to what: a command by what its program does, read from its
own syntax (escaped spaces, variables, substitutions); never a pattern or a flag. Nothing is
written for it: old threads read the same as new ones. `tool_labels.rs` says a call in one line
(its label, the live word, the pages it touched); a URL's host is read there alone.

`diff.rs` gives the changes between two row lists, so the view updates only what changed and
keeps its scroll (`thread-view`).

## Decisions

The crate replaced a thread assembled from three lists (turns guessed from a flat history, a
queue apart, inserts placed by counting messages), glued together with heuristics that made rows
jump; the history of that is `docs/thread-rewrite.md` (a plan, kept as background, not a spec).
