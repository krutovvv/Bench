# Pi's MCP: exposure and mcp.json

A short account of Pi's MCP documentation (https://pi.dev/docs/latest/mcp) for
`@earendil-works/pi-mcp`. The package's own README does not cover `exposure`, so this is not a bundle
of a source file: read the live page when a detail matters. How Bench uses it:
`agent/src/mcp/SPEC.md`.

## Exposure

Set per server with `exposure`. The default is `codemode`.

| Value | What it does |
|---|---|
| `codemode` | Callable only from code mode scripts: neither declared to the model nor listed in code mode's description. Scripts find the tools with `searchTools()`, `describeTool()` or `ALL_TOOLS`. `codemode-deferred` is an alias |
| `deferred` | Hidden until `tool_search` loads a match for the next model call |
| `direct` | Declared to the model like a built-in tool, and callable from code mode |
| `hidden` | Registered but unreachable |

`toolExposure` sets it per tool. Keys are exact tool names or patterns with `*`. An exact name beats a
pattern; among patterns the first match wins. So a server can be hidden by default with only chosen
tools exposed.

## How the tools are reached

- Code mode scripts can call `codemode` and `deferred` tools. This does not depend on the active tool
  set, so they stay available after a resume or a fork.
- `tool_search` loads deferred tools for the next model call; what it loaded is recorded in the
  transcript and stays declared on that branch.
- `direct` tools are always declared to the model.

## mcp.json

- Stdio servers: `command`, `args`, `env`, `cwd`.
- HTTP servers: `url`, `headers`, `oauth`.
- Both: `type` (optional), `timeout` (60 seconds by default), `enabled`, `exposure`, `toolExposure`,
  `description`.
