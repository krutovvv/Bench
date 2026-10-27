//! The colour themes the person picks in the gear. Each is a pair of `Palette`s — the surfaces
//! of its light and dark half — and the two syntax themes (`assets/themes/*.json`) that go with
//! them. The accent, the fonts and the shape are not part of a theme: they stay the person's own
//! choice in `super::Appearance`.

use gpui_kit::component::Theme;

/// The surfaces one mode of a theme owns: the chrome, the fields, the lists, the states. Named
/// as `gpui_component::Theme` names them, so painting them is one line each.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: u32,
    pub foreground: u32,
    pub sidebar: u32,
    pub sidebar_foreground: u32,
    pub sidebar_border: u32,
    pub sidebar_accent: u32,
    pub sidebar_accent_foreground: u32,
    /// The title bar, the status bar and the tab bar: one band, one rule against the page.
    pub chrome: u32,
    pub chrome_border: u32,
    pub border: u32,
    pub input: u32,
    pub window_border: u32,
    pub muted: u32,
    pub muted_foreground: u32,
    pub secondary: u32,
    pub secondary_hover: u32,
    pub secondary_active: u32,
    pub secondary_foreground: u32,
    /// The hovered/selected row's surface (not the accent hue: that is the person's choice).
    pub accent: u32,
    pub accent_foreground: u32,
    pub list_hover: u32,
    pub popover: u32,
    pub popover_foreground: u32,
    /// The scrollbar's thumb (drawn semi-transparent over the surface it is on).
    pub thumb: u32,
    pub danger: u32,
    pub danger_foreground: u32,
    pub success: u32,
    pub warning: u32,
}

/// A colour theme as the gear shows it.
pub struct Scheme {
    /// Its name on the chip.
    pub name: &'static str,
    /// What the two halves are called, in the hint under the chips.
    pub halves: &'static str,
    /// The colour that stands for it on its chip.
    pub swatch: u32,
    pub light: Palette,
    pub dark: Palette,
    /// The editor's and code blocks' colours: the JSON of the half it goes with.
    pub syntax_light: &'static str,
    pub syntax_dark: &'static str,
}

impl Scheme {
    pub fn pick(&self, dark: bool) -> &Palette {
        if dark { &self.dark } else { &self.light }
    }
}

/// A syntax theme's JSON, in the binary (the person never sees these files).
macro_rules! syntax {
    ($name:literal) => {
        include_str!(concat!("../../../../assets/themes/", $name, ".json"))
    };
}

/// Bench's own look, after Zed's One Light / One Dark: flat, square, a step under the loudest
/// themes. The first of `SCHEMES`, so it is the default and what a theme without a light half
/// (Dracula) borrows for its light mode.
const BENCH_LIGHT: Palette = Palette {
    background: 0xFAFAFA,
    foreground: 0x242529,
    sidebar: 0xEAEAEC,
    sidebar_foreground: 0x242529,
    sidebar_border: 0xD9D9DA,
    sidebar_accent: 0xDEDEE1,
    sidebar_accent_foreground: 0x242529,
    chrome: 0xE4E4E6,
    chrome_border: 0xD9D9DA,
    border: 0xD9D9DA,
    input: 0xC9C9CB,
    window_border: 0xC9C9CB,
    muted: 0xEBEBEC,
    muted_foreground: 0x6E6F75,
    secondary: 0xEDEDEE,
    secondary_hover: 0xE2E2E4,
    secondary_active: 0xD6D6D8,
    secondary_foreground: 0x242529,
    accent: 0xE2E2E4,
    accent_foreground: 0x242529,
    list_hover: 0xE6E6E8,
    popover: 0xFAFAFA,
    popover_foreground: 0x242529,
    thumb: 0x242529,
    danger: 0xC9503F,
    danger_foreground: 0xFFFFFF,
    success: 0x4F8A3F,
    warning: 0xB7791F,
};

const BENCH_DARK: Palette = Palette {
    background: 0x282C33,
    foreground: 0xDCE0E5,
    sidebar: 0x2F343E,
    sidebar_foreground: 0xDCE0E5,
    sidebar_border: 0x464B57,
    sidebar_accent: 0x363C46,
    sidebar_accent_foreground: 0xDCE0E5,
    chrome: 0x3B414D,
    chrome_border: 0x464B57,
    border: 0x464B57,
    input: 0x535966,
    window_border: 0x535966,
    muted: 0x333842,
    muted_foreground: 0xA9AFBC,
    secondary: 0x363C46,
    secondary_hover: 0x3E4451,
    secondary_active: 0x454A56,
    secondary_foreground: 0xDCE0E5,
    accent: 0x3E4451,
    accent_foreground: 0xDCE0E5,
    list_hover: 0x363C46,
    popover: 0x2F343E,
    popover_foreground: 0xDCE0E5,
    thumb: 0xDCE0E5,
    danger: 0xD07277,
    danger_foreground: 0x1E2127,
    success: 0xA1C181,
    warning: 0xDEC184,
};

