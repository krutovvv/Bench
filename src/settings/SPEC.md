---
id: settings
type: submodule-design
status: draft
title: src/settings — the gear, and the new project's page
parent: app
depends-on: [client, ui]
references: [agent, agent-judge, projects-and-agents]
---

# The gear

The app's settings (Bench → Settings…, ⌘,), a page in the center (`Center::Settings`), in tabs:
Agents (each agent's providers, voice and evaluator servers; servers' agents added and removed),
Presets (MCP and language servers a project takes with one click, compaction shares, reasoning
levels), Appearance. The new project's page is here too (`new_project.rs`, `Center::NewProject`).
What the gear keeps while it is open is `Gear` (`state.rs`).

## Boundary

- **What an agent is set to is the agent's**: the gear reads and writes it through that agent's REST
  (`with_settings`), never a copy here. The presets are this Mac's agent's. Only the appearance and
  Bench's own choices are kept on this Mac.
- **What a project is set to is not here**: that is its Settings (`env`). The model is here: one for
  every thread of every project.

## Decisions

- **A provider and an evaluator server are one card**: a name, an address, a model and a key, with the
  last check's word and the same small actions. `ServerForm` (`server_form.rs`) is what they share,
  `bench_ui::settings_card` and `settings_add` what they are drawn with. A third kind of server is a
  third user of these.
- **A role is a button on the card, for both**: one is the main and one the fallback (Make main, Make
  fallback; pressed again, no fallback). There is no separate fallback page or picker.
- **A key is set on the card that uses it**, never in a list of names.
- **No switch for the evaluator here**: no server, no evaluator; a project's Settings say whether its
  threads use it (`agent-judge`).
- **The gear belongs to the chat** (`workspace`): opened from the Code view it closes the Explorer
  first, so closing it leaves the chat whole.
- A provider's "Threads at once" is its `parallel`: read from the main provider only, as there is one model for every thread (`agent/src/runtime/slots.ts`). Empty: no limit.
