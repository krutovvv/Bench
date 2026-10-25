# The look: the rules behind every view

1. **Flat and square.** `theme.radius = 0`, `theme.shadow = false`. No rounded cards, no drop
   shadows, no gradients. Separation is done with 1px borders and one-step-darker chrome.
   Even "slight" rounding of inputs/buttons/message blocks was tried and rejected as ugly. The
   one exception: buttons that float inside a field (composer Dictate / Send) are circles.
2. **Light One Light by default** (dark One Dark and "System" optional): content `#FAFAFA`,
   sidebar `#EFEFF0`, title/status bars `#E9E9EA`, borders `#D9D9DA`, text `#242529`, muted
   `#6E6F75`, one accent (blue `#4F6FD8`). Exact palettes in Bench's `src/theme.rs`.
3. **One accent, only errors red.** Accent marks the active thing (selected row stripe, primary
   button, focus, live values). Red is for errors only — counts, failed steps. Warnings amber.
4. **Fonts:** UI 14px (it is also the rem, so spacing scales with it), code/mono 12px.
   Size presets S/M/L = 13/14/15 UI and 11/12/13 mono. System font for UI, Menlo for code.
   The conversation (answers and the user's messages) is `text_base` — a size up from the
   sidebar's `text_sm` lists, so the eye goes to the middle (2026-10-24, after Zed's Delta; it was
   `text_sm` before). Turns are told apart by avatars in a gutter, not by a colored band.
   The chrome (sidebar `#EAEAEC`, header / status bar `#E4E4E6`) is half a step darker than it
   was, and the sidebar meets the chat with **no rule**: the window's column dividers are
   transparent at rest and an accent line on hover (`ui/mod.rs` `quiet_divider`). The whole thread uses one size, the messages' `text_base`: steps, meta lines, cards' headers too; secondary things are muted, never smaller. One family everywhere in the chat: no
   monospace or bigger fonts jumping between blocks except real code.
5. **One inset per area:** 16px in the sidebar and in headers; 24px around reading content in
   the main area (the chat's question band and answer, the composer's text, task rows and the
   task card) — one number, the same left, right, top and bottom. Vertical rhythm on a 4/8
   grid. Equal gaps on both sides of anything.
6. **Every bar is the header's height and every rule crossing the window is one line:** the
   sidebar's search band, "New session", the composer, a list's tag strip, a "New item" row =
   36 with their rule (`bar_h` = 35 + 1). Header 36px, status bar 28px. What sits inside a bar
   is sized to fit it (the composer's buttons are 26), never the bar to its contents. Checked
   to the pixel: a 1px step is a bug.
7. **Long text never wraps in one-line places:** session titles, step summaries, list
   descriptions, status bar labels → `.truncate()` (ellipsis). Where titles may wrap (skills,
   prompts lists), use line-height 1.25 so the lines stay one block.
8. **Quiet affordances:** actions appear on hover (row "⋯", copy/speak/regenerate under an
   answer, "+" on a group header); no scrollbars — a thin rail of ticks instead; popups attach
   to the thing they belong to (command list rests on the input, no gap, no double border).
9. **No dialogs for simple choices.** Arm-then-confirm buttons ("Delete" → "Confirm delete"),
   inline fields, sensible defaults (e.g. a new project's environment starts from the Host's).
10. **English UI**, short labels, tooltips for icons, and nothing added to the status bar
    without the user's agreement.
11. **Nothing jumps.** Lay content out at its final size in the first frame (sizes of local
    images, diagrams, previews computed synchronously; reserve a box for what loads later).
    Show no placeholder values before real data (a status bar without "—"/default pickers).
12. **Lightweight only.** No browser engines, WebViews or JS runtimes inside an app, ever;
    pure-Rust crates and macOS tools; ffmpeg is acceptable.
13. **Slow work only on request.** Don't start network checks or long commands because a tab
    was opened — give them a button ("Check versions").