/// One Dark Pro (dark) and One Light Pro (light), the VS Code family: the page a step lighter
/// than the chrome, and a near-black rule between the parts.
const ONE_LIGHT: Palette = Palette {
    background: 0xFAFAFA,
    foreground: 0x383A42,
    sidebar: 0xEAEAEB,
    sidebar_foreground: 0x383A42,
    sidebar_border: 0xD3D4D6,
    sidebar_accent: 0xE0E0E1,
    sidebar_accent_foreground: 0x383A42,
    chrome: 0xEAEAEB,
    chrome_border: 0xD3D4D6,
    border: 0xD3D4D6,
    input: 0xCDCED1,
    window_border: 0xCDCED1,
    muted: 0xEAEAEB,
    muted_foreground: 0x7A7F85,
    secondary: 0xEFEFEF,
    secondary_hover: 0xE4E4E6,
    secondary_active: 0xDADADC,
    secondary_foreground: 0x383A42,
    accent: 0xE4E4E6,
    accent_foreground: 0x383A42,
    list_hover: 0xF0F0F1,
    popover: 0xFFFFFF,
    popover_foreground: 0x383A42,
    thumb: 0x383A42,
    danger: 0xE45649,
    danger_foreground: 0xFFFFFF,
    success: 0x50A14F,
    warning: 0xC18401,
};

const ONE_DARK: Palette = Palette {
    background: 0x282C34,
    foreground: 0xABB2BF,
    sidebar: 0x21252B,
    sidebar_foreground: 0x9DA5B4,
    sidebar_border: 0x181A1F,
    sidebar_accent: 0x2C313C,
    sidebar_accent_foreground: 0xABB2BF,
    chrome: 0x21252B,
    chrome_border: 0x181A1F,
    border: 0x3E4451,
    input: 0x4B5263,
    window_border: 0x4B5263,
    muted: 0x2C313C,
    muted_foreground: 0x9DA5B4,
    secondary: 0x2C313C,
    secondary_hover: 0x333842,
    secondary_active: 0x3E4451,
    secondary_foreground: 0xABB2BF,
    accent: 0x3E4451,
    accent_foreground: 0xABB2BF,
    list_hover: 0x2C313C,
    popover: 0x21252B,
    popover_foreground: 0xABB2BF,
    thumb: 0xABB2BF,
    danger: 0xE06C75,
    danger_foreground: 0x1B1E24,
    success: 0x98C379,
    warning: 0xE5C07B,
};

/// Dracula. It has no light half of its own, so its light mode is Bench's.
const DRACULA: Palette = Palette {
    background: 0x282A36,
    foreground: 0xF8F8F2,
    sidebar: 0x21222C,
    sidebar_foreground: 0xF8F8F2,
    sidebar_border: 0x191A21,
    sidebar_accent: 0x343746,
    sidebar_accent_foreground: 0xF8F8F2,
    chrome: 0x21222C,
    chrome_border: 0x191A21,
    border: 0x44475A,
    input: 0x6272A2,
    window_border: 0x6272A2,
    muted: 0x343746,
    muted_foreground: 0x6272A2,
    secondary: 0x343746,
    secondary_hover: 0x44475A,
    secondary_active: 0x4E5166,
    secondary_foreground: 0xF8F8F2,
    accent: 0x44475A,
    accent_foreground: 0xF8F8F2,
    list_hover: 0x343746,
    popover: 0x21222C,
    popover_foreground: 0xF8F8F2,
    thumb: 0xF8F8F2,
    danger: 0xFF5555,
    danger_foreground: 0x21222C,
    success: 0x50FA7B,
    warning: 0xF1FA8C,
};

