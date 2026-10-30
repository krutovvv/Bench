# Gotchas: what went wrong before, and the fix

Read when something behaves oddly or doesn't compile. Paths: widgets `gpui_kit::component::*`,
core `gpui_kit::*`. One exact name you can't find: the agent's `diagnostics` tool and the
errors after each edit come from the project's language servers (rust-analyzer in its
`lsp.json`, `agent/src/lsp/lsp.ts`) — read them before reading the framework's sources.

## Compile errors that already happened (and the fix)

- **A closure that outlives the loop** (E0597, "borrowed value does not live long enough"):
  copy what the handler needs out of the item first, then `move` that copy.
  ```rust
  for (k, t) in self.tools.iter().enumerate() {
      let id = t.id;                       // a Copy value, or .clone() a String
      rows.push(bench_ui::list_row(format!("t-{k}"), t.name.clone(), None, self.sel == id, None, None, cx)
          .on_click(cx.listener(move |this, _, _, cx| { this.sel = id; cx.notify(); }))
          .into_any_element());
  }
  ```
- **`size_full` on a resizable group** (E0599): the group has no styling; wrap it.
  `div().size_full().child(h_resizable("split").child(resizable_panel().child(a)).child(resizable_panel().child(b)))`
- **`.into()` inside a tuple** (E0283, "type annotations needed"): name the types.
  `let (title, right): (SharedString, AnyElement) = ("Settings".into(), div().into_any_element());`
- **A `src/` module named like a crate** (E0659 "ambiguous"): `src/thread/` (the open thread on
  screen) and the `thread` crate (`crates/thread`, its rows) share a name. In Bench write
  `::thread::Row` for the crate and `crate::thread::…` for the module.

## A `Button` is not a row of text

Both cost an agent 5–10 steps of reading `gpui-component-0.8.0` in the cargo registry:
- **Its label's font comes from its size, not from the parent.** `.small()` sets the label's
  size inside the button (`button_text_size`), so a `.text_xs()` on the bar or on the button
  itself changes nothing. A label of another size is a child: `Button::new(id).ghost().small()
  .child(div().text_xs().child(label))` (the status bar, `src/statusbar/mod.rs`).
- **Its label is one line** (`whitespace_nowrap` + `text_ellipsis`) and the button is as wide
  as it: a long label pushes it past its card. Text that must wrap (a choice of the agent's
  question) is a row, not a button: `choice_row(id, cx).child(text)` (`crates/ui/src/list.rs`).

## Layout helpers

- Resizable split: `h_resizable("id").child(resizable_panel().size(px(280.)).size_range(px(200.)..px(480.)).child(..))`.
- `div().truncate()` = one line + ellipsis; needs a width constraint: put it in a
  `flex_1().min_w_0()` (or `flex_shrink(1.)`) child of an `h_flex`.
- `div().absolute().bottom_full()` on a child of a `.relative()` parent places it right above
  the parent; `.top(px(…))` under it (the `/` list, `ui-overlays.md`).
- Hover reveal: parent `.group(name)`, child `.opacity(0.).group_hover(name, |s| s.opacity(1.))`.
  **Group names must be unique per row** (`SharedString::from(format!("row-{i}"))`).
- `.when(cond, |d| …)`, `.when_some(opt, |d, v| …)`, `.map(|d| …)`, `.children(option)`.

## Scrolling

- **Only the thread scrolls.** It is a `gpui::list` (`ListState`, `src/thread/mod.rs`), not
  a scrolling `v_flex`: rows that changed are `remeasure_items`d in place; the count changes
  by `splice` at the end only — splicing a range in the middle resets the offset inside it (the
  jitter). Never ask the list (`viewport_bounds`, `bounds_for_item`) inside `render_item`: it is
  borrowed; work it out before layout. Nothing scrolls it but the person
  (`build-thread.md`). An editor or a list inside it is as tall as its content.
- **A scrolling `v_flex` squeezes its children** once they're taller than it: rows and headers
  shrink, text overlaps its rules, the steps between sections turn uneven. Every child of a
  long list (row, section header, spacer) needs `.flex_none()`. Short lists hide the bug; it
  shows only when the data grows (Tasks → Upcoming).

