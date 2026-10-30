# Menus, popovers, tooltips, toasts, loaders

Things that appear over the window. They attach to what they belong to (a menu under its
button, the `/` list under the line being typed), rounded (theme radius 4, `rounded_md`), 1px
border, no shadow.

## Menu on a button ("⋯" on a row, a picker)

```rust
Button::new(("row-menu", i)).ghost().xsmall().icon(Ico::Ellipsis.icon())
    .dropdown_menu(move |menu, _, _| {
        menu.item(PopupMenuItem::new("Pin").icon(Ico::Pin.icon()).checked(pinned)
                .on_click({ let view = view.clone(); move |_, _, cx| view.update(cx, |this, cx| this.toggle_pin(i, cx)) }))
            .separator()
            .item(PopupMenuItem::new("Delete").on_click({ let view = view.clone(); move |_, _, cx| view.update(cx, |this, cx| this.arm_delete(i, cx)) }))
    })
```

- `.dropdown_menu` exists on `Button` only (gpui-component's `DropdownMenu`), not on a `div`:
  a menu on an icon is an icon `Button` (`.ghost().small().icon(…)`, no label — an empty label
  still adds the label's padding and gap). A right-click menu (`.context_menu`) goes on any element.
- Item closures are `'static`: clone the view entity (`let view = cx.entity();`) into each and
  call `view.update(cx, |this, cx| …)`.
- Separate unrelated groups of items with `.separator()` (Settings | Pin, Rename | colours | Delete).
- A custom row (color swatches): `PopupMenuItem::element(move |_window, cx| …)`; it is
  re-rendered, so read live state inside (`view.read(cx).…`) — a captured value leaves the check
  mark stale until the menu reopens.
- Don't open a menu before its items exist (no empty dropdowns).

## Right-click menu

.`context_menu(move |menu, _, _| menu.item(…))` on the row element. Bench builds one menu,
`item_menu` in `src/sidebar/menu.rs`, and gives it to both the row's hover "⋯"
(`dropdown_menu`) and its right-click (`context_menu`): in Finder's order — a project's New Thread,
Code, Settings…; then Rename…, a thread's Pin; colours; Delete (Remove Project…). Controls that must be found are **on the
row, on hover**, not in the menu: the project's gear (its environment, in the Explorer) and its
"+" sit beside the "⋯"; the fold chevron is always shown. Settings is deliberately not a menu
item -- a door that only opens on a right click is a door nobody finds.

## Popover (a panel on click)

```rust
Popover::new("stats").anchor(gpui_kit::Anchor::BottomRight)
    .trigger(Button::new("stats-btn").ghost().xsmall().label(label))
    .content(move |_, _, cx| v_flex().child(bench_ui::popover_section(false, cx).child(…)))
```

`trigger` must be selectable (a `Button` is). Anchor `BottomLeft` for things at the window's bottom (they open upward).
Its parts are `bench_ui::popover_section(ruled, cx)` and `popover_title`: no padding of its own, the rules edge to edge.
Bench: the agent's readout in `src/sidebar/agent.rs`; the thread's meters in `src/thread/meters.rs`.

## Tooltip

Every icon-only button has one (kit buttons take `tip`). On your own element:
`.id(..).tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))`. Name the
shortcut in it: `"Stop  ⌘."`.

## Toast (a message in the corner)

`window.push_notification((NotificationType::Error, msg), cx)` — for a failed action the person
started. Prefix the source ("SSH: connection refused"), show the backend's real reason. Never toast
background noise (a poll that failed): log it to the app's log file instead. Bench `fn toast`
in `src/app.rs`.

## A list attached to a field (the `/` commands)

`render_slash` in `src/thread/composer/input.rs`: an absolute panel (`bg(theme.popover)`, `border_1`,
`rounded_md`) at the text's column, under the line being typed — the composer is the thread's
last row and the thread scrolls on past it, so there is room. When the window has none below
(`viewport_h` against `composer_y` and the list's own height), it opens above the line. The
parent is `.relative()`; the list scrolls inside itself (`max_h`, `track_scroll`) — a popover,
not the thread. Keys (↑ ↓ Tab Esc) are caught with `.capture_key_down` on the
composer before the text field gets them (`ui-gotchas.md`, "Keyboard").

## Loading

- A spinning loader icon (one that doesn't spin reads as "broken"): `Ico::Loader.live(true)`
  — Bench's `crates/ui/src/icons.rs` animates it (`true` = small, `false` = extra small).
- A live label: `ShimmerText::new(label)` (the agent is working).
- A breathing dot: `.with_animation("breathe", Animation::new(Duration::from_millis(1400)).repeat().with_easing(pulsating_between(0.45, 1.)), |d, t| d.opacity(t))`.

## Cards under an answer

A small bordered block under a chat answer for things the answer produced: changed files,
sources, background processes (status, output, Stop). Same indent as the answer, actions on
the right of each line. Bench `processes_frame` in `src/thread/rows/processes.rs`.

## No dialogs for simple choices

Arm-then-confirm buttons, inline forms that replace the body, sensible defaults. A modal
dialog only for something that truly blocks (none in Bench so far).
