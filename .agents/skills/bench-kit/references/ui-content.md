# Content: what the body shows

An insert's body (`build-insert.md`) or a card under it shows the thing being worked on. Pick the ready piece:

| The person asks for | Use | Section |
|---|---|---|
| Input → output (a converter, a generator) | two panes with strips | "Two panes" |
| A code / text editor or viewer, line numbers, highlighting, search, undo, several cursors | `EditorState` + `Editor` | "Editor" |
| Rendered Markdown | `TextView::markdown(id, text)` | "Text views" |
| A web page / pasted rich text as it looked | `TextView::html(id, html)` | "Text views" |
| Plain text that can be selected and copied (a message, a log) | Markdown of the escaped text | "Text views" |
| An image | `img(path)` | "Images" |
| Changes to a file | open it in Zed; counts from `src/thread/diff.rs` | "Diff" |
| A terminal | Bench `src/inserts/terminal/session.rs` + `view.rs`, whole | "Terminal" |
| Nothing selected yet | `bench_ui::empty_state` | "Empty state" |
| Images, video, audio, Mermaid, LaTeX, attachments, drag and drop | `build-media.md` | — |

Never write your own text rendering, text buffer or highlighter: a session that did spent
2 hours and 300 tool calls on a broken editor; the recipe below is ~60 lines that compile.

## Empty state

