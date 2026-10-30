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
to reach it (`connection.rs`), installing it and bringing it up as its machine's service, the same on this Mac and over SSH
(`launch.rs`, `service.rs`),
its typed JSON-RPC session (`rpc/`), its REST API (`rest/`), `bench --probe`, and the seam a fake
agent plugs into (`fake.rs`).

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

- **A fake agent behind one seam**, as Zed's `FakeFs` (`fake.rs`, `FakeAgent`, installed once at start
  by `use_fake`): `Client`, `Connection` and the few readers of the machine (git, `~/.ssh/config`, a
  terminal's program, the browser's gateway) ask it first; nothing else in Bench knows it is there. The
  demo is its one implementation (`src/demo/agent.rs`).

- **The agent is its machine's service, one mechanism everywhere**: launchd on macOS, systemd
  (user, lingering) on Linux. Bench never starts it as its own child: closing Bench stops no agent
  on any machine; the service starts it with the machine, again when it dies, and its restart (the
  agent's Restart) ends the whole process tree. Bench runs one script per step — install, bring
  up, restart — in a local shell or over SSH, so this Mac and a server never differ. (Before, this
  Mac's agent was Bench's `nohup` child and a server's a `setsid` one: they outlived Bench as
  orphans nobody watched, and a test copy's agent stayed behind.)
- **One agent per home**, held by the agent itself (`agent.pid`, `agent/src/instance.ts`): a
  second start exits saying which agent holds it — two on one `pi.sqlite` would resume the same
  runs.
- **The agent's home is `~/.bench` on each machine**; the installed server lives in its `app/`,
  unpacked from the sources this Bench carries, their hash in `app/.build`. A running agent of
  other sources is restarted at once — its conversations are durable, a run it was in resumes. An
  agent therefore always matches this Bench, and the wire needs no versioning.
- A copy of the home (`BENCH_HOME`) gets no service: Bench only connects to the agent started on
  it by hand, so trying Bench on other data leaves nothing running.
- A server's agent is reached through an `ssh -L` tunnel; everything about connecting to the
  server is its block in `~/.ssh/config`, never duplicated in Bench.
- Installation is recorded by a marker written only after a successful `npm ci`, never inferred
  from a `node_modules` folder (an interrupted install was not retried).
