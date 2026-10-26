//! Every size in Bench. Code outside this crate names these, never a number of pixels.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

/// The spacing steps. Everything between elements is one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    /// 2 px: an icon and its label inside a chip.
    Xs,
    /// 4 px: fields of a form, items of a tight row.
    S,
    /// 8 px: buttons of a row, blocks of a card.
    M,
    /// 12 px: groups inside a block.
    L,
    /// 16 px: the inset from a panel's edge.
    Xl,
}

impl Gap {
    pub fn px(self) -> Pixels {
        px(match self {
            Gap::Xs => 2.,
            Gap::S => 4.,
            Gap::M => 8.,
            Gap::L => 12.,
            Gap::Xl => 16.,
        })
    }
}

/// A field's height (gpui-kit's input: 28 pt). Buttons, pickers and chips of a form match it.
pub const FIELD_H: f32 = 28.;
/// The air a focused field needs below it: the focus ring is 3 px wide and drawn outside the box.
pub const RING_CLEAR: f32 = 4.;
/// The head of every block (an insert, a code block, a pane, an open file).
pub const BLOCK_HEAD_H: f32 = 32.;
/// Small rows and chips in the thread: an attachment, a queued message, a plan's head.
pub const CHIP_H: f32 = FIELD_H;
/// The status bar with the rule over it.
pub const STATUS_H: f32 = 29.;
/// The thread's leading, in lines: messages and the composer share it.
pub const LEAD: f32 = 1.5;
/// A picture's square in the composer and in a sent message.
pub const THUMB: f32 = 64.;
/// A filled control outside a bar ("Add server"), in rems.
pub const BAR_CONTROL_REMS: f32 = 2.;

/// The thread's columns: the avatar at `CHAT_LEFT` (as far from the edge as two rows are apart), the text at `CHAT_TEXT`, and as much free on
/// the right of the text as on its left (`CHAT_RIGHT`).
pub const CHAT_LEFT: f32 = 2. * TURN_Y;
pub const AVATAR: f32 = 24.;
pub const CHAT_TEXT: f32 = CHAT_LEFT + AVATAR + 14.;
pub const CHAT_RIGHT: f32 = CHAT_TEXT;
/// Every thread row pads its top and bottom by this, so two rows are 2 × TURN_Y apart.
pub const TURN_Y: f32 = 14.;

/// Every bar's height without its rule (the header's 36 less 1): follows the font size.
pub fn bar_h(cx: &App) -> Pixels {
    cx.theme().font_size * 2.5
}

/// A row under a group header starts where the header's name does.
pub fn nest_pl(cx: &App) -> Pixels {
    cx.theme().font_size * 2.375 - px(4.)
}

/// One line of the code editor (mono font × 1.5).
pub fn editor_line(cx: &App) -> f32 {
    f32::from(cx.theme().mono_font_size) * 1.5
}

/// An editor tall enough for all of its `lines`: it never scrolls, the thread does.
pub fn editor_height(lines: usize, cx: &App) -> f32 {
    (lines.max(1) + 1) as f32 * editor_line(cx) + 16.
}
