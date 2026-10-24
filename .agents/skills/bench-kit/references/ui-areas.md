# The window's areas: what goes where, moving things, fitting sizes

Every Bench window has the same areas. Most UI requests are one of three kinds:
- **put** something in an area ("add a button for X");
- **move** it to another area ("move the Tools icon up over the sidebar");
- **fit** one piece to another ("the code header as tall as Changed files", "the border 1px off").

This page gives each area's exact numbers and a fixed procedure for each kind of request.
Follow the procedure step by step even when the change looks trivial: the person checks the
result to the pixel. The change cycle itself (build, screenshot, measure, ship) is in
`build-change-loop.md`.

## The map (numbers at the default 14px font; the screenshot is Retina, 2 px = 1 pt)

```
x: 0                         280 (sidebar, resizable 200–480)                        1240
y ┌──────────────────────────┬─────────────────────────────────────────────────────────┐
0 │ ● ● ●      TOP STRIP 🔧 ☰ │ HEADER: title · hint · [segmented] ……… [icon toggles]   │ 0–35
35│━━━━━━━━━━━━━━━━━━━━━━━━━━│━━━━━━━━━━━━━━━━━━━━━━━━━ (one line across) ━━━━━━━━━━━━━│ 35 rule
  │ 🔍 SEARCH BAND           │ [STRIP under the header: tags / tabs, if the mode has one]│ 36–70
70│━━━━━━━━━━━━━━━━━━━━━━━━━━│━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│ 70 rule
  │ GROUP HEADER        ˅ +  │ BODY: the mode's content                                │
  │ │ row                    │   chat: question band ─ answer ─ cards under the answer │
  │ │ row          (hover …) │   lists, two panes, an editor, a card on the right      │
  │                          │                                                         │
736│██ BOTTOM BAR: + New ██████│━━━━━━━━ COMPOSER (Message …)  ⊙ ⊙ ⊙ ━━━━━━━━━━━━━━━━━━│ 736 edge = rule
772│━━━━━━━━━━━━━━━━━━━━━━━━━━┷━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│ 772 rule
  │ STATUS BAR: facts …………………………………………………………… pickers · ⧉ New window · ⚙ Settings │ 773–800
  └────────────────────────────────────────────────────────────────────────────────────┘
```

