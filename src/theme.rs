//! Look and feel: flat, square, dense — after Zed's One Light / One Dark.
//! Appearance (light/dark/system, accent, size) is a user preference saved next to the app state.

use std::sync::Arc;

use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Small,
    #[default]
    Normal,
    Large,
}

impl Size {
    /// (UI font, code font). The UI font is also the rem, so spacing scales with it.
    fn fonts(self) -> (f32, f32) {
        match self {
            Size::Small => (13., 11.),
            Size::Normal => (14., 12.),
            Size::Large => (15., 13.),
        }
    }
}

/// Accent presets: (name, light-mode color, dark-mode color).
pub const ACCENTS: &[(&str, u32, u32)] = &[
    ("Blue", 0x4F6FD8, 0x74A5F2),
    ("Violet", 0x7A5BD0, 0xA98BF0),
    ("Green", 0x3F8A50, 0x86C08A),
    ("Teal", 0x16848A, 0x5FC0C4),
    ("Amber", 0xB7700F, 0xE2B05A),
    ("Coral", 0xCF5A3C, 0xEE8C6C),
    ("Rose", 0xC2457A, 0xEC86AE),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub mode: Mode,
    pub accent: usize,
    pub size: Size,
    /// Sidebar rows as their titles only, closer together (the default: the counts and
    /// addresses are rarely needed and pull the eye).
    pub compact_sidebar: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            accent: 0,
            size: Size::default(),
            compact_sidebar: true,
        }
    }
}

fn prefs_path() -> std::path::PathBuf {
    crate::connection::state_dir().join("appearance.json")
}

impl Appearance {
    pub fn load() -> Self {
        std::fs::read(prefs_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::connection::state_dir());
        let _ = std::fs::write(
            prefs_path(),
            serde_json::to_vec_pretty(self).unwrap_or_default(),
        );
    }

    /// Dark or not, resolving `System` from the window (or the app, before there is one).
    pub fn is_dark(&self, window: Option<&Window>, cx: &App) -> bool {
        match self.mode {
            Mode::Light => false,
            Mode::Dark => true,
            Mode::System => matches!(
                window
                    .map(|w| w.appearance())
                    .unwrap_or_else(|| cx.window_appearance()),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        }
    }
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn ca(hex: u32, alpha: f32) -> Hsla {
    let mut h: Hsla = rgb(hex).into();
    h.a = alpha;
    h
}

fn shade(color: Hsla, dl: f32) -> Hsla {
    Hsla {
        l: (color.l + dl).clamp(0., 1.),
        ..color
    }
}

/// Apply the appearance to every window.
pub fn apply(a: Appearance, window: Option<&Window>, cx: &mut App) {
    let dark = a.is_dark(window, cx);
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let highlight: Option<HighlightTheme> = serde_json::from_str(if dark {
        include_str!("../assets/themes/one-dark.json")
    } else {
        include_str!("../assets/themes/one-light.json")
    })
    .ok();
    let (accent_light, accent_dark) = ACCENTS
        .get(a.accent)
        .map(|(_, l, d)| (*l, *d))
        .unwrap_or((ACCENTS[0].1, ACCENTS[0].2));
    let (ui, mono) = a.size.fonts();

    Theme::update(cx, |t| {
        t.radius = px(0.);
        t.radius_lg = px(0.);
        t.shadow = false;
        t.font_size = px(ui);
        t.mono_font_size = px(mono);
        if let Some(h) = highlight {
            t.highlight_theme = Arc::new(h);
        }
        if dark {
            dark_palette(t);
        } else {
            light_palette(t);
        }

        let accent = c(if dark { accent_dark } else { accent_light });
        let hover = if dark { 0.06 } else { -0.05 };
        t.primary = accent;
        t.primary_hover = shade(accent, hover);
        t.primary_active = shade(accent, hover * 2.);
        t.primary_foreground = if dark { c(0x1E2127) } else { c(0xFFFFFF) };
        t.ring = accent;
        t.link = accent;
        t.caret = accent;
        t.selection = Hsla { a: 0.25, ..accent };
        t.list_active = Hsla {
            a: if dark { 0.22 } else { 0.14 },
            ..accent
        };
    });
}

fn light_palette(t: &mut Theme) {
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
    t.popover = c(0xFAFAFA);
    t.popover_foreground = c(0x242529);
    t.scrollbar_thumb = ca(0x242529, 0.18);
    t.scrollbar_thumb_hover = ca(0x242529, 0.3);

    t.danger = c(0xC9503F);
    t.danger_foreground = c(0xFFFFFF);
    t.success = c(0x4F8A3F);
    t.warning = c(0xB7791F);
}

fn dark_palette(t: &mut Theme) {
    // Zed One Dark: editor #282C33, panels #2F343E, chrome #3B414D.
    t.background = c(0x282C33);
    t.foreground = c(0xDCE0E5);
    t.sidebar = c(0x2F343E);
    t.sidebar_foreground = c(0xDCE0E5);
    t.sidebar_border = c(0x464B57);
    t.sidebar_accent = c(0x363C46);
    t.sidebar_accent_foreground = c(0xDCE0E5);
    t.title_bar = c(0x3B414D);
    t.title_bar_border = c(0x464B57);
    t.status_bar = c(0x3B414D);
    t.status_bar_border = c(0x464B57);
    t.tab_bar = c(0x3B414D);
    t.border = c(0x464B57);
    t.input = c(0x535966);
    t.window_border = c(0x535966);

    t.muted = c(0x333842);
    t.muted_foreground = c(0xA9AFBC);
    t.secondary = c(0x363C46);
    t.secondary_hover = c(0x3E4451);
    t.secondary_active = c(0x454A56);
    t.secondary_foreground = c(0xDCE0E5);
    t.accent = c(0x3E4451);
    t.accent_foreground = c(0xDCE0E5);
    t.list_hover = c(0x363C46);
    t.popover = c(0x2F343E);
    t.popover_foreground = c(0xDCE0E5);
    t.scrollbar_thumb = ca(0xDCE0E5, 0.18);
    t.scrollbar_thumb_hover = ca(0xDCE0E5, 0.3);

    t.danger = c(0xD07277);
    t.danger_foreground = c(0x1E2127);
    t.success = c(0xA1C181);
    t.warning = c(0xDEC184);
}
