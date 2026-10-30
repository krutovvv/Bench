---
id: protocol
type: module-design
status: draft
title: crates/protocol — the wire between Bench and its agent
parent: bench
---

# protocol

The one description of everything Bench and its agent say to each other. Both sides are typed
from it: Bench uses the Rust types directly, and `cargo test -p protocol` writes
`agent/src/protocol.gen.ts` (through `ts-rs`), which the agent imports. A test fails when the
generated file was out of date ("written again, commit it"), so a changed type that is not
committed on both sides does not pass.

## What it owns

- **Requests** (`rpc.rs`): one type per method, implementing `Request` with its method name and
  its response type. Bench's client answers a request as a future of that type (`client`).
- **Events** (`event.rs`): one `Event` enum for everything the agent says unasked — a thread
  changed, the threads list changed, the environment changed, and so on.
- **A thread on the wire** (`thread.rs`): `ThreadOpen` (the snapshot after a head), `ThreadEvent`
  (each commit's news), `ThreadItem` and its key, the queue, inserts' state, holds, processes,
  plan steps.

## Boundary

No GPUI, no I/O, no Bench types: serde, serde_json and ts-rs only. `thread-core` depends on it;
nothing in here knows how rows are drawn or how the agent stores anything.

## Decisions

- **Typed methods, never strings.** The earlier protocol of string methods and two big
  `Cmd`/`Evt` enums let client and agent drift apart silently; each new feature now adds a type,
  not a variant in a shared match.
- **The agent tells, Bench never polls.** Anything that changes on the agent's side is an
  `Event`; a new kind of change is a new variant, not a timer in Bench.
- **REST stays outside.** File contents, settings and providers go over REST (`/api/…`), whose
  shapes are not described here (unconfirmed whether that is a decision or not yet done).
- A server's agent is always reinstalled to match the Bench that connects to it, so the wire has
  no versioning and no compatibility with older agents.
