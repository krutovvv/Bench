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
x: 0                         280 (sidebar, resizable)                                   1240
y ┌──────────────────────────┬─────────────────────────────────────────────────────────┐
0 │ ● ● ●       TOP STRIP + ◧ │ HEADER: thread title · project chip ………………… ⧉ New window│ 0–35
35│━━━━━━━━━━━━━━━━━━━━━━━━━━│━━━━━━━━━━━━━━━━━━━━━━━━━ (one line across) ━━━━━━━━━━━━━│ 35 rule
  │ 🔍 SEARCH BAND           │ THE THREAD (gpui::list, the only scroll):              │
  │━━━━━━━━━━━━━━━━━━━━━━━━━━│   messages, "N actions", answers, inserts in the rows   │
  │ Host               ˅ +   │   where the person put them, cards under them           │
  │ │ thread                 │   tail: plan, processes, approval, a form, COMPOSER     │
  │ PROJECT            ˅ +   │   (the composer is the last row; room past the end)    │
  │ │ thread       (hover ⋯) │                                                         │
772│━━━━━━━━━━━━━━━━━━━━━━━━━━┷━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│ rule
  │ STATUS BAR: agent · model · context · speed · effort · approvals …… ▣ 📎 🎤 ⚙ Stop │ STATUS_H 29
  └────────────────────────────────────────────────────────────────────────────────────┘
