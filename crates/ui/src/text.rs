//! Text styles. A piece of text is one of these, never a size picked in place.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

use crate::metrics::LEAD;

/// The thread's text: messages, answers, the composer. One size, one leading.
pub fn body(cx: &App) -> Div {
    div()
        .text_color(cx.theme().foreground)
        .line_height(relative(LEAD))
}

/// Secondary text: meta, hints, empty states.
pub fn muted(cx: &App) -> Div {
    div().text_sm().text_color(cx.theme().muted_foreground)
}

/// Small text: chips, captions, pane strips.
pub fn small(cx: &App) -> Div {
    div().text_xs().text_color(cx.theme().muted_foreground)
}

/// A title: a block's or a form's name. Semibold: the UI font has no medium face, and Medium
/// draws as the regular text beside it.
pub fn title(cx: &App) -> Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
}

/// Code, paths, terminals.
pub fn mono(cx: &App) -> Div {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(cx.theme().mono_font_size)
}

/// An error, said in place.
pub fn error(cx: &App) -> Div {
    div().text_sm().text_color(cx.theme().danger)
}
