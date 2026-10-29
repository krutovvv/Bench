---
id: thread-view
type: submodule-design
status: draft
title: src/thread — a thread on screen
parent: app
depends-on: [thread-core, ui, inserts]
references: [thread-objects]
---

# A thread on screen

Draws one thread — the rows `thread-core` gives, the cards that hang around them, the composer —
and sends what the person does back to the thread's agent. It also keeps every agent's threads
(the sidebar's lists, drafts, journals seen in this run: `store.rs`) and opens, creates and
deletes threads (`open.rs`).

## Boundary

- **Order is not decided here.** Which rows exist and in what order is `thread-core`'s; this
  module draws a `Row` and never reorders, merges or hides rows itself.
- **The thread is its own entity**, `ThreadView` (as Zed's `AcpThreadView`): its state
  (`ActiveThread`), its composer, its run, its queue, its events. The center shows one; the right
  dock a second beside it ("Open beside", `beside.rs`), fully independent.
- **Two rules keep the view and the app from being borrowed twice.** What a view asks of the
  app names the thread with a `ThreadRef` and never reads a view (the view asking may be drawing
  at that moment). What the app asks of a view never reads the app; when a view's click needs
  the app to come back to it, the call is deferred.
- A thread's meters (context, speed, fallback) are the thread's, shown under its composer with
  its project and repo (`composer/foot.rs`) — not the window's.

## Invariants

- **Nothing moves the list but the person.** A run that ends, a row that grows, an answer that
  arrives never scroll the thread, as code in an editor does not scroll while it changes. The one
  exceptions: after the person sends, the composer is brought back into view if the new rows
  pushed it past the bottom edge; and a thread the person opens lands at its end (its last
  message and the composer in view), once its history has come.
- Rows reach the list by diff (`thread::diff`): unchanged rows keep their measured heights, only
  the count is spliced at the end, so the reader keeps their place even inside a row being
  written. Another thread opens at its end.
- `gpui::list` lays out only what is on screen; a thread of thousands of rows costs what a
  screenful costs.
- Only the thread scrolls (with a Mac overlay scrollbar); a block inside it does not scroll on
  its own, except a terminal, an editor, or a tool pane with its own wheel.

## Decisions

- **The look is one type**, chosen in `look.rs`: `Chat` (the default — the person's grey bubbles,
  no avatars, the composer pinned under the list) or `Document` (avatars in a gutter, the
  composer as the last row, an editor's room past the end). Switching is one line; nothing else
  branches on it.
- Each kind of row is a file in `rows/`; the composer's parts are files in `composer/`. A new kind
  of agent message is a new row file (the `BlockKind` extension point, with an unknown kind drawn
  as text, is the intent of `architecture` §6, not yet cut out — unconfirmed how unknown kinds
  are drawn today).
- Messages carry code, problems and terminal output as references (chips) that become fenced
  blocks with `path:lines` under the message's words — what the agent reads is what the thread
  shows (`explorer`).
- **The person works in the chat; the Explorer is where they go when they choose to.** What a
  turn changed shows under its answer as one folded line ("Changed files · N files +a −d",
  `rows/changed_files.rs`, from `Row::Changes`); a file's diff opens in place under its row, and
  only "Open in Code" on its hover takes it to a tab. Anything new the thread shows starts as such
  a quiet line and unfolds on a click: no panels, no badges that ask for attention.
- A review comment is a reference too (`Reference::Comment`, ⌘⇧L in the Explorer): `Comment <id>
  on \`path:lines\`: <note>` with the lines fenced; the agent closes each with `resolve_comment`.
- **A turn is reviewed when the person asks**, from "Review" at the far edge of its changes' head:
  one call to the thread's model reads what was asked, the plan, the answer and the turn's diff
  (`agent/src/runtime/review.ts`); its findings fold under the files and go to the agent only
  through "Send to the agent", as review comments. Nothing reaches the agent unasked.
