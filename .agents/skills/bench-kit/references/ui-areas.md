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
y ┌──────────────────────────────────────────────────────────────────────────────────────┐
0 │ ● ● ●  ◧ │ TITLE (13 semibold) [/ second thread ×]                  [Chat|Code] │ 0–40
40│━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│ rule
  │ ✎ New thread       ⌘N    │ THE THREAD (gpui::list, the only scroll), centred ≤ 860:│
  │ ⊞ New project            │   bubbles, "N actions", answers, inserts where put,     │
  │ 🔍 Search          ⌘K    │   cards under them; the overlay scrollbar               │
  │ Host / Project ● ✈ rows  │ ┌ + text …                                  🎤 (↑) ┐    │
  │──────────────────────────│ └ COMPOSER, pinned, COMPOSER_CARD_PB over the bar ─┘    │
  │ ● This Mac 1 working   ⚙ │                                                         │ SIDEBAR_FOOT_H 40
  └──────────────────────────┴─────────────────────────────────────────────────────────┘
```

| Area | Height | Inside | Inset | Code |
|---|---|---|---|---|
| Title bar | `TITLE_BAR_H` 40 incl. its rule | the window's buttons, the fold ⌘\, a short rule, the title (a second thread's after "/"), Chat \| Code | left `WINDOW_BUTTONS_W` | `render_window_lead`, `render_header` in `src/titlebar/mod.rs`; Chat \| Code `src/titlebar/tail.rs` |
| Actions | three rows | New thread, New project, Search (⌘K opens a list over the window) | `nav_row`, text at `SIDEBAR_TEXT_X` | `render_actions` in `src/sidebar/mod.rs`; the list `src/sidebar/search.rs` |
| Sidebar list | rest | Host's threads first, then each project, in the order the person dragged them, with its threads | rows at `SIDEBAR_ROW_INSET`, text at `SIDEBAR_TEXT_X` | `src/sidebar/mod.rs`; order and acts `src/sidebar/items.rs` |
| Left dock's foot | `SIDEBAR_FOOT_H` 40 incl. its rule | the open thread's agent (where, a dot, "N working") and its popover; the gear (⌘,) | `SIDEBAR_ROW_INSET` | `render_foot`, `src/sidebar/agent.rs`, under any left panel (`workspace/layout.rs`) |
| The thread | rest | rows from `crates/thread`, centred in `THREAD_MAX_W`; inserts where the journal puts them | `CHAT_LEFT` (`CHAT_LEFT_NARROW` in a dock) | `src/thread/mod.rs`; inserts `src/inserts/mod.rs` |
| Composer | its text | plus, text, mic, Send in one rounded field; chips over the text | the column's sides | `src/thread/composer/input.rs` |
| Composer's foot | `COMPOSER_FOOT_H` | the thread's project (a new thread's to choose), its repo chip; running terminals and the meters (context, speed) on the right | the column's sides | `src/thread/composer/foot.rs` |
| Code status bar | `STATUS_H` 29 incl. its rule, only under the Code view's documents and terminal | the caret (Ln, Col · indent · language), ✦ the thread beside ⌘J, ▭ the terminal ⌃` | `px_1` | `src/explorer/statusbar.rs` |
| Explorer bar (Code) | `bar_h` + its rule, level with the tabs | the project pull-down, Files \| Threads glyphs, Refresh, ⌘P | `SIDEBAR_ROW_INSET` | `explorer_picker`, `src/explorer/tree/view.rs` |

Every rule that crosses the window is one pixel row. Settings pages are a centred column of
`SETTINGS_W`.

## Which area a thing belongs to

| The thing | Area | Never |
|---|---|---|
| A new tool (a server, a utility) | an insert put in with `/` (`build-insert.md`) | a sidebar group, a mode, a side panel |
| A form a tool needs (a new server, a new host) | an agent-message card under the insert that asked for it | a dialog, a settings page |
| A project's settings (name, where, model, Telegram) | its Settings page (a tab, `DocKey::Settings`, `src/env/page.rs`) | a thread, a dialog |
| What a project contains (files, soul, prompts, skills, MCP, schedule) | the Explorer (`src/explorer/`, `src/explorer/SPEC.md`) | a thread, a settings tab |
| The sidebar's fold | the title bar, right after the traffic lights; New project: a row over the sidebar's list | a bar under the window |
| New window, Settings, and every other command | the menu bar (`app/menu.rs`), with its key | a button in a bar |
| Attach, dictate, Send / Stop | the composer's field | a bar |
| The agent's state | the left dock's foot; its popover | — |
| The Code view's toggles (terminal, thread beside) | its status bar | the title bar, the chat |
| An open file's caret, indent, language | the Code status bar | — |
| The thread's facts (project, repo, context, speed) | the line under its composer | the title bar |
| Rare actions on a thread or project | its row's hover "⋯" and right-click (one `item_menu`) | the header |
| Quick actions on a row (a new thread "+") | on the row, shown on hover | always visible |
| Actions on an answer (copy, read aloud) | at the end of its work line, on hover | a row of their own |
| Results that come with an answer (files, processes, code) | a block under the answer: `block_frame`, `block_head` | a dialog |
| Search over threads and projects | ⌘K's list over the window (the sidebar's "Search" row) | a field in the sidebar |
| A choice to make before starting | the empty state, one line | a separate screen |

