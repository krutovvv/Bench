//! A new thread's welcome: Bench's mark drawn from its logo (`assets/logo.svg`: two slabs, a
//! bench seen from the side or two lines of a chat) in the theme's ink and accent, and the
//! round suggestion chips under it, as Claude's and Messages' empty conversations offer them.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::{CHIP_H, THREAD_META};

/// The logo's slabs on its 1024 grid: the long one's width, the short one's, a slab's height,
/// the air between them.
const LONG: f32 = 444.;
const SHORT: f32 = 330.;
const SLAB: f32 = 190.;
const AIR: f32 = 44.;

/// Bench's mark, `size` wide: the upper slab in the text's ink, the lower in the accent — so it
/// is the logo in a light window and in a dark one.
pub fn logo_mark(size: f32, cx: &App) -> Div {
    let theme = cx.theme();
    let k = size / LONG;
    v_flex()
        .flex_none()
        .gap(px(AIR * k))
        .child(div().w(px(SHORT * k)).h(px(SLAB * k)).bg(theme.foreground))
        .child(div().w(px(size)).h(px(SLAB * k)).bg(theme.primary))
}

/// A suggestion under the welcome: a round chip with an icon, its words and a quiet hint (the
/// `/` word that does the same), hovered as a button.
pub fn suggestion_chip(
    id: impl Into<ElementId>,
    icon: Ico,
    label: impl Into<SharedString>,
    hint: Option<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .flex_none()
        .h(px(CHIP_H))
        .px_3()
        .gap_1p5()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(theme.border)
        .text_size(px(THREAD_META))
        .text_color(theme.foreground)
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(div().text_color(theme.muted_foreground).child(icon.icon().small()))
        .child(label.into())
        .children(hint.map(|h| div().text_color(crate::text::faint(&theme)).child(h)))
}
