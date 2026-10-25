# The UI catalog: every piece a Bench is made of

Like a component library's docs: each piece has a picture, when to use it, and the call. Build
every screen ONLY from these pieces. If something seems missing, it is almost always one of them
in another place (a thread's block, a pane, a row action, a card): look again before making a new one,
and ask the person before inventing a new kind of control.

Where the code lives:
- **`kit::…`** — `templates/app/src/ui/kit.rs`, the piece library (not a way to start an
  app): copy the piece you need into your `src/ui/`. Bench has the same pieces in its own files
  (`src/ui/mod.rs` `block_head`, `search_band`, `render_sidebar_top`; `src/ui/tools.rs` `pane_bar`, `small_button`, `segmented`).
- **gpui-kit components** — `gpui_kit::component::…` (editor, inputs, menus, Markdown…).
- **Bench** — `~/Projects/bench/src`, the reference app, for bigger pieces to copy.

The whole frame in one picture: `assets/gallery.png` (its code: `templates/examples/gallery.rs`,
compiled and checked). A real block built from these pieces: `assets/ui/tools-mode.png` (the Tools block's two panes).

Where each piece goes in the window, and how to move or fit one: `ui-areas.md`.

## The pieces

| Piece | Picture | Call | Page |
|---|---|---|---|
| Window frame: sidebar, header, body, status bar | `assets/gallery.png` | Bench `src/ui/mod.rs` | `ui-frame.md` |
| Header bar (36px, title + icon buttons) | `assets/ui/header-bar.png` | `kit::header_bar` | `ui-frame.md` |
| Status bar (28px, facts left, pickers right) | `assets/ui/status-bar.png`, `assets/ui/bench-status-bar.png` | `kit::status_text`, `kit::status_button` | `ui-frame.md` |
| Top strip over the sidebar (+, Hide sidebar) | — | `kit::sidebar_top`, `kit::mode_toggle` (its icon button; Bench `render_sidebar_top`) | `ui-frame.md`, `ui-buttons.md` |
| Search band (36, rules above and below) | `assets/ui/search.png` | `kit::search_band` | `ui-frame.md` |
| Bottom button filling its bar | `assets/ui/primary-button.png` | `kit::bottom_bar(kit::primary_button(..))` | `ui-frame.md` |
| Primary button (the one filled button) | `assets/ui/primary-button.png` | `kit::primary_button` | `ui-buttons.md` |
| Icon button (header, rows; toggles) | `assets/ui/header-bar.png` (right end) | `kit::icon_button` | `ui-buttons.md` |
| Small text button (pane strips) | `assets/ui/pane-bar.png` | `kit::small_button` | `ui-buttons.md` |
| Delete without a dialog (arm → confirm) | `assets/ui/sidebar-groups.png` (trash on a row) | `kit::arm_delete_button` | `ui-buttons.md` |
| Group header (folder-like, foldable, "+") | `assets/ui/sidebar-groups.png` | `kit::group_header` | `ui-lists.md` |
| List row (title, muted meta, accent stripe) | `assets/ui/sidebar-groups.png` | `kit::list_row` | `ui-lists.md` |
| Compact sidebar rows (one line, groups) | `assets/ui/bench-sidebar-tools.png` | `kit::list_row` with `meta: None` | `ui-lists.md` |
| Tabs (open documents) | `assets/ui/tabs.png` | `kit::tab_strip` | `ui-lists.md` |
| Row menu "⋯", right-click menu | — | `dropdown_menu`, `context_menu` | `ui-overlays.md` |
| Settings row (title, hint, control) | `assets/ui/settings-rows.png` | `kit::settings_row` | `ui-forms.md` |
| Segmented control (pick one of 2–4) | `assets/ui/settings-rows.png`, `assets/ui/pane-bar.png` | `kit::segmented` | `ui-forms.md` |
| Checkbox (yes/no setting) | — | `Checkbox::new(..)` | `ui-forms.md` |
| Text field, multi-line field, composer | `assets/ui/search.png` | `Input`, `Textarea` | `ui-forms.md` |
| A field that reads as text ("Name a group or Select folder") | — | `Input::new(..).appearance(false)` | `ui-forms.md` |
| Picker in the status bar (choose from a list) | `assets/ui/bench-status-bar.png` | ghost `Button` + `dropdown_menu` | `ui-forms.md` |
| Pane strip (Input / Output + buttons) | `assets/ui/pane-bar.png` | `kit::pane_bar` | `ui-content.md` |
| Two panes side by side (input → output) | `assets/ui/tools-mode.png` | `h_flex().items_stretch()` | `ui-content.md` |
| Code / text editor | `assets/ui/tools-mode.png` (right pane) | `EditorState` + `Editor` | `ui-content.md` |
| Markdown, HTML, plain selectable text | `assets/ui/tools-mode.png` (left pane: HTML) | `TextView::markdown` / `::html` | `ui-content.md` |
| Code block with a header (language, Copy) | — | Bench `render_code` | `ui-content.md` |
| A list re-read from its source (Tasks) | — | Bench `src/ui/tasks.rs` | `build-insert.md` |
| Image, diff, terminal | — | `img`, `Diff`, Bench `term.rs` | `ui-content.md` |
| Empty state ("Open a file to start") | `assets/ui/empty-state.png` | `kit::empty_state` | `ui-content.md` |
| Tooltip, toast, popover, popup list, loader | — | `Tooltip`, `push_notification`, `Popover` | `ui-overlays.md` |
| Things that went wrong and the fix | — | — | `ui-gotchas.md` |

## How every clickable piece takes its handler

```rust
// One action: a boxed listener.
Box::new(cx.listener(|this, _, window, cx| this.open_file_dialog(window, cx)))
Box::new(cx.listener(move |this, _, _, cx| { this.selected = k; cx.notify(); }))

// Several items (tabs, segmented): an Rc that gets the item's index.
let view = cx.entity();
Rc::new(move |k, _window, cx| view.update(cx, |this, cx| { this.tab = k; cx.notify(); }))
```

Change the state, then `cx.notify()` so the window repaints. Ids must be unique: `"name"`,
`("name", k)` or a `format!` string.

## The rules every piece already follows (don't break them around it)

Flat and square (no radius, no shadows), one accent, red only for errors, 16px inset from
every panel edge, bars that face each other the same height, text that truncates instead of
wrapping in one-line places, actions that appear on hover, no dialogs for simple choices.
The full list with the reasons: `design-look-rules.md`; colors and sizes: `design-system.md`.