```

| Area | Height | Inside | Inset | Code |
|---|---|---|---|---|
| Top strip (over the sidebar) | 35 | icon button, right-aligned: Hide sidebar (New project is the list's last row) | right 16 (`pr 12` + the icon's 4) | `render_sidebar_top`, `src/sidebar/top.rs` |
| Search band | `bar_h + 1` | the search input, no frame | magnifier over the head icons (measured) | `bench_ui::search_band` (`crates/ui/src/bars.rs`), used in `src/sidebar/mod.rs` |
| Sidebar list | rest | projects only: Host's threads first, then each project (pinned first) with its threads | nested rows `nest_pl` | `src/sidebar/mod.rs` (`render_loose`, `render_project`); order and acts `src/sidebar/items.rs` |
| Header | 36 incl. its rule | the thread's title, the project chip; at the right end New window | left 16 (`pl_4`), right 8 (`pr_2`) | `render_header`, `src/titlebar/mod.rs`; its right end `render_header_tail`, `src/inserts/terminal/form.rs` |
| The thread | rest | rows from `crates/thread`; inserts where the journal puts them; the tail | `CHAT_TEXT` / `CHAT_RIGHT`, rows `TURN_Y` | `render_chat`, `src/thread/mod.rs`; inserts `render_insert_by_id`, `src/inserts/mod.rs` |
| Cards under an answer / the tail | — | Changed files, sources, processes, approval, plan; code, audio, video | the answer's column | `src/thread/extras.rs`, `src/media/view.rs` |
| Composer | its text | text only, no buttons (⌘Enter sends); attachment chips over it | text at `CHAT_TEXT` | `src/thread/composer.rs` |
| Status bar | `STATUS_H` (29) incl. its top rule | left: the agent's state, model, context, speed, effort, approvals; right: background terminals, paperclip, mic, Settings gear, Stop last | items `px_2` | `src/statusbar/mod.rs` |

`bar_h` = `font_size × 2.5` (35). Every rule that crosses the window is one pixel row. The
status bar is the only shorter bar. The Settings panel (the gear) and the new project's page take
the thread's inset all around — `CHAT_LEFT` (28) from the divider, the window's right edge and the
bottom, and the same air over their tabs; their content fills that width, no max-width.

## Which area a thing belongs to

| The thing | Area | Never |
|---|---|---|
| A new tool (tasks, a server, files, a utility) | an insert put in with `/` (`build-insert.md`) | a sidebar group, a mode, a side panel |
| A form a tool needs (a new server, a new host) | an agent-message card under the insert that asked for it | a dialog, a settings page |
| A project's settings (name, where, model, Telegram) | its Settings page (`Center::Project`, `src/env/page.rs`) | a thread, a dialog |
| What a project contains (files, soul, prompts, skills, MCP, schedule) | the Explorer (`src/explorer/`, `docs/explorer.md`) | a thread, a settings tab |
| The sidebar's own buttons (hide) | top strip, right; New project: the list's last row | the status bar |
| New window | the header's right end (`render_header_tail`) | — |
| Settings (the gear), attach, dictate, Stop | status bar, right; Stop last | the composer |
| Facts (where, model, counts) | status bar, left; text or pickers, not buttons | — |
| A live choice (model, effort, approvals) | status bar, a picker; hidden until loaded | a default pretending to be real |
| Rare actions on a thread or project | its row's hover "⋯" and right-click (one `item_menu`) | the header |
| Quick actions on a row (a new thread "+") | on the row, shown on hover | always visible |
| Actions on an answer (copy, read aloud, regenerate) | over the answer's top-right corner, on hover | a row of their own |
| Results that come with an answer (files, processes, code) | a card under the answer: 1px frame, rounded, no fill, a head line | a dialog |
| Search over threads | the search band | inside the list |
| A choice to make before starting | the empty state, one line | a separate screen |

If the person names an area ("over the sidebar", "in the bottom corner"), that wins: do it,
then make sure the area's other rules still hold (insets, Stop last on the status bar).

## Moving a control from one area to another

The real case: "New window from the status bar to the header's right end." The steps are the
same for any icon.

1. **Find where it is now.** `grep -rn '"New window  ⌘' src/`: the tooltip is the most unique
   string. Note the handler it calls (`open_new_window`).
2. **Find the target area's builder.** It's in the map above (`render_header_tail`). Check that
   every place that draws the area uses it (`grep -rn render_header_tail src`).
3. **Rebuild the control as the target area's piece, not as a copy of the old one.** A status
   bar button (`button` inside `render_statusbar`) becomes a header icon:
   `bench_ui::icon_button(id, Ico::X, "Tip  ⌘…", on, cx)` (`crates/ui/src/button.rs`), its
   tooltip naming the shortcut. A button that opens rather than switches (New window, +) passes
   `on` false, so it never lights up.
4. **Order and spacing in the target**: as asked ("Tools, then Tasks"), the area's own gap
   (`gap_1`), and its inset (the last icon's edge 16pt from the sidebar's right edge).
5. **Delete the old one completely.** Remove the element, any now-unused imports and
   variables, and fix every comment and doc line that names the old place — stale comments
   mislead the next agent.
6. **Check what moved with it**: the shortcut (⌘⇧N) still works, the tooltip is right.
7. **Measure** (`build-change-loop.md`): a shot of your own copy before and after;
   `spans.py` over the header lists its icons with their positions and the gap to the edge
   (the icons that were there must not move);
   `spans.py` on the old area shows it's gone. Crop both areas and look at them.

Other moves done the same way:
- **Stop** from the composer to the status bar's right end; the paperclip and the mic too (the
  composer has no buttons).
- **An answer's hover actions** from a row under the answer to an overlay on its top-right
  corner (`absolute().top(23).right_6()`, on the chat background so the text under it is
  covered cleanly).

## Fitting one piece to another

The real case: "The code block's header must be as tall as the Changed files line." Others:
- the search band as tall as the header;
- an insert's head as tall as a code block's (`block_head`, `BLOCK_HEAD_H`);
- the composer's text at the thread's column (`CHAT_TEXT`).

1. **Name the reference piece and find its code.** Changed files' line ("CHANGED FILES" in
   `src/thread/extras.rs`). That is the target. "Similar" is not enough: take its exact
   padding and text size — better, one `bench_ui` constant for both.
2. **Copy those values to the piece you fit**, or make both use the same piece (`block_head`).
   The code header is `render_code` in `src/media/view.rs`; both are `text_base` muted (one text
   size in the thread).
3. **Check every child that can make it taller.** The Copy button had `py_0p5`: 2pt more than
   the reference line. Its vertical padding went. A row is as tall as its tallest child plus
   the row's padding.
4. **Measure both on one screenshot.** `rules.py` down a column that crosses each piece. The
   rule-to-rule distances must be equal (here both 25pt, from 851→901 and 1371→1421 px).
5. **If they're bars facing each other, count the rules.** A bar that draws its rule on top of
   `bar_h` is `bar_h + 1`; a neighbour of exactly `bar_h` is a visible 1px step.
6. **For insets, measure text starts, not code.** `textx.py` from the divider for each band.
   Glyphs differ by about 0.5pt; anything more is a real offset. The composer's textarea pads
   itself; `render_composer` pulls it back so its text starts at `CHAT_TEXT`.

## Placing an icon exactly

- An icon's visible edge = the container's padding + the icon box's own padding. To put a
  toggle's edge at the sidebar's 16pt inset: `pr(12)` on the strip + `p_1` (4pt) on the toggle.
- Inputs pad themselves (about 10pt) and glyphs have side bearings. The search magnifier sits
  over the group icons with `pl(3.5)`: a measured number, not a guess. Measure with `textx.py`
  on the search band and on a group header; the x values must match.
- Icon sizes are fixed per place (`design-icons.md`): `xsmall` in rows, `small` for head and
  status-bar icons. A size between two of these is not allowed: pick the place's size.
- Centered things get equal gaps on both sides. Measure left and right with `measure.py row`.

## When a change makes something else not fit

- **A wider inset**: every fixed-width box in that area must grow by the same amount (the task
  card went 380 → 404 when its padding went 16 → 24; its segmented control had run into the
  edge).
- **A smaller bar**: shrink what sits inside it (the mic's level bar fits the status bar). Never
  let the contents set the bar's height.
- **A field sized to its placeholder**: measure; a clipped or a too-wide field both show.
- **A list that grows past the window**: every child needs `.flex_none()`, or the rows get
  squeezed and the steps between them turn uneven.
- After any change, shoot **threads with other inserts** too (`BENCH_DEMO=…`): shared pieces
  (the top strip, the bars, `block_head`) are in every one.
