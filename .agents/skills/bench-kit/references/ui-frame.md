# The frame: window, sidebar, header bar, status bar

Every Bench has this frame and keeps it. A new app gets it from `scripts/new-app.sh`; a new
mode fills the same three places (sidebar, header, body) with its own content
(`build-modes.md`). Never remove the sidebar, the header or the status bar.

Picture: `assets/gallery.png` (the template), `assets/ui/tools-mode.png` (Bench's Tools mode).

```
┌ sidebar 280px ─────────┬ main ──────────────────────────────────────────┐
│ (traffic lights)  🔧 ☰ │ header_bar 36px: title ........... icon_buttons │
│ search band (36)       │ [tab_strip 36px, if the mode has tabs]          │
│ ─────────────────────  │                                                 │
│ group_header           │ body: editor / Markdown / two panes / settings  │
│ │ list_row             │       / empty_state                             │
│ │ list_row             │                                                 │
│ group_header           │                                                 │
│ ─────────────────────  │                                                 │
│ primary_button (36)    │ composer (36)                                   │
├────────────────────────┴─────────────────────────────────────────────────┤
│ status bar 28px: facts .............................. pickers · ⧉ · ⚙ │
└──────────────────────────────────────────────────────────────────────────┘
```

**One height for every bar: the header's 36.** The search band, the bottom button, a composer,
a list's tag strip: all 36 with their rule, and every rule that crosses the window is one line
(the search band's top rule = the header's bottom rule; the bottom button's top edge = the
composer's top rule). The user checks this to the pixel.

The title bar is transparent: the sidebar starts with a 35px strip beside the traffic lights
(`kit::sidebar_top`) holding the **mode toggles** on its right (Bench: Tools 🔧, then Tasks ☰),
the header sits at the top of the main column. Resizable split:
`h_resizable("layout").child(resizable_panel().size(px(280.)).size_range(px(200.)..px(480.)).child(sidebar)).child(resizable_panel().child(main))`
(wrap it in a `div().flex_1().min_h_0()` — the group itself takes no styling).

## Sidebar: top strip, search band, list, one button at the bottom

![](../assets/ui/search.png)

The search and the button are **bands from edge to edge**, not controls floating in a padded
bar: the search has rules above and below and the content's background between, with no frame
of its own (it *is* the divider), its magnifier right over the group icons; the bottom button
fills its whole bar (no rule, no margin), its top edge level with the composer's rule. Every
mode's sidebar starts with the same top strip, so the toggles never jump when the mode changes.

```rust
v_flex().size_full().bg(theme.sidebar)
    .child(kit::sidebar_top(vec![                          // beside the window buttons, 35
        kit::mode_toggle("mode-tools", Ico::Wrench, "Tools  ⌘3", self.tools_on, cx,
            Box::new(cx.listener(|this, _, w, cx| this.toggle_tools(w, cx)))).into_any_element(),
    ]))
    .child(kit::search_band(&self.search, cx))             // 36 with its two rules
    .child(v_flex().id("list").flex_1().min_h_0().overflow_y_scroll().py_1().children(rows))
    .child(kit::bottom_bar(kit::primary_button("new", Ico::Plus, "New", cx,
        Box::new(cx.listener(|this, _, _, cx| this.new_item(cx)))), cx))   // bar_h + 1
```

`kit::sidebar_top`, `mode_toggle`, `search_band`, `bottom_bar` are in `templates/app/src/ui/kit.rs`
(Bench: `render_sidebar_top` and `search_band` in `src/ui/mod.rs`).

Search state: `InputState::new(window, cx).placeholder("Search …")`, filter on
`InputEvent::Change` (keep the lowercase query in a field, `cx.notify()`).

## Header bar

![](../assets/ui/header-bar.png)

**`kit::header_bar(title, right, cx)`** — 36px over the main area: the title (medium, truncates),
then `right`, an `h_flex().gap_1()` of `kit::icon_button`s (or `div().into_any_element()`).

```rust
let right = h_flex().gap_1()
    .child(kit::icon_button("pin", Ico::Pin, "Pin", self.pinned, cx,
        Box::new(cx.listener(|this, _, _, cx| { this.pinned = !this.pinned; cx.notify(); }))))
    .into_any_element();
kit::header_bar(self.title(), right, cx)
```

What goes in the header, in this order: the title; a muted one-line hint (`flex_1().truncate()`,
Bench Tools); the item's choices as a `segmented` control; icon toggles with a tooltip that
names the shortcut ("Open files  ⌘B"); window-level buttons last. Item-specific actions that are
rare go into the item's "⋯" menu, not the header.

## Status bar

![](../assets/ui/status-bar.png)
![](../assets/ui/bench-status-bar.png)

28px, `bg(theme.status_bar)`, `border_t_1`, `text_xs`, everything in one muted tone.
- Left: facts — where and who (`kit::status_text("This Mac")`, "Ln 3, Col 9", "12 items").
  Facts are not controls.
- Right: live readouts and pickers (`ui-forms.md`, "Picker"), then only the window's own
  buttons: New window, then Settings ⚙ last — each a
  `kit::status_button(id, icon, tip, active, cx, on_click)`. **Mode toggles don't go here**:
  they sit over the sidebar (`kit::mode_toggle`), where the user asked for them ("the two
  utilities over the sidebar; the corner keeps only New window and Settings").
- Nothing goes into the status bar that the person didn't ask for. Hide a picker until its data
  has loaded — never show "—" or a default pretending to be real.

```rust
h_flex().h(px(28.)).flex_none().w_full().bg(theme.status_bar)
    .border_t_1().border_color(theme.border).text_xs().text_color(muted)
    .child(kit::status_text(format!("{} items", self.items.len())))
    .child(div().flex_1())
    .child(kit::status_button("settings", Ico::Settings, "Settings", settings_on, cx,
        Box::new(cx.listener(|this, _, _, cx| this.toggle_settings(cx)))))
```

## Bars that face each other

**`kit::bar_h(cx)`** = `font_size × 2.5` = 35 at the 14px font: the header's 36 less the one
rule a bar draws. A bar with one rule is `bar_h + 1` (the bottom button, a composer with its
top rule, a "+ New item" row); the search band with two rules is `bar_h + 1` too; a strip under
the header (the Tasks tag strip) is `min_h(bar_h)`. Header 36px, status bar 28px, pane strips
36px. What sits inside a bar is sized to fit it, never the reverse: the composer's round buttons
are 26 (were 30 — they made the bar 44 and the user saw it "bulge"). Use rems next to rem-sized
controls: fixed `px` heights drift apart at the S/L font sizes.

## Modes

Several jobs with their own lists (chats, servers, tools) are **modes**: the frame stays, the
sidebar, header and body switch. A toggle in the sidebar's top strip and ⌘1/⌘2/⌘3/⌘4 switch
them. Recipe: `build-modes.md`.
