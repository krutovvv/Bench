# The look: the rules behind every view

Bench looks like a Mac app — Xcode, Finder, Mail, Messages, System Settings — built from
`bench_ui` (`crates/ui`). The person settled it on 2026-10-28 as final: keep it, extend it in
the same voice, and when a new piece is needed, copy Apple's concrete values (sizes, weights,
radii) rather than inventing them. Zed stays the reference for how the window is organised
(workspace, docks, panels, items — `zed-as-reference`), not for how it looks.

1. **One look, two halves.** `BENCH_LIGHT` and `BENCH_DARK` (`crates/ui/src/theme/palettes.rs`):
   white pages on a light grey chrome (sidebar and bars `#F6F6F7`), hairlines a shade off the
   page (`#E2E3E7`), text `#1B1C1F`, muted `#5D6068`; the dark half the same steps in graphite.
   Light or dark follows macOS by default (Appearance → Mode: Light · Dark · System). No theme,
   no accent, no size to choose: one accent, Bench's blue (`#2F6BED` / `#5B9BFF`).
2. **One accent; red only for errors.** The accent marks the live, the chosen and the primary:
   Send with words, a selected completion, a chosen navigator glyph, the focused field. Red is
   errors only; amber is "waiting" (a question). A mark is a Finder tag: an 8pt dot
   (`bench_ui::mark`), never a tinted row.
3. **Type as macOS sets it.** The system font (SF), Menlo for code.
   - The thread's words 14 (the rem); its meta (work line, steps, cards' heads) 13
     (`THREAD_META`).
   - The sidebar's and the tree's rows 13 (`SIDEBAR_TEXT`); group heads 11 semibold muted, in
     their own case (`SIDEBAR_HEAD_TEXT`) — never capitals.
   - Bars 12 (`BAR_TEXT`), the window's title 13 semibold (`BAR_TITLE`).
   - Controls 13: segmented, form actions, completions, popovers.
   - Code 12 at Zed's leading (`CODE_LEAD` 1.618); the thread's prose at `LEAD` 1.5.
   Hierarchy is size and weight in Apple's steps (13 / 11 semibold), plus colour: muted, then
   `bench_ui::faint` for the quietest.
4. **Every size is a `bench_ui` name** (`crates/ui/src/metrics.rs`). No `px(…)` written in a
   screen; a missing size goes there first, named for what it is.
5. **Corners by kind, as Apple has them.** Fields, buttons, rows, the sidebar's search: the
   theme radius (`rounded_md`, 6). Cards and grouped boxes (blocks, a work's steps, popovers,
   the completion list): `rounded_lg`. The composer 14 (`COMPOSER_CARD_R`), the person's
   bubble 16 (`BUBBLE_R`), round buttons and dots full. A table is rounded only outside.
6. **Bars and rules line up.** The title bar spans the window (`TITLE_BAR_H` 40): traffic
   lights, the sidebar's fold, a short rule, the title, Chat | Code at the right end. The status
   bar is `STATUS_H` 29. Bars facing each other share a height and their rules are one pixel row:
   the Explorer's bar and the documents' tabs (`bar_h`), the IDE's heads.
7. **Controls are Mac controls.** A choice of a few is a segmented control (`segmented`: a grey
   track, the chosen raised on white with a soft shadow); on/off is a switch; a project to pick
   is a pull-down (name + chevron); a list of lists is a navigator's glyphs. Popovers and the
   menu bar are macOS's (`app/menu.rs`: Bench, File, Edit, View, Thread, Window, Help).
8. **Lists are source lists.** One-line rows, set in from the edges and rounded; the selected
   row's ground is `sidebar_accent`, hover `list_hover`; text keeps its colour. A grouped box
   (a soft `foreground` 3% ground, no rule) for lists inside the thread.
9. **Hairlines, not frames.** Separation is a 1pt rule at `border` (inside cards `border` 70%);
   no double rules, no rule where a ground already separates. Shadows only where macOS has them:
   a raised segment, a popover, the completion list, the Jump pill.
10. **Only the thread scrolls** (the terminal and editors excepted), with a Mac's thin overlay
    scrollbar. No scroll inside a scroll: lists are as long as they are, an empty state is one
    line.
11. **Nothing jumps.** Lay content out at its final size in the first frame; no placeholder
    values before real data.
12. **Quiet affordances.** Row actions on hover ("⋯", "+"), copy and read-aloud on an answer;
    tooltips name the shortcut; every shortcut also lives in the menu bar.
13. **No dialogs for simple choices.** Arm-then-confirm for deletes, rename in place, sensible
    defaults.
14. **English UI**, short labels in sentence case ("New project", "Bring All to Front" only
    where macOS itself title-cases a menu).
15. **Lightweight only.** No browser engines, WebViews or JS runtimes inside the app; slow work
    only on request.

## In short, each with its values

- **Colours.** One look, two halves: `BENCH_LIGHT` and `BENCH_DARK` in
  `crates/ui/src/theme/palettes.rs` plus their syntax JSON (`assets/themes/bench-*.json`, checked
  against the palette by `cargo test -p bench_ui`). Light or dark is the person's Mode, macOS's by
  default; there is no theme or accent to choose (one accent, Bench's blue). A thread's or a
  project's mark is a Finder tag dot (`bench_ui::mark`).
- **Type.** The thread's words 14, its meta 13; sidebar rows 13, group heads 11 semibold in
  their own case; bars 12, the title 13 semibold; controls 13; code Menlo 12 at Zed's leading.
- **Controls.** Mac ones: segmented controls, switches, pull-downs, navigator glyphs, a
  completion list for `/`, the menu bar with every shortcut (`src/app/menu.rs`).
- **The thread.** A classic chat: the person's grey bubbles, no avatars, a centred column,
  the composer pinned as one rounded field (plus, text, mic, Send). The document look stays in
  code (`src/thread/look.rs`).
- **Blocks.** Grouped boxes: the page's ground, a soft hairline, a card's corner
  (`block_frame`); every block head is `block_head`. Fields are `FIELD_H` (28).
- **Scrolling.** Only the thread scrolls, with a Mac's overlay scrollbar. No scroll inside a
  scroll, except the terminal and editors. Lists are as long as they are. Empty states take one
  line.
- **One place for each change.** A change to how blocks look goes to all of them at once.
