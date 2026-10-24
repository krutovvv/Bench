# Custom render elements and raw input (pinned gpui-pre fork)

> **Stop — you probably don't need this file.** A text or code editor, a file viewer, Markdown,
> lists, menus: use the ready components (`references/ui-content.md`). This file is only for drawing
> that no component does (Bench's terminal grid is the one example), and only when the user asked
> for it. A session that used it to write its own editor failed after 2 hours.

When component widgets can't do the job (canvas-like surfaces, a terminal grid), you drop to the
raw GPUI API. gpui-kit pins a PRE-RELEASE GPUI fork
(`gpui-pre` / `gpui-base`), whose API differs from Zed-head and from every tutorial. Verified
against gpui-pre-0.3.9 / gpui-base-0.8.0 / gpui-component-0.8.0 — grep
`~/.cargo/registry/src/*/gpui-{pre,base}-*/src` for anything not listed here; never guess.

## Crate map

- `gpui_kit::*` — re-exports the fork's public surface (`pub use ::gpui::*`), incl. `h_flex`/
  `v_flex`/`div` (also present at `component::` root).
- `gpui_kit::component` — gpui-component. **`Button`, `Input`, `Label`, `Editor` are NOT at the
  component root** — import them from their modules: `component::button::Button`,
  `component::input::{Input, InputEvent, InputState}`, `component::label::Label`.
- `InputState` itself lives in `gpui_base::input` (re-exported by `component::input`). Value:
  `state.read(cx).value()` → `SharedString`; programmatic set is
  `set_value(text, window, cx)` (needs `&mut Window`, emits no `Change`, clears undo).

## The `Element` trait (gpui-pre 0.3.9)

```rust
fn request_layout(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>,
                  window: &mut Window, cx: &mut App) -> (LayoutId, Self::RequestLayoutState);
fn prepaint(... , bounds, &mut layout_state, window, cx) -> Self::PrepaintState;
fn paint(..., bounds, &mut layout_state, &mut prepaint_state, window, cx);
fn source_location(&self) -> Option<&'static core::panic::Location<'static>>; // required
```

- Full stretch in request_layout: `style.size.width = Length::Definite(relative(1.))` — `Length`
  has only `Definite`/`Auto` variants; bare `Some(relative(1.))` does not typecheck.
- Hitbox for events: `window.insert_hitbox(bounds, HitboxBehavior::Normal)` in prepaint.

## Geometry and colors

- `Pixels` field is **private** (`pub(crate) f32`): convert with `f32::from(p)`, build with
  `px(f32)`; `Pixels / Pixels` yields `f32` (impl Div). Never write `p.0`.
- `Hsla` fields are `h, s, l, a` — GPUI colors are **HSL, not RGB**. Use `hx("#rrggbb")` or
  convert; `hx` isn't const, so palette colors as `fn` not `const`.
- Theme fields (`cx.theme().colors`, gpui_component `theme_color.rs`): `background`,
  `foreground`, `muted_foreground`, `accent`, `border`, `title_bar`, `caret`, …
  `.theme()` needs `use gpui_kit::component::ActiveTheme as _;` (impl'd for `App`; `Context<T>`
  and `Window` reach it by deref).
- `.when()/.when_some()/.map()` come from trait `FluentBuilder` (gpui `util`) — make sure it is
  in scope if the compiler says the method doesn't exist on `Div`.
- There is **no `z_index`** on `Div` — stacking is DOM/paint order; paint overlays after the
  surface they float on.

## Raw window-level events (outside `div` handlers)

Keyboard actions are NOT `Keystroke` — `Keystroke` does not impl `InputEvent`; the event is
`KeyDownEvent` with the keystroke inside:

```rust
window.on_key_event::<KeyDownEvent>(move |ev, _phase, window, cx| {
    let ks = ev.keystroke.clone();
    window.update(cx, |window, app| {
        app_entity.update(app, |this, cx| this.handle_key(&ks, window, cx))
    });
});
```

- Mouse/wheel: `on_mouse_event::<MouseDownEvent | MouseMoveEvent | MouseUpEvent |
  ScrollWheelEvent>`; `ScrollDelta::Lines` (cells) vs `Pixels`; check `hitbox.bounds.contains(&ev.position)`.
- In these callbacks the `cx` is an async-style context — `cx.update(entity, …)` is **private**;
  reach the view via `window.update(cx, |window, app| entity.update(app, |this, cx| …))`.
- Cursor: `window.set_cursor_style(CursorStyle::IBeam, &hitbox)` — `IBeam`, not `Text`.

## Painting a text surface

- Shape: `window.text_system().shape_line(text.into(), px(size), &[TextRun{len, font, color,
  background_color, underline, strikethrough}], None)`; measure once via `shaped.width`
  (`Pixels`).
- Draw: `shaped.paint(origin, px(line_height), TextAlign::Left, max_width_or_None, window, cx)`.
- Clip to the element: `window.with_content_mask(Some(ContentMask { bounds }), |window| …)`;
  background/selection rects: `window.paint_quad(fill(bounds, hsla))`.
- Virtualize: paint only rows in `[scroll_y/line_h, +visible_rows]`; for huge single lines shape
  only a character window around the horizontal scroll offset; keep style caches keyed by line
  index behind an `Rc<RefCell<…>>`, invalidated from the first edited line.
- Share per-frame state (scroll, measured font advance, reveal counter) with event handlers via
  a `ViewportShared` of `Rc<Cell<f32/usize>>` cloned into both the element and the handlers.