/// Visual Studio Code's own defaults: Dark+ and Light+.
const VS_LIGHT: Palette = Palette {
    background: 0xFFFFFF,
    foreground: 0x1F1F1F,
    sidebar: 0xF3F3F3,
    sidebar_foreground: 0x1F1F1F,
    sidebar_border: 0xE7E7E7,
    sidebar_accent: 0xE8E8E8,
    sidebar_accent_foreground: 0x1F1F1F,
    chrome: 0xF3F3F3,
    chrome_border: 0xD7D7D7,
    border: 0xD7D7D7,
    input: 0xC8C8C8,
    window_border: 0xC8C8C8,
    muted: 0xF5F5F5,
    muted_foreground: 0x6C6C6D,
    secondary: 0xF0F0F0,
    secondary_hover: 0xE8E8E8,
    secondary_active: 0xDADADA,
    secondary_foreground: 0x1F1F1F,
    accent: 0xADD6FF,
    accent_foreground: 0x1F1F1F,
    list_hover: 0xF0F0F0,
    popover: 0xFFFFFF,
    popover_foreground: 0x1F1F1F,
    thumb: 0x1F1F1F,
    danger: 0xCD3131,
    danger_foreground: 0xFFFFFF,
    success: 0x107C10,
    warning: 0xE0A622,
};

const VS_DARK: Palette = Palette {
    background: 0x1E1E1E,
    foreground: 0xD4D4D4,
    sidebar: 0x252526,
    sidebar_foreground: 0xCCCCCC,
    sidebar_border: 0x333333,
    sidebar_accent: 0x2A2D2E,
    sidebar_accent_foreground: 0xD4D4D4,
    chrome: 0x2D2D30,
    chrome_border: 0x3F3F46,
    border: 0x454545,
    input: 0x616161,
    window_border: 0x616161,
    muted: 0x2D2D30,
    muted_foreground: 0x9D9D9D,
    secondary: 0x2D2D30,
    secondary_hover: 0x3A3D41,
    secondary_active: 0x3F3F46,
    secondary_foreground: 0xCCCCCC,
    accent: 0x094771,
    accent_foreground: 0xFFFFFF,
    list_hover: 0x2A2D2E,
    popover: 0x252526,
    popover_foreground: 0xCCCCCC,
    thumb: 0xD4D4D4,
    danger: 0xF44747,
    danger_foreground: 0x1E1E1E,
    success: 0x73C991,
    warning: 0xCCA700,
};

/// JetBrains: Darcula and IntelliJ Light.
const DARCUDA: Palette = Palette {
    background: 0x2B2B2B,
    foreground: 0xBBBBBB,
    sidebar: 0x3C3F41,
    sidebar_foreground: 0xBBBBBB,
    sidebar_border: 0x4E5254,
    sidebar_accent: 0x45494A,
    sidebar_accent_foreground: 0xBBBBBB,
    chrome: 0x3C3F41,
    chrome_border: 0x4E5254,
    border: 0x4E5254,
    input: 0x5E6060,
    window_border: 0x5E6060,
    muted: 0x3C3F41,
    muted_foreground: 0x808080,
    secondary: 0x3C3F41,
    secondary_hover: 0x45494A,
    secondary_active: 0x4E5254,
    secondary_foreground: 0xBBBBBB,
    accent: 0x214283,
    accent_foreground: 0xFFFFFF,
    list_hover: 0x3C3F41,
    popover: 0x2B2B2B,
    popover_foreground: 0xBBBBBB,
    thumb: 0xBBBBBB,
    danger: 0xFF6B68,
    danger_foreground: 0x2B2B2B,
    success: 0x6A8759,
    warning: 0xBBB529,
};

const INTELLIJ_LIGHT: Palette = Palette {
    background: 0xFFFFFF,
    foreground: 0x000000,
    sidebar: 0xF2F2F2,
    sidebar_foreground: 0x000000,
    sidebar_border: 0xD4D4D4,
    sidebar_accent: 0xE8E8E8,
    sidebar_accent_foreground: 0x000000,
    chrome: 0xF2F2F2,
    chrome_border: 0xC0C0C0,
    border: 0xD4D4D4,
    input: 0xB3B3B3,
    window_border: 0x8C8C8C,
    muted: 0xF2F2F2,
    muted_foreground: 0x6E6E6E,
    secondary: 0xF5F5F5,
    secondary_hover: 0xE8E8E8,
    secondary_active: 0xDCDCDC,
    secondary_foreground: 0x000000,
    accent: 0xCEEEFC,
    accent_foreground: 0x000000,
    list_hover: 0xF2F2F2,
    popover: 0xFFFFFF,
    popover_foreground: 0x000000,
    thumb: 0x000000,
    danger: 0xD80606,
    danger_foreground: 0xFFFFFF,
    success: 0x067D17,
    warning: 0x9E8400,
};