If the person names an area ("over the sidebar", "in the bottom corner"), that wins: do it,
then make sure the area's other rules still hold (insets, Chat | Code last in the title bar).

## Moving a control from one area to another

A control moving from a bar to the header's right end. The steps are the same for any icon.

1. **Find where it is now.** `grep -rn 'Hide sidebar  ⌘' src/`: the tooltip is the most unique
   string. Note the handler it calls.
2. **Find the target area's builder.** It's in the map above (`render_header_tail`). Check that
   every place that draws the area uses it (`grep -rn render_header_tail src`).
3. **Rebuild the control as the target area's piece, not as a copy of the old one.** A status
   bar button (`bench_ui::status_button`) becomes a header icon:
   `bench_ui::icon_button(id, Ico::X, "Tip  ⌘…", on, cx)` (`crates/ui/src/button.rs`), its
   tooltip naming the shortcut. A button that opens rather than switches (New project, +) passes
   `on` false, so it never lights up.
4. **Order and spacing in the target**: as asked, the area's own gap (`Gap::Xs`), and its inset (the last icon's edge 16pt from the sidebar's right edge).
5. **Delete the old one completely.** Remove the element, any now-unused imports and
   variables, and fix every comment and doc line that names the old place — stale comments
   mislead the next agent.
6. **Check what moved with it**: the shortcut still works, the tooltip is right.
7. **Measure** (`build-change-loop.md`): a shot of your own copy before and after;
   `spans.py` over the header lists its icons with their positions and the gap to the edge
   (the icons that were there must not move);
   `spans.py` on the old area shows it's gone. Crop both areas and look at them.

Other moves done the same way: New window from the header to the menu bar; Stop, attach and
the mic from the status bar into the composer's field; a second thread's title from a head over
the dock into the title bar.

## Fitting one piece to another

The real case: "The code block's header must be as tall as the Changed files line." Others:
- the Explorer's bar as tall as the documents' tabs (`bar_h`, one rule);
- an insert's head as tall as a code block's (`block_head`, `BLOCK_HEAD_H`);
- a step row as tall as a changed file's (`LIST_ROW_H`).

1. **Name the reference piece and find its code.** Changed files' head (`src/thread/rows/changed_files.rs`). That is the target. "Similar" is not enough: take its exact
   padding and text size — better, one `bench_ui` constant for both.
2. **Copy those values to the piece you fit**, or make both use the same piece (`block_head`).
   The code header is `bench_ui::code_frame`; both are `THREAD_META` muted.
3. **Check every child that can make it taller.** A Copy button with its own vertical padding was
   2pt more than the reference line. Its vertical padding went. A row is as tall as its tallest child plus
   the row's padding.
4. **Measure both on one screenshot.** `rules.py` down a column that crosses each piece. The
   rule-to-rule distances must be equal (here both 25pt, from 851→901 and 1371→1421 px).
5. **If they're bars facing each other, count the rules.** A bar that draws its rule on top of
   `bar_h` is `bar_h + 1`; a neighbour of exactly `bar_h` is a visible 1px step.
6. **For insets, measure text starts, not code.** `textx.py` from the divider for each band.
   Glyphs differ by about 0.5pt; anything more is a real offset. The composer's textarea pads
   itself; `render_composer` pulls it back by `INPUT_PAD_X` / `INPUT_PAD_Y`.

## Placing an icon exactly

- An icon's visible edge = the container's padding + the icon box's own padding. To put a
  toggle's edge at the sidebar's 16pt inset: the strip's `SIDEBAR_STRIP_PR` + the toggle's own
  `ICON_BUTTON_PAD`.
- Inputs pad themselves (about 10pt; `flush_field` pulls a bare one back) and glyphs have side
  bearings. A number that lines them up is measured, not a guess, and named in `metrics.rs`. Measure with `textx.py`
  on the search field and on a group head; the x values must match.
- Icon sizes are fixed per place (`design-icons.md`): `xsmall` in rows, `small` for head and
  status-bar icons. A size between two of these is not allowed: pick the place's size.
- Centered things get equal gaps on both sides. Measure left and right with `measure.py row`.

## When a change makes something else not fit

- **A wider inset**: every fixed-width box in that area must grow by the same amount (a card went
  380 → 404 when its padding went 16 → 24; its segmented control had run into the edge).
- **A smaller bar**: shrink what sits inside it (the mic's level bar fits the status bar). Never
  let the contents set the bar's height.
- **A field sized to its placeholder**: measure; a clipped or a too-wide field both show.
- **A list that grows past the window**: every child needs `.flex_none()`, or the rows get
  squeezed and the steps between them turn uneven.
- After any change, shoot **threads with other inserts** too (`BENCH_DEMO=…`): shared pieces
  (the top strip, the bars, `block_head`) are in every one.
