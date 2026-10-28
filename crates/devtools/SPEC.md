---
id: devtools
type: module-design
status: draft
title: crates/devtools — the /tools utilities as pure functions
parent: architecture
---

# devtools

The everyday utilities the `/tools` insert offers — Base64, URL and HTML entities, JSON ⇄ YAML,
Unix time, hashes, UUIDs, regex, cron, HTML ⇄ Markdown and the like. `Tool` is the list (names,
groups, modes) and `run` applies one to its input and options.

## Boundary

- Pure functions of input and options, each tested. No GPUI, no I/O, no Bench types: the
  insert's view (`src/inserts/tools/`, `inserts`) stays thin and holds only what is shown.
- The agent has no tool of its own for a utility: the utility's input and output reach the model
  in the insert's `<block-state>` with the next message (`inserts`).
- A new utility is a `Tool` variant and its function here; its `/` word follows from the list.
