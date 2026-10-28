//! How a thread is laid out — one type, chosen here. `Chat` (the default): no avatars, the
//! person's messages as bubbles, the composer pinned under the list. `Document`: the thread as
//! an editor's file — avatars in a gutter, the composer as its last row, room past the end.
//! To bring the document back, change `Current` below; nothing else.

use gpui_kit::*;

/// A thread's layout.
pub trait Look {
    /// Who speaks is an avatar in a gutter beside each turn.
    const AVATARS: bool;
    /// The composer is the list's last row (it scrolls with the thread); else it is pinned
    /// under the list.
    const COMPOSER_IN_LIST: bool;
    /// The person's messages sit in a bubble on the right.
    const USER_BUBBLE: bool;
}

/// A classic chat, as Messages and every chat app: what a first-time reader expects.
pub struct Chat;
impl Look for Chat {
    const AVATARS: bool = false;
    const COMPOSER_IN_LIST: bool = false;
    const USER_BUBBLE: bool = true;
}

/// The thread as a document being written with the agent (Bench's first look).
#[allow(dead_code)]
pub struct Document;
impl Look for Document {
    const AVATARS: bool = true;
    const COMPOSER_IN_LIST: bool = true;
    const USER_BUBBLE: bool = false;
}

/// The look in use.
pub type Current = Chat;

pub const AVATARS: bool = <Current as Look>::AVATARS;
pub const COMPOSER_IN_LIST: bool = <Current as Look>::COMPOSER_IN_LIST;
pub const USER_BUBBLE: bool = <Current as Look>::USER_BUBBLE;

/// A turn's row in the current look: with its avatar (`Some(agent)`, or the gutter kept empty
/// with `None`) in the document, set in by the thread's margins alone in the chat.
pub fn turn(agent: Option<bool>, body: impl IntoElement, cx: &App) -> Div {
    if AVATARS {
        bench_ui::turn_row(agent, body, cx)
    } else {
        bench_ui::turn_row_plain(body, cx)
    }
}

/// Where a turn's text starts from the row's edge.
pub fn text_x() -> f32 {
    if AVATARS {
        bench_ui::CHAT_TEXT
    } else {
        bench_ui::CHAT_LEFT
    }
}

/// Its right margin.
pub fn text_right() -> f32 {
    if AVATARS {
        bench_ui::CHAT_RIGHT
    } else {
        bench_ui::CHAT_LEFT
    }
}
