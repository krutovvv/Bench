//! Look and feel: flat, square, dense. Three things the person owns, in `Appearance` and saved
//! next to the app state: the half of the day (`Mode`: light, dark, System), the colour theme
//! (`Scheme`s in `palettes`, what the gear calls Theme) and the accent, the size and the sidebar.
//! A theme paints the surfaces and the editor's syntax; the accent, the fonts and the radius are
//! the same under every theme.

pub mod palettes;

use std::sync::Arc;

use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Light,
    Dark,
    /// macOS's own: Bench turns dark when the Mac does.
    #[default]
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

/// The accent: (name, light-mode color, dark-mode color). One, Bench's blue; the person
/// chooses light or dark, not a hue.
pub const ACCENTS: &[(&str, u32, u32)] = &[("Blue", 0x2F6BED, 0x5B9BFF)];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub mode: Mode,
    /// The colour theme: a name from `palettes::SCHEMES` ("" is the first of them).
    pub scheme: String,
    pub accent: usize,
    pub size: Size,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            scheme: palettes::SCHEMES[0].name.into(),
            accent: 0,
            size: Size::default(),
        }
    }
}

impl Appearance {
    /// Read `appearance.json` in `dir`; the default when it is missing or unreadable.
    pub fn load_from(dir: &std::path::Path) -> Self {
        std::fs::read(dir.join("appearance.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, dir: &std::path::Path) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(
            dir.join("appearance.json"),
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

    /// The chosen colour theme (an unknown name falls back to the first).
    pub fn scheme(&self) -> &'static palettes::Scheme {
        palettes::find(&self.scheme)
    }
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn shade(color: Hsla, dl: f32) -> Hsla {
    Hsla {
        l: (color.l + dl).clamp(0., 1.),
        ..color
    }
}

/// The monospace font for code, terminals and paths: Menlo on the desktop, the built-in Fira
/// Mono in the browser build (no system fonts there).
pub const MONO: &str = if cfg!(target_family = "wasm") {
    "Fira Mono"
} else {
    "Menlo"
};

/// Apply the appearance to every window.
pub fn apply(a: &Appearance, window: Option<&Window>, cx: &mut App) {
    let dark = a.is_dark(window, cx);
    let scheme = a.scheme();
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
        scheme.syntax_dark
    } else {
        scheme.syntax_light
    })
    .ok();
    let (accent_light, accent_dark) = ACCENTS
        .get(a.accent)
        .map(|(_, l, d)| (*l, *d))
        .unwrap_or((ACCENTS[0].1, ACCENTS[0].2));
    let (ui, mono) = a.size.fonts();

    Theme::update(cx, |t| {
        // The browser has no system fonts: the built-in ones (main.rs adds them).
        if cfg!(target_family = "wasm") {
            t.font_family = "Inter Variable".into();
            t.mono_font_family = MONO.into();
        }
        // Slightly round, the same everywhere (buttons, fields, menus; the blocks use rounded_md).
        t.radius = px(4.);
        t.radius_lg = px(6.);
        t.shadow = false;
        t.font_size = px(ui);
        t.mono_font_size = px(mono);
        if let Some(h) = highlight {
            t.highlight_theme = Arc::new(h);
        }
        palettes::paint(t, scheme.pick(dark), dark);

        let accent = c(if dark { accent_dark } else { accent_light });
        let hover = if dark { 0.06 } else { -0.05 };
        t.primary = accent;
        t.primary_hover = shade(accent, hover);
        t.primary_active = shade(accent, hover * 2.);
        // Primary buttons (Save, Send, …) in the accent too, not the kit's black.
        t.button_primary = t.primary;
        t.button_primary_hover = t.primary_hover;
        t.button_primary_active = t.primary_active;
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
