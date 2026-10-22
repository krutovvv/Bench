//! Look and feel: flat, square, dense — after Zed's One Light.

use gpui_kit::component::Theme;
use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::*;

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn ca(hex: u32, alpha: f32) -> Hsla {
    let mut h: Hsla = rgb(hex).into();
    h.a = alpha;
    h
}

pub fn apply(cx: &mut App) {
    let t = Theme::global_mut(cx);
    t.radius = px(0.);
    t.radius_lg = px(0.);
    t.shadow = false;
    t.font_size = px(14.);
    t.mono_font_size = px(12.);
    // One Light syntax colors for every editor and diff (config, MCP, skills, files).
    if let Ok(h) =
        serde_json::from_str::<HighlightTheme>(include_str!("../assets/themes/one-light.json"))
    {
        t.highlight_theme = std::sync::Arc::new(h);
    }

    // Surfaces: editor-white content, one step darker chrome, thin neutral borders.
    t.background = c(0xFAFAFA);
    t.foreground = c(0x242529);
    t.sidebar = c(0xEFEFF0);
    t.sidebar_foreground = c(0x242529);
    t.sidebar_border = c(0xD9D9DA);
    t.sidebar_accent = c(0xE2E2E4);
    t.sidebar_accent_foreground = c(0x242529);
    t.title_bar = c(0xE9E9EA);
    t.title_bar_border = c(0xD9D9DA);
    t.status_bar = c(0xE9E9EA);
    t.status_bar_border = c(0xD9D9DA);
    t.tab_bar = c(0xE9E9EA);
    t.border = c(0xD9D9DA);
    t.input = c(0xC9C9CB);
    t.window_border = c(0xC9C9CB);

    t.muted = c(0xEBEBEC);
    t.muted_foreground = c(0x6E6F75);
    t.secondary = c(0xEDEDEE);
    t.secondary_hover = c(0xE2E2E4);
    t.secondary_active = c(0xD6D6D8);
    t.secondary_foreground = c(0x242529);
    t.accent = c(0xE2E2E4);
    t.accent_foreground = c(0x242529);
    t.list_hover = c(0xE6E6E8);
    t.list_active = c(0xDCDDE6);
    t.popover = c(0xFAFAFA);
    t.popover_foreground = c(0x242529);

    // One accent: Zed blue.
    t.primary = c(0x4F6FD8);
    t.primary_hover = c(0x4462CC);
    t.primary_active = c(0x3A57BF);
    t.primary_foreground = c(0xFFFFFF);
    t.ring = c(0x4F6FD8);
    t.link = c(0x3F5FCF);
    t.selection = ca(0x4F6FD8, 0.22);
    t.caret = c(0x4F6FD8);
    t.scrollbar_thumb = ca(0x242529, 0.18);
    t.scrollbar_thumb_hover = ca(0x242529, 0.3);

    t.danger = c(0xC9503F);
    t.danger_foreground = c(0xFFFFFF);
    t.success = c(0x4F8A3F);
    t.warning = c(0xB7791F);
}
