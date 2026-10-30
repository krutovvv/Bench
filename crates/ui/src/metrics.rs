//! Every size in Bench. Code outside this crate names these, never a number of pixels.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

/// The spacing steps. Everything between elements is one of these, in rems, so they follow the
/// theme's size: at its 14 px, Xs is 3.5 and Xl is 14. Say them as `.gap(Gap::M)`, `.px(Gap::L)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    Zero,
    /// A hairline: the lines of a log.
    Hair,
    /// An eighth of the font: a label and its hint.
    Xxs,
    /// A quarter: fields of a form, items of a tight row.
    Xs,
    /// Three eighths: an icon and its label.
    S,
    /// Half: buttons of a row, blocks of a card.
    M,
    /// Five eighths: a settings row's air.
    Ml,
    /// Three quarters: the inset inside a block.
    L,
    /// The font's size: the inset from a panel's edge.
    Xl,
    /// One and a half: air over a group.
    Xxl,
    /// Twice: air over a page.
    Huge,
}

impl Gap {
    fn length(self) -> DefiniteLength {
        match self {
            Gap::Zero => px(0.).into(),
            Gap::Hair => px(HAIRLINE).into(),
            Gap::Xxs => rems(0.125).into(),
            Gap::Xs => rems(0.25).into(),
            Gap::S => rems(0.375).into(),
            Gap::M => rems(0.5).into(),
            Gap::Ml => rems(0.625).into(),
            Gap::L => rems(0.75).into(),
            Gap::Xl => rems(1.).into(),
            Gap::Xxl => rems(1.5).into(),
            Gap::Huge => rems(2.).into(),
        }
    }
}

impl From<Gap> for DefiniteLength {
    fn from(gap: Gap) -> Self {
        gap.length()
    }
}

