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
/// A row of a list inside a block (the changed files): a Mac list's medium row.
pub const LIST_ROW_H: f32 = 26.;
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
/// The thread's column at its widest, its margins with it (a text of ~720 px): a wider pane
/// centres it rather than lengthening its lines.
pub const THREAD_MAX_W: f32 = 860.;
/// The person's message as a bubble (the chat): its padding and corner.
pub const BUBBLE_PX: f32 = 14.;
pub const BUBBLE_PY: f32 = 8.;
pub const BUBBLE_R: f32 = 16.;
/// The pinned composer (the chat): its card's corner, its inset from the thread's sides, the
/// air under it, its tools' line.
pub const COMPOSER_CARD_R: f32 = 18.;
pub const COMPOSER_CARD_PB: f32 = 12.;
/// The air inside the card under its tools' line.
pub const COMPOSER_CARD_PB_IN: f32 = 6.;
/// The chat composer's Send: a round button, and Stop's square mark inside it.
pub const SEND_BUTTON: f32 = 26.;
pub const STOP_MARK: f32 = 9.;
/// Every thread row pads its top and bottom by this, so two rows are 2 × TURN_Y apart.
pub const TURN_Y: f32 = 14.;

/// Every bar's height without its rule (the header's 36 less 1): follows the font size.
pub fn bar_h(cx: &App) -> Pixels {
    cx.theme().font_size * 2.5
}

/// The window's header: a bar and the rule under it.
pub fn header_h(_cx: &App) -> Pixels {
    px(TITLE_BAR_H + HAIRLINE)
}

/// The window's top bar: a Mac's unified title bar with its tools (the title, the meters, Chat
/// | Code) — taller than the status bar under the window (28), as a toolbar is over a status line.
pub const TITLE_BAR_H: f32 = 40.;

/// A page laid out for reading (the new project's): its column's width.
pub const PAGE_W: f32 = 680.;
/// A choice drawn as a card (the new project's "where"): its least height, so a row of them
/// lines up whatever their text.
pub const CHOICE_CARD_MIN_H: f32 = 128.;
/// A choice's radio: its ring, and the ring when picked (the dot is the hole left inside).
pub const RADIO: f32 = 14.;
pub const RADIO_ON_RING: f32 = 4.;

/// The bars' type (the header, the status bar, a pane's strip): one step under the sidebar's,
/// and the header's own title at the sidebar's size, heavier — so the chrome is quieter than
/// what it frames, not a different scale.
pub const BAR_TEXT: f32 = 12.;
/// The thread's own lines, not the conversation: the work line ("8 actions · 3:17 · Done") and
/// a block's head ("Background · 1 running") — a step under the messages, so the answer leads.
pub const THREAD_META: f32 = 13.;
pub const BAR_TITLE: f32 = 13.;
/// The short rule between the sidebar's part of the top bar and the title.
pub const BAR_SEPARATOR_H: f32 = 16.;

/// The header's Chat | Code switch: the track's inset around its segments, and a segment's height.
pub const MODE_SWITCH_PAD: f32 = 2.;
pub const MODE_SEGMENT_H: f32 = 22.;

/// Room left of the header for the window's buttons (macOS's three lights), when the sidebar
/// that usually sits under them is folded away.
pub const WINDOW_BUTTONS_W: f32 = 78.;

/// The sidebar's strip under the window buttons: its icons end at the sidebar's 16 px inset
/// (an icon's 4 px box padding + this).
pub const SIDEBAR_STRIP_PR: f32 = 12.;

/// A row under a group header starts where the header's name does.
pub fn nest_pl(cx: &App) -> Pixels {
    cx.theme().font_size * 2.375 - px(4.)
}

/// Where the sidebar's words start, headers and rows alike, as in a Mac's source list: a row's
/// ground is set in by `SIDEBAR_ROW_INSET` (10, HIG), its text 10 further in.
pub const SIDEBAR_TEXT_X: f32 = SIDEBAR_ROW_INSET + 10.;

/// One line of the code editor (mono font × 1.5).
pub fn editor_line(cx: &App) -> f32 {
    f32::from(cx.theme().mono_font_size) * 1.5
}

/// An editor tall enough for all of its `lines`: it never scrolls, the thread does.
pub fn editor_height(lines: usize, cx: &App) -> f32 {
    (lines.max(1) + 1) as f32 * editor_line(cx) + 16.
}

// ---- The window ---------------------------------------------------------------------------

/// A new window's size, and how far macOS's three lights sit from its corner.
pub const WINDOW_W: f32 = 1240.;
pub const WINDOW_H: f32 = 800.;
pub const TRAFFIC_LIGHTS_INSET: f32 = 14.;

// ---- Lines and marks ----------------------------------------------------------------------

