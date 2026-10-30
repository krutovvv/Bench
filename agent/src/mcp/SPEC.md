---
id: agent-mcp
type: submodule-design
status: draft
title: agent/src/mcp — MCP servers, and how their tools reach a thread
parent: agent
references: [agent-judge, inserts]
---

# MCP

A project's MCP servers (`mcp.json`, Pi's format), through the official client
`@earendil-works/pi-mcp`: stdio or streamable HTTP, per agent, connected with its first thread.
Sign-in for servers that ask for it is `oauth.ts` (callback on `localhost:47823`, credentials in
`~/.bench/mcp-oauth/`). A project worked on a server runs its command servers there, over the same
SSH connection.

## Decisions

- **`exposure` is Pi's, and this module owns its default.** `codemode` by default: the tools are the
  scripts', each server a namespace of code mode, found with `searchTools()`, a `CallToolResult`
  whole; neither offered to the model nor listed. `direct` offers them to the model too, `deferred`
  puts them behind `tool_search`, `hidden` makes them unreachable; `toolExposure` overrides per tool
  (`*` patterns).
- **Why code mode by default:** the person runs local models, and every token of tool declarations
  in the prompt slows each request; one server with 170 tools is 150k tokens.
- **`direct` is a legitimate per-server choice**, not a hack: an old skill that expects direct tools
  moves by setting `"exposure": "direct"` on that server. The `/browser` insert's server is `direct`
  by default, its tools being the insert's agent tool (`inserts`).
- **Tool search only where it has work**: `forAgent` (`extensions.ts`) adds it only where some tools
  are `deferred`, as Pi's description of it sends the model there for every MCP tool.
- A server whose `tools/list` failed is retried; its tools stay known and say why they cannot run
  now, so a thread does not lose them to a moment's failure.
