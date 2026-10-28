---
id: workspace
type: submodule-design
status: draft
title: src/workspace — docks, panels and the center
parent: app
references: [explorer]
---

# The workspace

The window's layout, as Zed's workspace: three docks around the center, each showing one of its
panels, and the center showing one item. The window's places and what fills each — threads or
the Explorer's tree on the left, a thread, a document or a settings page in the center, the
thread beside on the right, the project's terminal at the bottom — are described once in
`docs/explorer.md` (`explorer`, "The window's three places").

## Boundary

This module decides only **which** panel or item is shown, at what size, open or folded; every
panel and item is drawn by its own module (`sidebar`, `explorer`, `thread`, `env`,
`inserts/terminal`). It holds no area's state.

## Decisions

- A `Dock` is a list of `Panel`s with one shown, as Zed's. A new way to look at things is a new
  `Panel` variant in its dock's list and its drawing in `render_panel`; nothing else changes.
- The right dock holds a panel, never a file: a file "beside" would be a second way of opening
  files. Files open as tabs in the center; the right dock shows the thread beside a document, or a
  second thread beside a thread.
- The center's tabs are the center's own bar under the title bar (tabs in the title bar dragged
  the window).
