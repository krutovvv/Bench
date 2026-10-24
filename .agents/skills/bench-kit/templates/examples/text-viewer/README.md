# Text viewer: the "just a text editor" app, finished

The request "a minimal text editor: open a file, line numbers, highlighting, look, close — no
chat" (the one a weaker model once spent two hours on) built from `scripts/new-app.sh` and the
kit's recipes. Compiled with zero warnings, run, screenshot: `text-viewer.png`.

```bash
~/Projects/bench-kit/.agents/skills/bench-kit/scripts/new-app.sh text-view "Text View"
cd ~/Projects/text-view
E=~/Projects/bench-kit/.agents/skills/bench-kit/templates/examples/text-viewer/src
cp $E/app.rs $E/lang.rs $E/main.rs src/
cp $E/ui/mod.rs $E/ui/sidebar.rs $E/ui/statusbar.rs src/ui/
cargo build && ./target/debug/text-view src/app.rs
```
(The files were made for the crate name `text-view`: `main.rs` has the window title "Text View";
for another name, take the template's own `main.rs` and add `mod lang;` to it.)

What changed against the template, file by file (about 150 lines, no new crates):
- `src/app.rs` — the template's `Item` list became **Recent files** (`Vec<PathBuf>` in
  `recent.json`, newest first, 30 at most, only files that opened); added `editor:
  Entity<EditorState>` (line numbers, no soft wrap, ⌘F) and `file`; `load_file` (first 2 MB,
  highlighter by name or `#!` line), `open_file_dialog`, `forget`; a file given on the command
  line opens at start. The editor's events repaint the status bar.
- `src/lang.rs` — Bench's highlighter choice (`language_for_text`), copied as is.
- `src/ui/mod.rs` — the header shows the file's name; the body is the `Editor` without its own
  border, or `kit::empty_state("Open a file to start")`.
- `src/ui/sidebar.rs` — rows are recent files (`kit::list_row`: name, its folder as the meta
  line, the open one selected, "×" removes it from the list); the bottom button is **Open**.
- `src/ui/statusbar.rs` — "Ln 12, Col 4" from `editor.read(cx).cursor_position()` (0-based).

Kept as they are: the frame, settings (theme, accent, size), the search band (it filters
Recent), the look. Not added because not asked: saving (one line when asked:
`std::fs::write(path, self.editor.read(cx).value().as_bytes())` + ⌘S in a key handler), tabs,
a file tree (that is Bench's mini IDE), the agent (see `build-new-app.md`, "Later they want
the agent").