**`bench_ui::empty_state(text, cx)`** — one muted line: what to do to start ("Copy part of a web
page, then press ⌘V"). Never a blank body, never placeholder data.

## Two panes (input → output)

Each pane: a strip (**`bench_ui::pane_bar(title, error, buttons, cx)`** — a muted title, or the
error in red in its place; small buttons on the right; the /tools insert has its own on
`block_head`, `pane_bar` in `src/inserts/tools/view.rs`) over the content. Between panes a 1px
line. `h_flex` centers its children vertically: give it `.items_stretch()` or the panes shrink
to their content.

```rust
let left = v_flex().flex_1().min_w_0()
    .child(bench_ui::pane_bar("Input", None, vec![
        bench_ui::compact_button("paste", "Paste").on_click(cx.listener(|this, _, w, cx| this.paste(w, cx))).into_any_element(),
        bench_ui::compact_button("clear", "Clear").on_click(cx.listener(|this, _, w, cx| this.clear(w, cx))).into_any_element(),
    ], cx))
    .child(div().flex_1().min_h_0().child(Editor::new(&self.input).appearance(false).h_full()));
let right = v_flex().flex_1().min_w_0()
    .child(bench_ui::pane_bar("Output", self.error.clone(), vec![
        bench_ui::compact_button("copy", "Copy").on_click(cx.listener(|this, _, _, cx| this.copy(cx))).into_any_element(),
    ], cx))
    .child(div().flex_1().min_h_0().child(Editor::new(&self.output).appearance(false).h_full()));
h_flex().size_full().items_stretch()
    .child(left)
    .child(div().w(px(1.)).h_full().bg(theme.border))
    .child(right)
```

The output follows the input as it is typed: subscribe to the input's `InputEvent::Change` and
recompute. Wrong input → the error in the output strip, the output empty — no toast. In a
thread the panes are as tall as their longer side (the insert's `body` in `src/inserts/tools/mod.rs`
from `editor_line`): the thread scrolls, not the panes. The full working example: the /tools
insert (`src/inserts/tools/{mod,state,view}.rs`, the utilities in `crates/devtools/src/`).

## Text views

- **Markdown**: `TextView::markdown(id, text)`, selectable out of the box (the Root draws the
  selection and binds ⌘C). Code blocks the size of the text: Bench `src/media/view.rs`
  `markdown()` (a `TextViewStyle` with a `code_block` refinement). Long texts are kept and
  parsed once (`ui::media::Texts`, `kept_markdown`) — never a new `TextView` per frame.
- **Code blocks with a header** (an agent's answer): the language on the left, **Copy** on the
  right, the code under it, one 1px frame, no fill. gpui-kit's own block only puts the language
  in its corner and lets `code_block_actions` float over the first line — no header. So split
  closed fences out of the answer before rendering (Bench `src/media/mod.rs` `segments()`,
  `src/media/view.rs` `answer_segments` → `Segment::Code { lang, code }`; an unclosed fence, still
  streaming, stays text) and draw each:
  ```rust
  v_flex().w_full().border_1().rounded_md().border_color(theme.border)
      .child(h_flex().py_1().pl_4().pr_2().items_center().justify_between()   // = Changed files' line
          .border_b_1().border_color(theme.border).text_xs().text_color(muted)
          .child(if lang.is_empty() { "code".into() } else { lang.clone() })
          .child(h_flex().id(("code-copy", k)).gap_1().px_1p5().items_center().cursor_pointer()
              .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
              .child(Ico::Copy.icon().xsmall()).child("Copy")
              .on_click(move |_, window, cx| {
                  cx.write_to_clipboard(ClipboardItem::new_string(code.trim_end().into()));
                  window.push_notification((NotificationType::Success, SharedString::from("Copied")), cx);
              })))
      .child(TextView::markdown(id, format!("```{lang}\n{code}```"))
          .code_block_actions(|_, _, _| div())          // any action hides the corner label
          .style(TextViewStyle::default().code_block(StyleRefinement::default()
              .text_size(rems(0.8125)).line_height(relative(1.5))
              .border_0().bg(transparent_black()))))    // no fill: the frame alone
  ```
  `window.push_notification` needs `use gpui_kit::component::WindowExt as _;`. Full code:
  Bench `src/media/view.rs` `render_code`.
- **HTML as it looked**: `TextView::html(id, html)` — headings, lists, links, bordered tables,
  images; `<script>`/`<style>` skipped. Bench shows a pasted web page with it
  (`render_rich_input` in `src/inserts/tools/view.rs`), with a `Rendered | HTML` segmented switch.
- **Plain text to select and copy** (a person's message, a log): not `SelectableText` (its
  copy needs keyboard focus, which stays in the composer) — a `TextView::markdown` of the
  escaped text: Bench `src/media/view.rs` `plain_text`.
- Import: `use gpui_kit::component::text::TextView;`.

## Images

`img(path).size_full().object_fit(ObjectFit::Contain)` in a box of known size. With only max
sizes an image distorts: set `w` and `h` from the real size (`imagesize::size(path)`) scaled to
fit. Bytes you made (decoded Base64, a download): write them to a new file in the app's state
folder each time (the image cache keys on the path), delete the previous one.

## Diff

Bench draws no diffs: "Changed files" under an answer lists the files the agent edited with
their +/− counts (`src/thread/diff.rs` `stats` over the tools' unified diffs; the card in
`src/thread/extras.rs` `render_session_files`), a click opens the file in Zed. If a diff
view is asked for: gpui-kit's `DiffFile::parse(patch)`, `DiffState::new(files, cx)`,
`Diff::new(&state)` — not a hand-drawn one.

## Terminal

Bench `src/inserts/terminal/session.rs` (a PTY with `portable-pty` + the VT parser
`alacritty_terminal`) and `view.rs` (the grid painted cell-exactly by a custom element); the
browser twin `session_web.rs`, `view_web.rs`. Extend them; don't write another.

**Paths in a terminal open in Zed** (as Warp does, without Warp): ⌘-hover underlines a link
(pointer cursor), ⌘-click opens it — a URL in the browser, a file or folder in Zed at its `:line`
(compiler errors, grep; a server's over Zed's SSH). Recognized: absolute, `~/…`,
relative (`src/main.rs`) and bare file names from `ls` (`wp-config.php`, `nginx.conf`; not
`1.5`). Relative ones are resolved against the terminal's folder: a local shell's own (`lsof -a
-p <pid> -d cwd -Fn`), a server's from the last prompt on screen (`user@host:/var/www$`,
`[user@host /etc]#`), else home. A server answers "file or folder" over ssh. Code:
`src/inserts/terminal/session.rs` `link_span`, `prompt_folder`, `cwd`, `hover_at`;
`src/inserts/terminal/servers.rs` `open_terminal_link`; `src/app/projects.rs` `open_in_zed`. Notes on the
custom element: `build-custom-render.md`.

## Editor

What `Editor` already does by itself, with no code from you: typing, selection with mouse and
keys, copy / cut / paste, undo ⌘Z / redo ⌘⇧Z, several cursors (⌘⌥↑ / ⌘⌥↓), word moves
(⌥← / ⌥→), indent ⌘] / outdent ⌘[, line numbers, indent guides; find and replace with
`.searchable(true)`. "Select next occurrence" (⌘D) is not built in.

Builder: `EditorState::new(window, cx).line_number(true).soft_wrap(false)`; `.folding(true)`
for fold arrows; `set_highlighter(lang, cx)` with `crate::lang::language_for(path)` /
`language_for_text` (`src/explorer/lang.rs`; only grammars enabled in `Cargo.toml`); `set_value`,
`value()`. In a thread an editor never scrolls inside: give it the height of its text,
`bench_ui::editor_height(lines, cx)` (`crates/ui/src/metrics.rs`), and let the thread scroll.

**The window sees ⌘ keys first.** `global_key` (`src/app/keys.rs`) is a `capture_key_down` at
the root: a ⌘ shortcut it matches never reaches the focused editor. ⌘F is one — it focuses the
sidebar's search everywhere, so an editor's own find bar is not opened by ⌘F today. To give a
key back to an editor, return `false` for it while the editor has focus. Never bind a bare
⌃-key there: the terminal needs them.

Limits to tell the person honestly (do not try to "fix" them by writing your own editor): the
editor is smooth up to a few MB of text. Open the first 2 MB of a bigger file and say the rest
is not shown. Anything heavier opens in Zed (the project's ⋯ menu, Open in Zed).

The finished editors in Bench are a project's environment texts: `section::code`
(`src/env/section.rs`) and Instructions' editor (`src/env/ide.rs`). Copy from them.

### "Open…" with the macOS dialog

```rust
pub fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let picked = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Open".into()),
    });
    cx.spawn_in(window, async move |this, cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        let _ = this.update_in(cx, |this, window, cx| this.load_file(path, window, cx));
    })
    .detach();
}
```