/// GitHub's own themes.
const GITHUB_LIGHT: Palette = Palette {
    background: 0xFFFFFF,
    foreground: 0x1F2328,
    sidebar: 0xF6F8FA,
    sidebar_foreground: 0x1F2328,
    sidebar_border: 0xD0D7DE,
    sidebar_accent: 0xEDEFF2,
    sidebar_accent_foreground: 0x1F2328,
    chrome: 0xF6F8FA,
    chrome_border: 0xD0D7DE,
    border: 0xD0D7DE,
    input: 0xAFBAC4,
    window_border: 0xAFBAC4,
    muted: 0xF6F8FA,
    muted_foreground: 0x656D76,
    secondary: 0xF6F8FA,
    secondary_hover: 0xEDEFF2,
    secondary_active: 0xE7EBEF,
    secondary_foreground: 0x1F2328,
    accent: 0xDDF4FF,
    accent_foreground: 0x1F2328,
    list_hover: 0xF6F8FA,
    popover: 0xFFFFFF,
    popover_foreground: 0x1F2328,
    thumb: 0x1F2328,
    danger: 0xCF222E,
    danger_foreground: 0xFFFFFF,
    success: 0x1A7F3B,
    warning: 0x9A6700,
};

const GITHUB_DARK: Palette = Palette {
    background: 0x0D1117,
    foreground: 0xC9D1D9,
    sidebar: 0x161B22,
    sidebar_foreground: 0xC9D1D9,
    sidebar_border: 0x30363D,
    sidebar_accent: 0x21262D,
    sidebar_accent_foreground: 0xC9D1D9,
    chrome: 0x161B22,
    chrome_border: 0x30363D,
    border: 0x30363D,
    input: 0x484F58,
    window_border: 0x484F58,
    muted: 0x161B22,
    muted_foreground: 0x8B949E,
    secondary: 0x21262D,
    secondary_hover: 0x30363D,
    secondary_active: 0x3D4452,
    secondary_foreground: 0xC9D1D9,
    accent: 0x162945, // #388BFD at a fifth over the page
    accent_foreground: 0xC9D1D9,
    list_hover: 0x161B22,
    popover: 0x161B22,
    popover_foreground: 0xC9D1D9,
    thumb: 0xC9D1D9,
    danger: 0xF85149,
    danger_foreground: 0x0D1117,
    success: 0x3FB950,
    warning: 0xD29922,
};

/// The gear's list, in the order it shows them. The first is the default.
pub const SCHEMES: &[Scheme] = &[
    Scheme {
        name: "Bench",
        halves: "the built-in light and dark",
        swatch: 0x4F6FD8,
        light: BENCH_LIGHT,
        dark: BENCH_DARK,
        syntax_light: syntax!("one-light"),
        syntax_dark: syntax!("one-dark"),
    },
    Scheme {
        name: "One Dark Pro",
        halves: "One Dark Pro and One Light Pro",
        swatch: 0x61AFEF,
        light: ONE_LIGHT,
        dark: ONE_DARK,
        syntax_light: syntax!("one-light-pro"),
        syntax_dark: syntax!("one-dark-pro"),
    },
    Scheme {
        name: "Dracula",
        halves: "Dracula dark, Bench light",
        swatch: 0xBD93F9,
        light: BENCH_LIGHT,
        dark: DRACULA,
        syntax_light: syntax!("one-light"),
        syntax_dark: syntax!("dracula"),
    },
    Scheme {
        name: "VS Code",
        halves: "Dark+ and Light+",
        swatch: 0x007ACC,
        light: VS_LIGHT,
        dark: VS_DARK,
        syntax_light: syntax!("vs-light"),
        syntax_dark: syntax!("vs-dark"),
    },
    Scheme {
        name: "JetBrains",
        halves: "Darcula and IntelliJ Light",
        swatch: 0xCC7832,
        light: INTELLIJ_LIGHT,
        dark: DARCUDA,
        syntax_light: syntax!("intellij-light"),
        syntax_dark: syntax!("darcula"),
    },
    Scheme {
        name: "GitHub",
        halves: "GitHub Dark and Light",
        swatch: 0x58A6FF,
        light: GITHUB_LIGHT,
        dark: GITHUB_DARK,
        syntax_light: syntax!("github-light"),
        syntax_dark: syntax!("github-dark"),
    },
];

/// The scheme called `name`; the first when the name is unknown (an older file, a new theme).
pub fn find(name: &str) -> &'static Scheme {
    SCHEMES
        .iter()
        .find(|s| s.name == name)
        .unwrap_or(&SCHEMES[0])
}

