# Adding a mode to a Bench

A **mode** is a top-level place of the window with its own sidebar, header and main area:
Bench has Sessions (⌘1), SSH (⌘2), Tools (⌘3) and Tasks (⌘4). The status bar, the window frame
and the settings stay the same in every mode. The working example below is Bench's Tools mode —
`src/app/tools.rs` (state, actions), `src/ui/tools.rs` (views), `src/devtools/` (logic). Copy its
shape for any new mode ("CRM", "Assets", "Notes"…). A mode over data that lives in a native app
(Bench Tasks over Apple Reminders): the same shape plus section 8 and `build-native-data.md`.

## 1. The mode's state: one struct, its own fields and subscriptions

```rust
// src/app/<mode>.rs
pub struct Tools {
    pub on: bool,
    pub tool: Tool,                       // what the sidebar has selected
    pub search: Entity<InputState>,       // the sidebar's search
    pub query: String,
    pub input: Entity<EditorState>,       // the main area's controls
    pub output: Entity<EditorState>,
    _subscriptions: Vec<Subscription>,    // keep them alive with the struct
}

impl Tools {
    pub fn new(window: &mut Window, cx: &mut Context<HermesApp>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tools"));
        let input = cx.new(|cx| EditorState::new(window, cx).line_number(true).searchable(true));
        let output = cx.new(|cx| EditorState::new(window, cx).line_number(true).searchable(true));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, state, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.tools.query = state.read(cx).value().trim().to_lowercase();
                    cx.notify();
                }
            }),
            // Output follows the input as you type.
            cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::Change = ev {
                    this.run_tool(window, cx);
                }
            }),
        ];
        Self { on: false, tool: Tool::Base64, search, query: String::new(), input, output,
               _subscriptions: subscriptions }
    }
}
```