- A scroll column elsewhere (the sidebar, a popover's list):
  `v_flex().id("x").overflow_y_scroll().track_scroll(&handle)`; `ScrollHandle`: `offset()`,
  `max_offset()`, `scroll_to_item(ix)`, `top_item()`.
- **Such a column scrolls together with every other scrollable under the pointer** — gpui's
  `track_scroll` listener runs in the Bubble phase for *all* hitboxes the wheel passes, so an
  inner list over the thread's `gpui::list` moves both. Where the wheel belongs to the inner
  one, dress the scroller in `gpui_base::ScrollableMask::new(Axis::Vertical, &handle)`: it takes
  axis-dominant wheels in the Capture phase and stops them, giving the wheel to the thread only
  at the pane's own edge (CSS `overscroll-behavior: auto`). The mask is a **sibling** of the
  scrolled element sized to its viewport (a child would slide off with the scroll offset):
  `div().relative().h(px(h)).child(scroller.absolute().inset_0()…).child(ScrollableMask::new(…))`
  — `inserts::scroll_pane` is that helper, `slash_body` the popup's copy of it.
  Never hand-write the wheel (`on_scroll_wheel` + `set_offset`): that is a second scroller.
- **A scroll container ignores the last child's margin** → append a real spacer
  (`div().h_3().flex_none()`).
- The thread's scrollbar is a Mac's overlay one (`gpui_kit::component::scroll::Scrollbar`); don't add others.

## Keyboard

- Intercept keys before a text field consumes them: `.capture_key_down(listener)` on an
  ancestor; check `ev.keystroke.key` ("up", "down", "tab", "escape") and
  `!ev.keystroke.modifiers.modified()`, then `cx.stop_propagation()`.
- Enter in a textarea arrives as `InputEvent::PressEnter`; decide there whether it picks a popup
  row or sends.

## Theme and appearance

- Switch: `Theme::change(ThemeMode::Dark|Light, None, cx)` then `Theme::update(cx, |t| …)`.
- Follow macOS: `cx.observe_window_appearance(window, |this, window, cx| …)` and re-apply when the
  mode is System; resolve with `window.appearance()` (`Dark | VibrantDark`).
- Root sets `window.set_rem_size(theme.font_size)` — changing font size rescales rem spacing.

## Images and GPU memory

- `img(path)` with only max sizes distorts (height stays intrinsic) — set `w` and `h` from the
  real size (`imagesize` crate) scaled to fit.
- Images you create (`Arc<RenderImage>`, e.g. video frames) stay in the GPU atlas until
  `cx.drop_image(img, None)` (outside a window update) or `cx.drop_image(img, Some(window))`.

## Markdown extensions

`TextView::markdown(id, text).plugin(P)` where `P: MarkdownPlugin` (`name`, `parse(&mdast::Node)`
→ `MarkdownNode::new(name, data).text(..)`, `render`; `is_block()` for block nodes). The parser
already recognizes `$…$` (`Node::InlineMath`) and `$$…$$` (`Node::Math`). mdast types:
`gpui_kit::component::text::markdown_ast`. Plugins rebuilt every frame are fine (no re-parse).

## Platform

- Folder picker: `cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(..) })` → `oneshot` receiver.
- Clipboard: `cx.write_to_clipboard(ClipboardItem::new_string(text))`.
- Open URLs/apps: `std::process::Command::new("open")` (macOS); for Zed use the `zed` CLI
  (`ssh://host/path` for remote files) and pass the project folder first so Zed opens the repo.

## Mistakes we made (don't repeat)

- **Slicing a string by bytes aborts the app.** `&name[name.len() - 3..]` to take "⌘3" off
  "Tools  ⌘3" cut through `⌘` (3 bytes in UTF-8): a panic, and a GPUI app panicking on the UI
  thread is gone (SIGABRT, "slice_error_fail" in the crash report). It fired on hovering the
  lit-up Tools toggle. Labels hold `⌘ ⇧ ⌥ — …` and Cyrillic: split at a known separator
  (`rsplit_once("  ")`), or use `chars()`; never `[a..b]` on text that isn't ASCII by
  construction. A crash leaves a report: `ls -t ~/Library/Logs/DiagnosticReports | grep <crate>`
  → read the crashed thread's frames; the first frame in your crate is the place.

- **A periodic re-read replaced the whole list** and dropped a new, not-yet-saved item: its card
  closed by itself a few seconds after "New task". Keep local-only items across re-reads
  (`build-insert.md`, "Patterns for a body").
- **A bar sized by its contents**: round buttons made a bar taller than the header next to it —
  the person saw it "bulge". Size the contents to the bar.
- **A 1px step between facing bars**: one drew its rule *on top of* `bar_h`, its neighbour was
  exactly `bar_h`. Count the rules in the heights (`bar_h + 1`).
- **A wider padding broke a fixed width**: raising the task card's padding 16 → 24 pushed its
  segmented control into the edge. When an inset grows, widen fixed-width boxes by the same.

- Copying a widget's look 1:1 from another app (a fixed-width command popup) — rebuild it in
  this app's grid instead (full chat width, attached to its anchor).
- Changing an element's size on hover/selection — change color only.
- Letting a selected-state color override a user-chosen color.
- Showing an empty dropdown before data arrives.
- Loading content sizes in the background so the thread jumped after opening.
- Rendering diagrams/formulas through headless Chrome (rejected: too heavy) — see `references/build-media.md`.
- Telling the person it's done after building only `target/debug`: they run
  `/Applications/Bench.app`, rebuilt by `ship.sh` when they ask.
- Hard-coded `px()` heights next to rem-based controls (they drift apart at other font sizes).

## Inputs

- `Input` height comes from its size (Medium = 2rem, Small = 1.5rem, Large = 2.75rem);
  `.h(..)` doesn't change the inner box. The textarea pads its text ≈10.5px itself — account for
  it when aligning text with other elements.
- `set_value` emits no `InputEvent::Change`: after setting a value from code, call your
  recompute yourself.
- Change an editor's language at runtime: `set_highlighter(lang, cx)`.
