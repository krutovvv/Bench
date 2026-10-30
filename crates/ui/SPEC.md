---
id: ui
type: module-design
status: draft
title: crates/ui (bench_ui) and the storybook — every element and size
parent: architecture
---

# bench_ui

Bench's elements, as Zed's `ui` crate is Zed's: theme, icons, text, buttons, fields, blocks,
rows, forms, lists, bars, the IDE's pieces, the Finder-tag marks. Every screen is assembled from
them, so a change to how a kind of thing looks is made here once and lands everywhere.
`crates/storybook` (`cargo run -p storybook`) shows every element in every state; its
`reference/` pictures are what the look is compared against.

## Boundary

- Depends on `gpui-kit` and serde only. Knows nothing of Bench: no app state, no agent, no
  protocol.
- Every element is stateless: data and handlers in, an element out. State lives with the
  feature that uses the element.
- **The only place with pixel values and spacing.** A size is a named constant in `metrics.rs`
  (grouped by window area); a gap or an inset a `Gap` step, in rems (`.gap(Gap::M)`, `.px(Gap::L)`).
  Screens refer to those names and write no bare Tailwind spacing. A piece a screen needs
  and this crate lacks is added here first, then used. Behaviour thresholds (a click's slop) are
  named constants beside the code that uses them, not here.

## Decisions

- **One element per pattern**: a family of similar things has one scaffold, and its users bring their
  content: `code_frame` (an answer's code, a step card's), `settings_card` (a provider, an evaluator
  server), `chip`, `fold_more` and its `fold_foot`, `fold_row`, `popover_section`, `segmented`.
- **What handles a click stops its propagation**: every clickable element here says so (`own_press`),
  so a press on it never reaches what is under it (a row, the thread, which would give the composer
  the focus).

## The look

The look is a Mac app's — Xcode, Finder, Mail, System Settings — settled as final on 2026-10-28:
extended in the same voice with Apple's concrete values, never redesigned. Its rules (type
sizes, controls, blocks, scrolling) are `.agents/skills/bench-kit/references/design-look-rules.md`
and the summary in `AGENTS.md` ("The look, in short"); they are not repeated here.

- **One look, two halves.** `BENCH_LIGHT` and `BENCH_DARK` (`theme/palettes.rs`) plus a syntax
  theme JSON for each half (`assets/themes/bench-*.json`, written by `scripts/gen_themes.py`);
  tests check the JSON against the palette. Light or dark follows the person's Mode; there is no
  theme or accent to choose.
- A thread's or a project's mark is a Finder tag dot (`mark.rs`).
- **A thing put down among other things lands beside one, never on it**: `DropEdge`, `drop_edge`
  (which half of a block a point is in) and `drop_line` (the accent hairline over the block, half
  of it past its edge). It is drawn over the block, not in its flow, so the list around it never
  moves while something is held in the air. Any list that takes a reorder uses these, not its own
  arithmetic; `src/sidebar/order.rs` is the one that does today.

## Known limitations

- Whether a screen writes a raw pixel number or a bare spacing is not checked mechanically: a review
  catches it. Whether a change moved a pixel is: every demo state against a reference
  (`.agents/skills/bench-kit/scripts/pixels/`), run by hand.
