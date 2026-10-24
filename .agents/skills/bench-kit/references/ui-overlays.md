# Menus, popovers, tooltips, toasts, loaders

Things that appear over the window. They attach to what they belong to (a menu under its
button, a command list resting on the composer), square, 1px border, no shadow.

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

- Item closures are `'static`: clone the view entity (`let view = cx.entity();`) into each and
  call `view.update(cx, |this, cx| …)`.
- Separate unrelated groups of items with `.separator()` (Pin | colors | Open in… | Delete).
- A custom row (color swatches): `PopupMenuItem::element(move |_window, cx| …)`; it is
  re-rendered, so read live state inside (`view.read(cx).…`) — a captured value leaves the check
  mark stale until the menu reopens.
- Don't open a menu before its items exist (no empty dropdowns).

## Right-click menu

`.context_menu(move |menu, _, _| menu.item(…))` on the row element. Bench `src/ui/files.rs`
(rows of the file tree: Open, Copy path, Reveal in Finder, Delete).

## Popover (a panel on click)

```rust
Popover::new("stats").anchor(gpui_kit::Anchor::BottomRight)
    .trigger(Button::new("stats-btn").ghost().xsmall().label(label))
    .content(move |_, _, cx| v_flex().p_3().gap_1().child(…))
```

`trigger` must be selectable (a `Button` is). Anchor `BottomRight` for things in the status bar.
Bench: the context-usage readout in `src/ui/statusbar.rs`.

## Tooltip

Every icon-only button has one (kit buttons take `tip`). On your own element:
`.id(..).tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))`. Name the
shortcut in it: `"Tools  ⌘3"`.

## Toast (a message in the corner)

`window.push_notification((NotificationType::Error, msg), cx)` — for a failed action the person
started. Prefix the source ("SSH: connection refused"), show the backend's real reason. Never toast
background noise (a poll that failed): log it to the app's log file instead. Bench `fn toast`
in `src/app.rs`.

## A list attached to a field (commands, mentions)

Rests on the top of the composer, the same width, no gap, no double border:

```rust
div().absolute().bottom_full().left_0().right_0().px_4()
    .child(div().bg(theme.popover).border_t_1().border_l_1().border_r_1().border_color(theme.border)
        .child(v_flex().id("list").max_h(px(28. * 8.)).overflow_y_scroll().track_scroll(&scroll).children(rows)))
```

The parent is `.relative()`. Keys (↑ ↓ Tab Esc) are caught with `.capture_key_down` on the
composer before the text field gets them (`ui-gotchas.md`, "Keyboard"). Bench `src/ui/composer.rs`.

## Loading

- A spinning loader icon (one that doesn't spin reads as "broken"): `Ico::Loader.live(true)`
  — Bench's `src/icons.rs` animates it (`true` = small, `false` = extra small).
- A live label: `ShimmerText::new(label)` (the agent is working).
- A breathing dot: `.with_animation("breathe", Animation::new(Duration::from_millis(1400)).repeat().with_easing(pulsating_between(0.45, 1.)), |d, t| d.opacity(t))`.

## Cards under an answer

A small bordered block under a chat answer for things the answer produced: changed files,
sources, background processes (status, output, Stop). Same indent as the answer, actions on
the right of each line. Bench `render_processes` / files in `src/ui/chat.rs`.

## No dialogs for simple choices

Arm-then-confirm buttons, inline forms that replace the body, sensible defaults. A modal
dialog only for something that truly blocks (none in Bench so far).
