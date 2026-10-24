# Switching off the modes a person doesn't use

`scripts/new-bench.sh` copies the reference Bench with everything on: the Hermes chat
(Sessions, ⌘1), SSH servers with terminals (⌘2), the mini IDE (⌘B), Tools (⌘3), Tasks over
Apple Reminders (⌘4). Most people need fewer. **Don't delete code — switch the mode off** in
`src/bench.rs`:

```rust
pub const SSH: bool = true;    // servers + terminals: the terminal icon in the header, ⌘2
pub const IDE: bool = true;    // file tree + editor beside a chat or terminal: ⌘B, ⌘P, "Open files"
pub const TOOLS: bool = true;  // everyday utilities: the wrench over the sidebar, ⌘3
pub const TASKS: bool = true;  // Apple Reminders as a task list: the list icon over the sidebar, ⌘4
```

`false` removes the mode's toggle and shortcut; its code stays and still builds, so turning it
back on later ("I want the tasks after all") is one word. Then `cargo build`, run, screenshot.

What to switch off, from the interview (`start-interview.md`):
- No servers in their work → `SSH = false`.
- They never look at files the agent changed → `IDE = false` (the Changed files card stays; its
  "open" goes to the system app).
- None of the 19 tools fits → `TOOLS = false`; when some fit, keep the mode and remove tools from
  its list instead (`templates/examples/tools-mode/`).
- Their tasks live elsewhere, or they have none → `TASKS = false`. On: the first open asks
  macOS for Reminders access (the .app must carry `NSRemindersFullAccessUsageDescription`, the
  bundle script already adds it).

The Hermes chat itself is the core and stays. A person who asked for an app **without** an
agent ("just a text editor") doesn't get a stripped Bench: they get `scripts/new-app.sh` (the
frame alone) — and the chat can be added to it later, as a panel for the open document
(`build-hermes-client.md`).

Then add what is theirs: a new mode (`build-modes.md`), a tool, a card under an answer.