/// A rule or a divider between two things.
pub const HAIRLINE: f32 = 1.;
/// A state's dot (an agent up or down, a terminal running, a bot connected).
pub const DOT: f32 = 6.;
/// A colour's sample in a menu, and the cell it sits in.
pub const SWATCH: f32 = 12.;
pub const SWATCH_CELL: f32 = 16.;
/// A checkbox (a task, a question's choice).
pub const CHECKBOX: f32 = 14.;
/// An icon or a box beside a line of text, lowered onto the text's first line.
pub const ICON_ON_LINE: f32 = 2.;
pub const ICON_ON_SMALL_LINE: f32 = 3.;
/// A caret's width.
pub const CARET_W: f32 = 2.;

// ---- Popovers and fields ------------------------------------------------------------------

/// A panel hung under a chip: numbers (the context), a chart (the speed), lines that wrap
/// (the agent's state).
pub const POPOVER_S: f32 = 260.;
pub const POPOVER_M: f32 = 300.;
pub const POPOVER_L: f32 = 380.;
/// The composer's "/" list.
pub const SLASH_LIST_W: f32 = 560.;
/// A path typed into the thread's head (a server's folder).
pub const PATH_FIELD_W: f32 = 360.;

// ---- The sidebar ---------------------------------------------------------------------------

/// The sidebar's type: a project's name and a thread's title share one size, a step under the
/// messages', so the list reads as a list and not as a page; what they are tells them apart (the
/// name is heavier, the title quieter), not how big they are.
pub const SIDEBAR_TEXT: f32 = 13.;
/// A group's header in the sidebar (a project, the Host): 11 pt semibold in the secondary
/// colour, as Finder's, Mail's and Xcode's sidebar sections — a label over its rows, not a row.
pub const SIDEBAR_HEAD_TEXT: f32 = 11.;
/// Air above a project's head (the first one sits closer), and under it: a header is near its
/// rows and far from the group above, so the groups read apart (Finder's sidebar sections).
pub const GROUP_GAP: f32 = 20.;
pub const GROUP_HEAD_PB: f32 = 3.;
/// A thread's row: its marker stripe, and its padding in the ordinary and the compact list.
pub const ROW_STRIPE: f32 = 2.;
/// A sidebar row is set in from the sidebar's edges by this, so its rounded ground (the open
/// thread's, a hover's) floats in the list.
pub const SIDEBAR_ROW_INSET: f32 = 10.;
/// A mark's dot beside a name (Finder's tag).
pub const TAG_DOT: f32 = 8.;
pub const THREAD_ROW_PY: f32 = 4.;
pub const THREAD_ROW_PY_COMPACT: f32 = 3.;
/// A search hit's inset.
pub const HIT_PL: f32 = 14.;

// ---- The thread ----------------------------------------------------------------------------

/// The files under a message: air above them (after its words) and under them.
pub const ATTACHED_MT: f32 = 10.;
pub const ATTACHED_PB: f32 = 4.;
/// The composer: its field's own padding (gpui-component's input), the tools' column on its
/// right, the tools' line under it, the gap between its lines, the air under the field.
pub const INPUT_PAD_X: f32 = 12.;
/// An icon button's padding round its glyph (`icon_button`).
pub const ICON_BUTTON_PAD: f32 = 4.;
pub const INPUT_PAD_Y: f32 = 10.;
pub const COMPOSER_TOOLS_W: f32 = 56.;
pub const COMPOSER_TOOLS_H: f32 = 40.;
pub const COMPOSER_GAP: f32 = 10.;
pub const COMPOSER_PB: f32 = 6.;
/// The rail of ticks on the thread's right: a tick, the rail, its inset from the edge.
pub const RAIL_TICK_W: f32 = 10.;
pub const RAIL_TICK_H: f32 = 2.;
pub const RAIL_W: f32 = 14.;
pub const RAIL_INSET: f32 = 4.;
/// The speed panel's chart: a bar a second, its height, the gap between bars.
pub const SPEED_BAR_W: f32 = 3.;
pub const SPEED_CHART_H: f32 = 56.;
pub const SPEED_BAR_GAP: f32 = 1.;

// ---- Attachments and inserts --------------------------------------------------------------

/// An attachment's chip: its widest, its icon's square, its remove badge and the badge's inset.
pub const ATTACHMENT_MAX_W: f32 = 260.;
pub const ATTACHMENT_ICON: f32 = 24.;
pub const BADGE: f32 = 18.;
pub const BADGE_INSET: f32 = 3.;
/// A terminal's line when it is drawn as text (the demo's, the web build's).
pub const TERMINAL_LINE_H: f32 = 20.;
/// The task list: a list's head row, the inset of a tag row and of a task (with its 2 px
/// stripe the same 24), the card's field widths.
pub const TASK_HEAD_H: f32 = 22.;
pub const TASK_INSET: f32 = 24.;
pub const TASK_DAY_W: f32 = 110.;
pub const TASK_LIST_W: f32 = 120.;
pub const TASK_TIME_W: f32 = 100.;
pub const TASK_SPENT_W: f32 = 64.;