| Area | Height | Inside | Inset | Code (Bench) |
|---|---|---|---|---|
| Top strip (over the sidebar) | 35 | mode toggles, right-aligned, Tools then Tasks | right 16 (`pr 12` + the toggle's `p_1`) | `render_sidebar_top`, `ui/mod.rs` |
| Search band | 36 incl. 2 rules (`bar_h + 1`) | the search input, no frame | magnifier over the group icons (`pl 3.5`, measured) | `search_band`, `ui/mod.rs` |
| Sidebar list | rest | group headers, rows | 16; nested rows `nest_pl` | `ui/sidebar.rs`, `ui/servers.rs`, `ui/tools.rs`, `ui/tasks.rs` |
| Bottom bar | 36 (`bar_h + 1`) | the one primary button, filling it | — | `ui/sidebar.rs` (end) |
| Header | 36 incl. its rule | title, a muted hint, choices (segmented), icon toggles on the right | left 16 (`pl_4`), right 8 (`pr_2`) | `render_header`, `ui/mod.rs`; per mode `render_<mode>_header` |
| Strip under the header | 35 (`min_h(bar_h)`), its rule meets the search band's | tag pills, tabs | 24 (`px_6`) in content, 16 for tabs | `ui/tasks.rs` (tags) |
| Body | rest | the mode's content | 24 (`px_6`) around reading content | per mode |
| Cards under an answer | — | Changed files, processes, code, audio, video | the answer's 24 | `ui/chat.rs`, `ui/media.rs` |
| Composer | 36 (rule + `min_h(bar_h)`) | text, then 26pt round buttons | text at 24, buttons 24 from the edge | `ui/composer.rs` |
| Status bar | 28 incl. its top rule | left: facts; right: pickers, New window, Settings | items `px_2` | `ui/statusbar.rs` |
| Right panel (IDE, files) | full height, 640 (360–1400) | tabs 36, viewer, a file list | 16 | `ui/files.rs`, `ui/mod.rs` |

`bar_h` = `font_size × 2.5` (35). Every bar is the header's 36 with its rule; every rule that
crosses the window is one pixel row. The status bar is the only shorter bar (28).

## Which area a thing belongs to

| The thing | Area | Never |
|---|---|---|
| A switch between modes (Tools, Tasks) | top strip, right; the same strip on every mode's sidebar | the status bar |
| The SSH mode switch (changes the whole window) | header, right end, last | — |
| Window-level buttons (New window, Settings) | status bar, right end, Settings last | the header |
| Facts (where, who, Ln/Col, counts) | status bar, left; text, not buttons | — |
| A live choice (model, effort, approvals) | status bar, right, a picker; hidden until loaded | a default pretending to be real |
| Choices of the open item (Rendered/HTML, a time range) | header, as a segmented control | — |
| Frequent actions on the open item | header, right, icon toggles with tooltip + shortcut | — |
| Rare actions on an item | its row's "⋯" menu (shown on hover) | the header |
| Quick actions on a row (delete, pin) | on the row, shown on hover | always visible |
| Actions on an answer (copy, read aloud, regenerate, time) | over the answer's top-right corner, on hover | a row of their own |
| Results that come with an answer (files, processes, code) | a card under the answer: 1px frame, no fill, a header line | a dialog |
| The list's main action (New session, Open, New task) | the bottom bar, the one filled button | the header |
| Search over the list | the search band | inside the list |
| Filters of the main list (tags) | the strip under the header, as tall as the search band | the sidebar |
| A form (new server, settings of an item) | the body, in place (no modal) | a window or dialog |
| A choice to make before starting ("Name a group or Select folder") | the empty state, centered, on one line | a separate screen |

If the person names an area ("over the sidebar", "in the bottom corner"), that wins: do it,
then make sure the area's other rules still hold (same strip in every mode, insets).

## Moving a control from one area to another

The real case: "Put the Tools icon in the sidebar's top strip, Tools before Tasks; the bottom
right corner keeps only New window and Settings."

1. **Find where it is now.** `grep -rn '"Tools  ⌘3"' src/`: it's the tooltip, the most unique
   string. Note the handler it calls (`toggle_tools`) and any flag around it
   (`.when(crate::bench::TOOLS, …)`).
2. **Find the target area's builder.** It's in the map above (`render_sidebar_top`). Check that
   **every variant of the area** uses it: here, every mode's sidebar starts with
   `.child(self.render_sidebar_top(cx))` (`grep -rn render_sidebar_top src/ui`). If one mode's
   sidebar draws its own top, the icon will jump when the mode changes. Make that sidebar use the
   builder too.
3. **Rebuild the control as the target area's piece, not as a copy of the old one.** A status
   bar button (full bar height, `xsmall` icon) becomes a mode toggle (24pt square, `small`
   icon, `p_1`, accent on `list_active` while on, tooltip "Tools  ⌘3" / "Back  ⌘3"). Where the
   piece is: in an app from `new-app.sh`, `kit::mode_toggle` / `kit::status_button` /
   `kit::icon_button`; in a Bench from `new-bench.sh`, the `toggle` closure inside
   `render_sidebar_top` (`src/ui/mod.rs`) and `button` inside `render_statusbar`. Something that
   isn't a mode (New window: it opens, it doesn't switch) uses the same square with `on` always
   false, so it never lights up and its tooltip never says "Back".
4. **Order and spacing in the target**: as asked ("Tools, then Tasks"), the area's own gap
   (`gap_1`), and its inset (the last icon's edge 16pt from the sidebar's right edge).
5. **Delete the old one completely.** Remove the element, any now-unused imports and
   variables, and fix every comment and doc line that names the old place (`src/bench.rs` said
   "the wrench in the status bar" — stale comments mislead the next agent).
