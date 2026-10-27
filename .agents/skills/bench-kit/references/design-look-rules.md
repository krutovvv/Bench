# The look: the rules behind every view

1. **Flat, slightly rounded.** `radius` 4 (controls), `radius_lg` 6; every bordered block is
   `rounded_md` (`bench_ui::block`), `shadow = false`. No gradients, no floating cards, no fills
   behind cards. Separation is 1px borders and one-step-darker chrome. `crates/ui/src/theme.rs`.
2. **Light One Light by default** (dark One Dark and "System" optional): content `#FAFAFA`,
   sidebar `#EAEAEC`, header / status bar `#E4E4E6`, borders `#D9D9DA`, text `#242529`, muted
   `#6E6F75`, one accent (blue `#4F6FD8`). Exact palettes: `light_palette`, `dark_palette`.
3. **One accent, only errors red.** Accent marks the active thing (selected row stripe, primary
   button, focus, live values). Red is for errors only — counts, failed steps. Warnings amber.
4. **Fonts:** UI 14px (it is also the rem), code 12px; S/M/L = 13/14/15 and 11/12/13 by ⌘+ / ⌘−.
   System font for UI, Menlo for code. **The thread is one size, `text_base`** — messages,
   steps, meta lines, cards' heads, the composer; secondary things are muted, never smaller.
   Chrome is `text_sm`. Who speaks is an avatar in a gutter, not a band. The sidebar meets the
   thread with **no rule**: column dividers are transparent at rest, an accent line on hover
   (`src/workspace/layout.rs` `quiet_divider`).
5. **Every size is a `bench_ui` name** (`crates/ui/src/metrics.rs`): gaps are `Gap` steps
   (2/4/8/12/16), fields `FIELD_H` 28, block heads `BLOCK_HEAD_H` 32, the thread's columns
   `CHAT_LEFT` / `CHAT_TEXT` / `CHAT_RIGHT` (28 / 66 / 66), rows `TURN_Y` 14 top and bottom.
   No `px(…)` outside `crates/ui`; a missing size is added there first.
6. **Every bar is the header's height and every rule crossing the window is one line:** the
   sidebar's top strip + search band, the header = 36 (`bar_h` = 35 + 1). Status bar
   `STATUS_H` 29 with its rule. What sits inside a bar is sized to fit it, never the bar to its
   contents. Checked to the pixel: a 1px step is a bug.
7. **Long text never wraps in one-line places:** session titles, step summaries, list
   descriptions, status bar labels → `.truncate()` (ellipsis). Where titles may wrap (skills,
   prompts lists), use line-height 1.25 so the lines stay one block.
8. **Quiet affordances:** actions appear on hover (row "⋯", copy/speak/regenerate on an
   answer, "+" on a project); no scrollbars — a thin rail of ticks instead; popups attach to the
   thing they belong to (the `/` list under the composer's line, or above it without room).
   The composer has no buttons: ⌘Enter, ⌘⇧D, drop or paste; Stop and the mic are the status bar's.
9. **Only the thread scrolls.** No scroll inside a scroll (the terminal excepted): editors are as
   tall as their text (`editor_height`), lists as long as they are, empty states one line.
10. **No dialogs for simple choices.** Arm-then-confirm (a thread: ⌫ ⌫; a project:
   Remove · Cancel; `arm_delete_button`), rename in place, inline fields, sensible defaults (e.g. a new project's environment starts from the Host's).
11. **English UI**, short labels, tooltips for icons, and nothing added to the status bar
    without the user's agreement.
12. **Nothing jumps.** Lay content out at its final size in the first frame (sizes of local
    images, diagrams, previews computed synchronously; reserve a box for what loads later).
    Show no placeholder values before real data (a status bar without "—"/default pickers).
13. **Lightweight only.** No browser engines, WebViews or JS runtimes inside an app, ever;
    pure-Rust crates and macOS tools; ffmpeg is acceptable.
14. **Slow work only on request.** Don't start network checks or long commands because a tab
    was opened — give them a button ("Check versions").

