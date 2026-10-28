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
- **The only place with pixel values.** A size or a gap is a named constant in `metrics.rs`
  (grouped by window area) or a `Gap` step; screens refer to those names. A piece a screen needs
  and this crate lacks is added here first, then used. Behaviour thresholds (a click's slop) are
  named constants beside the code that uses them, not here.

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

## Known limitations

- Whether a screen writes a raw pixel number is not checked mechanically: `px(…)` in `src/`
  wraps named constants by convention, and a review catches the rest (unconfirmed: no test or
  lint enforcing it was found).
