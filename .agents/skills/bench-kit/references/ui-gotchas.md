# Gotchas: what went wrong before, and the fix

Read when something behaves oddly or doesn't compile. Paths: widgets `gpui_kit::component::*`,
core `gpui_kit::*`. One exact name you can't find: hover it with the `rust_lsp` tools
(`build-hermes-tools.md`) — don't read the framework's sources to learn it.

## Compile errors that already happened (and the fix)

- **A closure that outlives the loop** (E0597, "borrowed value does not live long enough"):
  copy what the handler needs out of the item first, then `move` that copy.
  ```rust
  for (k, t) in self.tools.iter().enumerate() {
      let id = t.id;                       // a Copy value, or .clone() a String
      rows.push(kit::list_row(format!("t-{k}"), t.name.clone(), None, self.sel == id, None,
          None, cx, Box::new(cx.listener(move |this, _, _, cx| { this.sel = id; cx.notify(); })))
          .into_any_element());
  }
  ```
- **`size_full` on a resizable group** (E0599): the group has no styling; wrap it.
  `div().size_full().child(h_resizable("split").child(resizable_panel().child(a)).child(resizable_panel().child(b)))`
- **`.into()` inside a tuple** (E0283, "type annotations needed"): name the types.
  `let (title, right): (SharedString, AnyElement) = ("Settings".into(), div().into_any_element());`
- **A `ui/` module named like a crate module** (E0255/E0425): `src/tools.rs` and
  `src/ui/tools.rs` clash once both are imported. Name views after what they show:
  `src/ui/tool_view.rs`.
- **`kit::` not found** (E0433): every `src/ui/*.rs` that uses it needs `use super::kit;`.

## Layout helpers

- Resizable split: `h_resizable("id").child(resizable_panel().size(px(280.)).size_range(px(200.)..px(480.)).child(..))`.
- `div().truncate()` = one line + ellipsis; needs a width constraint: put it in a
  `flex_1().min_w_0()` (or `flex_shrink(1.)`) child of an `h_flex`.
- `div().absolute().bottom_full()` on a child of a `.relative()` parent places it right above
  the parent (popups over the composer). Padding on the absolute wrapper = side margins.
- Hover reveal: parent `.group(name)`, child `.opacity(0.).group_hover(name, |s| s.opacity(1.))`.
  **Group names must be unique per row** (`SharedString::from(format!("row-{i}"))`).
- `.when(cond, |d| …)`, `.when_some(opt, |d, v| …)`, `.map(|d| …)`, `.children(option)`.

## Scrolling

- **A scrolling `v_flex` squeezes its children** once they're taller than it: rows and headers
  shrink, text overlaps its rules, the steps between sections turn uneven. Every child of a
  long list (row, section header, spacer) needs `.flex_none()`. Short lists hide the bug; it
  shows only when the data grows (Tasks → Upcoming).

- Scroll column: `v_flex().id("x").overflow_y_scroll().track_scroll(&handle)`.
- `ScrollHandle`: `offset()`, `max_offset()`, `scroll_to_bottom()`, `scroll_to_item(ix)`,
  `scroll_to_top_of_item(ix)`, `top_item()`. Follow-the-tail chat: remember `max_offset` each
  render; if the user scrolled up (not at bottom and max unchanged) stop following; show a
  "Jump to latest" chip.
- **A scroll container ignores the last child's margin** → append a real spacer
  (`div().h_3().flex_none()`) when trailing cards end the content.
- Don't add visible scrollbars by design; use a rail/minimap if navigation is needed.

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
  (`build-modes.md`, section 8).
- **A bar sized by its contents**: 30px round buttons made the composer 44 tall next to a 36
  header — the user saw it "bulge". Size the contents to the bar.
- **A 1px step between facing bars**: the composer drew its top rule *on top of* `bar_h` while the
  sidebar's bottom bar was exactly `bar_h`. Count the rules in the heights (`bar_h + 1`).
- **A wider padding broke a fixed width**: raising the task card's padding 16 → 24 pushed its
  segmented control into the edge. When an inset grows, widen fixed-width boxes by the same.

- Copying a widget's look 1:1 from another app (a fixed-width command popup) — rebuild it in
  this app's grid instead (full chat width, attached to its anchor).
- Changing an element's size on hover/selection (rail ticks grew) — change color only.
- Letting a selected-state color override a user-chosen color.
- Showing an empty dropdown before data arrives.
- Loading content sizes in the background so the chat jumped after opening.
- Rendering diagrams/formulas through headless Chrome (rejected: too heavy) — see `references/build-media.md`.
- Telling the user it's done after building only `target/debug` (they run `dist/*.app`).
- Hard-coded `px()` heights next to rem-based controls (they drift apart at other font sizes).

## Inputs

- `Input` height comes from its size (Medium = 2rem, Small = 1.5rem, Large = 2.75rem);
  `.h(..)` doesn't change the inner box. The textarea pads its text ≈10.5px itself — account for
  it when aligning text with other elements.
- `set_value` emits no `InputEvent::Change`: after setting a value from code, call your
  recompute yourself.
- Change an editor's language at runtime: `set_highlighter(lang, cx)`.
