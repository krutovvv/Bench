---
id: client
type: submodule-design
status: draft
title: src/client — talking to the agent
parent: app
depends-on: [protocol]
references: [agent, projects-and-agents]
---

# The client

Everything Bench needs to reach an agent, as Zed's `client` crate: where the agent runs and how
to reach it (`connection.rs`), installing and starting it on this Mac or over SSH (`launch.rs`),
its typed JSON-RPC session (`rpc/`), its REST API (`rest/`), and `bench --probe`.

## Contracts

- **RPC**: JSON-RPC 2.0 over a WebSocket at `/api/ws?token=`. `Client` answers a
  `protocol::Request` as a future of that request's own response type; what the agent says unasked
  arrives as `Incoming` on one channel. One client per agent; which agent a request goes to is
  `app/agents.rs`'s choice, not this module's.
- **REST**: blocking calls under `/api/` with the `X-Bench-Token` header — files, settings,
  providers, projects — made off the UI thread. `with_settings` names the agent settings a call is
  about.
- **A session token** per agent, in `~/.bench/server.json` with its port on the agent's machine;
  port and token are remembered so the next launch reattaches.

## Decisions

- **The agent's home is `~/.bench` on each machine**; the installed server lives in its `app/`,
  copied from the sources this Bench carries. A running agent is reused; one built from other
  sources is restarted when idle — its conversations are durable, so nothing is lost. A server's
  agent therefore always matches this Bench, and the wire needs no versioning.
- A server's agent is reached through an `ssh -L` tunnel; everything about connecting to the
  server is its block in `~/.ssh/config`, never duplicated in Bench.
- Installation is recorded by a marker written only after a successful `npm ci`, never inferred
  from a `node_modules` folder (an interrupted install was not retried).
