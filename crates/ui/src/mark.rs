//! The colours a thread or a project can be marked with, as Finder's tags: a small dot of the
//! colour beside the name (`sidebar/`), the menu's row of them (`sidebar/menu.rs`).

use gpui_kit::*;

/// Red, orange, yellow, green, blue, violet, pink — Finder's tag colours. Saved by index: new
/// ones go at the end, so a saved mark never changes colour.
const MARKS: [u32; 7] = [0xFF5F57, 0xFF9F0A, 0xFFCC00, 0x34C759, 0x0A84FF, 0xBF5AF2, 0xFF6482];

/// How many mark colours there are.
pub const MARK_COUNT: usize = MARKS.len();

/// The mark colour `index`, if there is one.
pub fn mark_color(index: usize) -> Option<Hsla> {
    MARKS.get(index).map(|c| rgb(*c).into())
}