In the app struct: `pub tools: tools::Tools,`; in `new()`: `let tools = tools::Tools::new(window, cx);`
(next to the other modes') and `tools,` in `Self { … }`; `pub(crate) mod tools;` in `app.rs`.

Note: `set_value` on an input does NOT emit `InputEvent::Change` — after setting a value from
code, call the recompute yourself (`this.tool_input(text, window, cx)` does both).

## 2. Switching modes

```rust
pub fn toggle_tools(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.tools.on = !self.tools.on;
    self.panel = Panel::Chat;          // leave Settings if it was open
    cx.notify();
}
// Every other mode's switch turns this one off:
pub fn set_ssh_mode(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
    self.ssh.on = on;
    self.tools.on = false;
    // …
}
```

## 3. The root picks the mode's views

```rust
// sidebar (src/ui/sidebar.rs, first lines of render_sidebar)
if self.tools.on { return self.render_tools_sidebar(cx); }
if self.ssh.on { return self.render_ssh_sidebar(cx); }
// header (src/ui/mod.rs, render_header)
if self.tools.on && self.panel == Panel::Chat { return self.render_tools_header(cx); }
// body (render)
let body = match self.panel {
    Panel::Chat if self.tools.on => self.render_tools_body(cx),
    Panel::Chat if ssh => self.render_ssh_body(window, cx),
    // …
};
```
Hide panels that belong to another mode (Bench hides the IDE panel in Tools:
`.when(self.right.is_some() && !self.tools.on, …)`).

## 4. A toggle and a shortcut

The toggle goes in the sidebar's top strip, right of the window buttons — **not** the status
bar (the user moved them there: "the two utilities over the sidebar; the bottom-right corner
keeps only New window and Settings"). Bench `render_sidebar_top` in `src/ui/mod.rs`, the
template's `kit::sidebar_top` + `kit::mode_toggle`:
```rust
kit::mode_toggle("mode-tools", Ico::Wrench, "Tools  ⌘3", self.tools.on, cx,
    Box::new(cx.listener(|this, _, window, cx| this.toggle_tools(window, cx))))
```
Every mode's sidebar starts with that same strip (`.child(self.render_sidebar_top(cx))`), so
the toggles never move when the mode changes. Order = the shortcuts' order. Turning one mode
on turns the others off (`self.ssh.on = false; self.tasks.on = false;` in each toggle).

Shortcuts for the whole window: one `capture_key_down` on the root element sees every key first.
```rust
.capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
    if this.global_key(&ev.keystroke, window, cx) { cx.stop_propagation(); }
}))
// global_key: only ⌘-combinations; everything else goes on to the focused field.
let m = &ks.modifiers;
if !m.platform || m.control || m.alt { return false; }
match (ks.key.as_str(), m.shift) {
    ("3", false) => self.toggle_tools(window, cx),
    _ => return false,
}
true
```

## 5. The sidebar

Same structure as the template's: the top strip with the toggles, the search band
(`search_band`, 36 with its two rules), then a scrolling list of group headers (muted icon +
name) and rows (`nest_pl`, the guide line, the accent stripe when selected, `py_1` in compact
mode), clickable:
`.on_click(cx.listener(move |this, _, window, cx| this.select_tool(tool, window, cx)))`.
Copy `render_tools_sidebar` from Bench `src/ui/tools.rs`.

## 6. Main-area patterns

- **Header**: name (medium), a muted one-line hint (`flex_1().truncate()`), the mode's choices
  as a segmented control, then the usual header buttons.
- **Two panes side by side** — input | output. `h_flex` centers its children vertically: give it
  `.items_stretch()`, or panes shrink to their content.
  ```rust
  h_flex().size_full().items_stretch()
      .child(v_flex().flex_1().min_w_0().child(pane_bar(..)).child(
          div().flex_1().min_h_0().child(Editor::new(&input).appearance(false).h_full())))
      .child(div().w(px(1.)).h_full().bg(theme.border))
      .child(/* output pane, same shape */)
  ```
- **A pane's strip** (36px, title or an error in red, small square buttons on the right):
  `pane_bar`, `small_button`, `segmented` in Bench `src/ui/tools.rs`.
- **Rendered Markdown** in a pane: `TextView::markdown(id, text)` inside a
  `div().id(..).flex_1().min_h_0().overflow_y_scroll()`.
- **Rendered HTML** (a pasted web page, an e-mail): `TextView::html(id, html)` the same way;
  give the pane a `Rendered | HTML` segmented switch to show the code in an editor instead. When
  no editor is visible, a ⌘V for the pane goes through the window's `global_key`.
- **An image from bytes** (decoded Base64, a download): write it to a file in the app's state
  folder with a new name each time (the image cache keys on the path), delete the previous one,
  show `img(path).size_full().object_fit(ObjectFit::Contain)`.

## 7. Work that takes time, clipboard, files

```rust
// Off the UI thread, back with the result:
cx.spawn_in(window, async move |this, cx| {
    let html = cx.background_spawn(async move { devtools::clipboard_html() }).await;
    let _ = this.update_in(cx, |this, window, cx| this.tool_input(html.unwrap_or_default(), window, cx));
}).detach();

// Clipboard:
let text = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
cx.write_to_clipboard(ClipboardItem::new_string(text));

// Save as… (a new file):
let picked = cx.prompt_for_new_path(&downloads_dir, Some("image.png"));
cx.spawn_in(window, async move |_, _| {
    if let Ok(Ok(Some(dest))) = picked.await { let _ = std::fs::copy(&src, dest); }
}).detach();
```

Polling something that has no events (Bench polls the agent's background processes): one task
started in `new()`, `loop { timer(3 s).await; this.update(cx, |this, _| this.poll())?; }`,
ending when the update fails (the window closed). Errors of a poll go to the log, never a toast.

## 8. A list that is re-read from its source

When the mode shows data another program also changes (Reminders edited on the iPhone or by an
agent), re-read it now and then (Bench Tasks: every 4 s while the mode is on, from the window's
500 ms ticker) and replace the list. Three rules, each from a real bug:
- **Keep what exists only here.** A new item not yet saved (no title typed) is not in the
  source; replacing the list drops it and its open card closes "by itself" a few seconds after
  "New task". Before replacing, carry over the items with a temporary id (`new:<token>`); drop
  a blank one when the person leaves it (selects another, switches list).
- **Don't overwrite a card being typed into**: while it is dirty, keep its copy in place of the
  re-read one; save after 700 ms without typing, then re-read.
- **A long list scrolls instead of squeezing**: every row, section header and spacer in the
  scrolling `v_flex` gets `.flex_none()`. Without it GPUI shrinks the children to fit the
  height: headers get crushed and the steps between sections turn uneven ("24 Tomorrow" became
  a sliver once Upcoming grew past the window).
Bench `src/app/tasks.rs` (`on_reminders`, `tick_tasks`, `drop_blank_new`), `src/ui/tasks.rs`.


## 9. Checklist: every file a new mode touches in a Bench

Taken from adding Tasks (⌘4) to Bench. Do them in this order, `cargo build` after each line:
1. `src/<logic>.rs` — the data type and its pure functions, with tests (Tasks: `src/tasks.rs`:
   `Task`, lists, parsing of the notes lines). `mod <logic>;` in `main.rs`.
2. `src/<source>.rs` — reading and writing the native store on a worker thread (Tasks:
   `src/reminders.rs`). `mod` in `main.rs`; crates in `Cargo.toml` (`build-crates.md`).
3. `src/app/<mode>.rs` — the mode's state struct + `impl HermesApp { toggle_<mode>, select_…,
   … }`; `pub(crate) mod <mode>;` in `src/app.rs`; the field `pub <mode>: <mode>::<Mode>` in
   `HermesApp` and its `new()` in `HermesApp::new` (name the local binding `<mode>_mode` if a
   module of that name is in scope — `tasks` clashed once).
4. `src/ui/<mode>.rs` — `render_<mode>_sidebar`, `render_<mode>_header`, `render_<mode>_body`;
   `mod <mode>;` in `src/ui/mod.rs`. The sidebar starts with `.child(self.render_sidebar_top(cx))`.
5. Dispatch: top of `render_sidebar` (`src/ui/sidebar.rs`), `render_header` and the body `match`
   (`src/ui/mod.rs`); hide panels of other modes (`&& !self.<mode>.on`).
6. The toggle in `render_sidebar_top` (`src/ui/mod.rs`) behind `crate::bench::<MODE>`; the flag
   in `src/bench.rs`; every other mode's toggle sets `self.<mode>.on = false` (`app/servers.rs`
   `set_ssh_mode`, `app/tools.rs`, `app/install.rs`), and yours turns the others off.
7. Shortcuts in `src/app/keys.rs` `global_key`: the mode's ⌘<n>, and ⌘N / ⌘F / ⌘K meaning
   "new / search / done" while the mode is on.
8. Periodic work: call `self.tick_<mode>(cx)` from the window's 500 ms ticker in `app.rs`.
9. Icons: SVGs in `assets/icons/`, lines in `src/icons.rs`.
10. A macOS permission (Reminders, Calendar, Contacts): its usage strings in
    `scripts/bundle-macos.sh`'s Info.plist, then test from the bundled app (`build-verification.md`).
