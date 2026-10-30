//! Rows and columns with a spacing step. Alignment is said in words, not offsets.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::metrics::Gap;

/// Children side by side, centered vertically, `gap` apart.
pub fn row(gap: Gap) -> Div {
    h_flex().items_center().gap(gap)
}

/// Children one under another, `gap` apart, as wide as the column.
pub fn stack(gap: Gap) -> Div {
    v_flex().w_full().gap(gap)
}

/// A row whose children go to both ends (a title left, actions right).
pub fn spread(gap: Gap) -> Div {
    row(gap).w_full().justify_between()
}

/// A box as wide as its column, up to `max`. What keeps an aspect ratio goes inside it, filling it:
/// on one box, Taffy takes the ratio from the column's width before `max` clamps it, and the
/// column under a picture keeps a picture's worth of air.
pub fn at_most_wide(max: Pixels, child: impl IntoElement) -> Div {
    div().w_full().max_w(max).child(child)
}

/// Space that takes what is left in a row: what follows goes to the right end.
pub fn spacer() -> Div {
    div().flex_1()
}
