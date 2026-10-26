//! Text fields. Every one-line field is `FIELD_H` tall; a multi-line one is as tall as its text.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::*;

use crate::icons::Ico;

/// A one-line field.
pub fn field(state: &Entity<InputState>) -> Input {
    Input::new(state)
}

/// A field that reads as text until focused (a name typed in place).
pub fn bare_field(state: &Entity<InputState>) -> Input {
    Input::new(state).appearance(false)
}

/// A password: masked, with the eye to show it.
pub fn secret_field(state: &Entity<InputState>) -> Input {
    Input::new(state).mask_toggle()
}

/// A search field: the magnifier in front, a clear button at the end.
pub fn search_field(state: &Entity<InputState>, cx: &App) -> Input {
    Input::new(state).cleanable(true).prefix(
        Ico::Search
            .icon()
            .small()
            .text_color(cx.theme().muted_foreground),
    )
}