/// Paint a palette over the theme's colours. Everything a theme does not own (the accent hue,
/// the fonts, the radius) is left to `super::apply`.
pub fn paint(t: &mut Theme, p: &Palette, dark: bool) {
    let c = |hex: u32| gpui_kit::rgb(hex).into();
    let ca = |hex: u32, a: f32| {
        let mut h: gpui_kit::Hsla = gpui_kit::rgb(hex).into();
        h.a = a;
        h
    };
    t.background = c(p.background);
    t.foreground = c(p.foreground);
    t.sidebar = c(p.sidebar);
    t.sidebar_foreground = c(p.sidebar_foreground);
    t.sidebar_border = c(p.sidebar_border);
    t.sidebar_accent = c(p.sidebar_accent);
    t.sidebar_accent_foreground = c(p.sidebar_accent_foreground);
    t.title_bar = c(p.chrome);
    t.title_bar_border = c(p.chrome_border);
    t.status_bar = c(p.chrome);
    t.status_bar_border = c(p.chrome_border);
    t.tab_bar = c(p.chrome);
    t.border = c(p.border);
    t.input = c(p.input);
    t.window_border = c(p.window_border);
    t.muted = c(p.muted);
    t.muted_foreground = c(p.muted_foreground);
    t.secondary = c(p.secondary);
    t.secondary_hover = c(p.secondary_hover);
    t.secondary_active = c(p.secondary_active);
    t.secondary_foreground = c(p.secondary_foreground);
    t.accent = c(p.accent);
    t.accent_foreground = c(p.accent_foreground);
    t.list_hover = c(p.list_hover);
    t.popover = c(p.popover);
    t.popover_foreground = c(p.popover_foreground);
    t.scrollbar_thumb = ca(p.thumb, 0.18);
    t.scrollbar_thumb_hover = ca(p.thumb, 0.3);
    t.danger = c(p.danger);
    t.danger_foreground = c(p.danger_foreground);
    t.success = c(p.success);
    t.warning = c(p.warning);
    // The text on an accent button: the page's own colour in the dark half, white in the light.
    t.primary_foreground = if dark { c(p.background) } else { c(0xFFFFFF) };
    t.button_primary_foreground = t.primary_foreground;
}
#[cfg(test)]
mod test {
    use super::*;
    use gpui_kit::component::highlighter::HighlightTheme;

    /// A syntax theme the kit cannot read is a code block without colours, and nothing else says
    /// so: the gear would ship the chip blind.
    #[test]
    fn every_scheme_ships_the_syntax_themes_of_both_halves() {
        for s in SCHEMES {
            assert_ne!(s.dark.background, s.light.background, "{}", s.name);
            for (json, half) in [(s.syntax_dark, "dark"), (s.syntax_light, "light")] {
                let v: serde_json::Value = serde_json::from_str(json)
                    .unwrap_or_else(|e| panic!("{} ({half}): {e}", s.name));
                assert_eq!(v["appearance"].as_str(), Some(half), "{}", s.name);
                let roles = v["style"]["syntax"].as_object().expect("roles");
                assert!(roles.len() >= 30, "{}: {} roles", s.name, roles.len());
                let _: HighlightTheme = serde_json::from_str(json)
                    .unwrap_or_else(|e| panic!("{} ({half}) for the kit: {e}", s.name));
            }
        }
    }

    /// The two halves are one look: an editor that disagrees with the surfaces around it is what
    /// the person notices first.
    #[test]
    fn the_editor_of_each_half_sits_on_that_half() {
        for s in SCHEMES {
            for (json, p) in [(s.syntax_dark, &s.dark), (s.syntax_light, &s.light)] {
                let v: serde_json::Value = serde_json::from_str(json).expect("json");
                let bg = v["style"]["editor.background"].as_str().expect("bg");
                assert_eq!(
                    bg.to_ascii_uppercase(),
                    format!("#{:06X}", p.background),
                    "{}",
                    s.name
                );
            }
        }
    }

    /// Two chips with one name would leave the person guessing which is which.
    #[test]
    fn scheme_names_are_each_their_own() {
        let mut names: Vec<&str> = SCHEMES.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SCHEMES.len());
    }

    #[test]
    fn an_unknown_name_is_the_first_scheme() {
        assert_eq!(find("").name, SCHEMES[0].name);
        assert_eq!(find("Krauti").name, SCHEMES[0].name);
    }
}