6. **Check what moved with it**: the shortcut (⌘3) still works, the flag still hides it, the
   tooltip is right, the active state lights up.
7. **Measure** (`build-change-loop.md`): a shot of your own instance before and after;
   `spans.py shot.png 8 28 0 280` lists the strip's icons with their positions and the gap to
   the sidebar's edge (the last gap must stay 16, the icons that were there must not move);
   `spans.py` on the old area shows it's gone. Crop both areas and look at them. Switch modes with a hook and shoot each one: the toggles must not move.

Other moves done the same way:
- **New window** from the header to the status bar, before Settings.
- **An answer's hover actions** from a row under the answer to an overlay on its top-right
  corner (`absolute().top(23).right_6()`, on the chat background so the text under it is
  covered cleanly). They're on the line of the steps' summary when there is one.
- **"Open files"** from the row's "⋯" menu, additionally, to an icon in the header, shown only
  while there are files to open. Kept in the menu too, because the person asked for both.

## Fitting one piece to another

The real case: "The code block's header must be as tall as the Changed files line." Others:
- the search band as tall as the header;
- the tag strip's rule on the search band's rule;
- the composer's rule on the bottom button's edge;
- the composer's text at the chat's inset.

1. **Name the reference piece and find its code.** Changed files' line is `h_flex().px_2()
   .py_1().text_xs()` in `ui/chat.rs`. That is the target. "Similar" is not enough: take its
   exact padding and text size.
2. **Copy those values to the piece you fit.** The code header became `.py_1()` + `.text_xs()`
   instead of a fixed `h(px(30.))`.
3. **Check every child that can make it taller.** The Copy button had `py_0p5`: 2pt more than
   the reference line. Its vertical padding went. A row is as tall as its tallest child plus
   the row's padding.
4. **Measure both on one screenshot.** `rules.py` down a column that crosses each piece. The
   rule-to-rule distances must be equal (here both 25pt, from 851→901 and 1371→1421 px).
5. **If they're bars facing each other, count the rules.** The composer draws its top rule on
   top of `min_h(bar_h)`, so the bottom bar next to it must be `bar_h + 1`. A bottom bar of
   exactly `bar_h` was a visible 1px step.
6. **For insets, measure text starts, not code.** `textx.py` from the divider for each band.
   Glyphs differ by about 0.5pt; anything more is a real offset. Fix it by the measured number
   (the composer's text needs `pl(11)` because the textarea pads itself about 13pt).

## Placing an icon exactly

- An icon's visible edge = the container's padding + the icon box's own padding. To put a
  toggle's edge at the sidebar's 16pt inset: `pr(12)` on the strip + `p_1` (4pt) on the toggle.
- Inputs pad themselves (about 10pt) and glyphs have side bearings. The search magnifier sits
  over the group icons with `pl(3.5)`: a measured number, not a guess. Measure with `textx.py`
  on the search band and on a group header; the x values must match.
- Icon sizes are fixed per place (`design-icons.md`): `xsmall` in the status bar and rows,
  `small` for toggles, 13pt in the composer's circles. A size between two of these is not
  allowed: pick the place's size.
- Centered things (the recording waveform) get equal gaps on both sides. Measure left and right
  with `measure.py row`.

## When a change makes something else not fit

- **A wider inset**: every fixed-width box in that area must grow by the same amount (the task
  card went 380 → 404 when its padding went 16 → 24; its segmented control had run into the
  edge).
- **A smaller bar**: shrink what sits inside it (the composer's circles 30 → 26, icons 15 → 13,
  the waveform 32 → 26). Never let the contents set the bar's height.
- **A field sized to its placeholder**: measure. At 110pt "Name a group" was clipped; at 120
  there was a gap after it; 118 fits.
- **A list that grows past the window**: every child needs `.flex_none()`, or the rows get
  squeezed and the steps between them turn uneven.
- After any change, shoot the **other modes** too: shared pieces (the top strip, the bars) are
  in every mode.