impl From<Gap> for Length {
    fn from(gap: Gap) -> Self {
        Length::Definite(gap.length())
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
/// A long code block while folded: its first lines, with the line that unfolds it below.
pub const CODE_FOLD_H: f32 = 140.;
/// The thread's leading, in lines: messages and the composer share it.
pub const LEAD: f32 = 1.5;
/// The code editor's leading, Zed's "comfortable" (1.618 × the font): the library's 1.5 set code
/// lines a touch tighter than Zed and Xcode do.
pub const CODE_LEAD: f32 = 1.618;
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
/// A thread narrower than this (in a dock, beside a document or a thread) takes the narrow
/// sides: its rows and composer set in by CHAT_LEFT_NARROW, as a sidebar's content is.
pub const THREAD_NARROW_W: f32 = 640.;
pub const CHAT_LEFT_NARROW: f32 = 16.;
/// The thread's column at its widest, its margins with it (a text of ~720 px): a wider pane
/// centres it rather than lengthening its lines.
pub const THREAD_MAX_W: f32 = 860.;
/// The person's message as a bubble (the chat): its padding and corner.
pub const BUBBLE_PX: f32 = 14.;
pub const BUBBLE_PY: f32 = 8.;
pub const BUBBLE_R: f32 = 16.;
/// The pinned composer (the chat): one line of a field, as Claude's — the plus, the text, the
/// mic and Send — its corner, the air under it in the window: its foot's line centred where the
/// sidebar's foot (the agent, the gear) is, one line across the window's bottom.
pub const COMPOSER_CARD_R: f32 = 14.;
pub const COMPOSER_CARD_PB: f32 = (SIDEBAR_FOOT_H - COMPOSER_FOOT_H) / 2.;
/// gpui-kit's `.small()` button: 24 pt high.
pub const SMALL_BUTTON_H: f32 = 24.;
/// The line under the composer (its project, repo, meters), as Claude's: its small buttons'
/// height, so their hover is all the air between it and the field.
pub const COMPOSER_FOOT_H: f32 = SMALL_BUTTON_H;
/// The tallest the pinned composer grows, in lines; past it the text scrolls inside the field, so
/// Send and the thread stay on screen whatever is pasted.
pub const COMPOSER_MAX_LINES: usize = 16;
/// The air inside the card around its line (the buttons'; the text is centred on their height).
pub const COMPOSER_CARD_PY: f32 = 8.;
/// Its sides: the plus and Send set in by this from the card's rule.
pub const COMPOSER_CARD_PX: f32 = 8.;
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

/// A new thread's welcome (`welcome.rs`): Bench's mark, its title (a Mac's large title, as an
/// empty window of Notes or Xcode's welcome names itself), the gap over its suggestions.
pub const WELCOME_MARK: f32 = 40.;
pub const WELCOME_TITLE: f32 = 22.;
pub const WELCOME_GAP: f32 = 20.;
/// Its message when nothing can answer yet (no model): a readable column, not the thread's.
pub const WELCOME_NOTE_W: f32 = 440.;

/// A page laid out for reading (the new project's): its column's width.
pub const PAGE_W: f32 = 680.;
/// A settings page's column (the gear's, a project's Settings): its widest.
pub const SETTINGS_W: f32 = 760.;
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
/// The second line of a thread's row (a run's calls under its title): a step quieter than meta.
pub const THREAD_DETAIL: f32 = 12.;
/// A step's side column (what came back), as Finder's Date Modified: a step under the detail.
pub const THREAD_SIDE: f32 = 11.;
pub const BAR_TITLE: f32 = 13.;
/// The short rule between the sidebar's part of the top bar and the title.
pub const BAR_SEPARATOR_H: f32 = 16.;

/// The header's Chat | Code switch: the track's inset around its segments, and a segment's height.
pub const MODE_SWITCH_PAD: f32 = 2.;
pub const MODE_SEGMENT_H: f32 = 22.;
/// A segmented control's segment on a page (settings, forms): a Mac's regular size.
pub const SEGMENT_H: f32 = 24.;

/// Room left of the header for the window's buttons (macOS's three lights): the sidebar's fold
/// after them stands clear of the green one, a step wider than the lights' own spacing.
pub const WINDOW_BUTTONS_W: f32 = 84.;

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
/// The chat's "/" list, as a Mac's completion list (Xcode's, Spotlight's): its rows, and the air
/// between it and the field it sits over, as wide as that field.
pub const COMPLETION_ROW_H: f32 = 26.;
pub const COMPLETION_GAP: f32 = 6.;
/// A step's verb column in a work's list ("read", "web fetch"): the targets start in one line.
pub const STEP_VERB_W: f32 = 88.;
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
/// A thread's row: its marker stripe, and its padding.
pub const ROW_STRIPE: f32 = 2.;
/// A sidebar row is set in from the sidebar's edges by this, so its rounded ground (the open
/// thread's, a hover's) floats in the list.
pub const SIDEBAR_ROW_INSET: f32 = 10.;
/// The Code view's status bar with the rule over it (none in the chat).
pub const STATUS_H: f32 = 29.;
/// The sidebar's foot (its agent and the gear) with the rule over it, as Claude's account row.
pub const SIDEBAR_FOOT_H: f32 = 40.;
/// A mark's dot beside a name (Finder's tag).
pub const TAG_DOT: f32 = 8.;

/// Where a row dragged in the sidebar will land: the line that says it (`drop_line`).
pub const DROP_LINE: f32 = 2.;
/// A group head's marks (a Telegram bot, a schedule): a step under the head's controls (12), so
/// they inform without asking to be clicked.
pub const HEAD_MARK_ICON: f32 = 10.;
pub const THREAD_ROW_PY: f32 = 4.;
/// A fork's corner (`nest_corner`): its width, its height over the title's middle, its round.
pub const NEST_CORNER_W: f32 = 7.;
pub const NEST_CORNER_H: f32 = 8.;
pub const NEST_CORNER_R: f32 = 2.;
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
/// The speed panel's chart: a bar a second, its height, the gap between bars.
pub const SPEED_BAR_W: f32 = 3.;
pub const SPEED_CHART_H: f32 = 56.;
pub const SPEED_BAR_GAP: f32 = 1.;

// ---- Attachments and inserts --------------------------------------------------------------

/// The size of the thread's words, in points.
pub const THREAD_TEXT: f32 = 14.;
/// A utility insert: its least height, and the air its panes keep round their lines.
pub const TOOL_MIN_H: f32 = 200.;
pub const TOOL_LINES_PAD: f32 = 24.;
/// The tallest a terminal or a browser tab gets in a thread.
pub const TERMINAL_MAX_H: f32 = 280.;
pub const BROWSER_MAX_H: f32 = 620.;
/// A terminal view's own padding above and below its lines, together.
pub const TERMINAL_PAD_Y: f32 = 16.;
/// The largest box an image or a video takes in an answer; a document's first page; a diagram or
/// a Markdown file's width and a diagram's height; how much of a long Markdown file shows.
pub const MEDIA_MAX_W: f32 = 560.;
pub const MEDIA_MAX_H: f32 = 420.;
pub const MEDIA_PAGE_W: f32 = 240.;
pub const MEDIA_WIDE_W: f32 = 720.;
pub const DIAGRAM_MAX_H: f32 = 620.;
pub const MARKDOWN_PREVIEW_H: f32 = 280.;
/// A new thread's welcome stands this far down the window (a share of its height).
pub const WELCOME_TOP: f32 = 0.22;
/// A plan's nested step: its indent per level.
pub const PLAN_INDENT: f32 = 16.;
/// The mic's level bar while dictating: how far past the glyph it stands, its height from silence
/// to loud.
pub const MIC_BAR_OUT: f32 = 2.;
pub const MIC_BAR_MIN_H: f32 = 2.;
pub const MIC_BAR_RANGE_H: f32 = 9.;
/// The `/` list in the document's composer: its row, how many show, the air inside it.
pub const SLASH_ROW_H: f32 = 30.;
pub const SLASH_ROWS: f32 = 8.;
pub const SLASH_PAD: f32 = 10.;
/// The menu's icon column, which a row without an icon pulls back over to sit centred.
pub const MENU_ICON_COL: f32 = 16.;

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
