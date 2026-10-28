//! Bench's colours: one pair of `Palette`s — the surfaces of its light and dark half — and the
//! two syntax themes (`assets/themes/bench-*.json`) that go with them. Light or dark is the
//! person's Mode (macOS's by default); the accent, the fonts and the shape are `super`'s.

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

/// Bench's own two halves, one look: white pages on a light chrome, hairline borders a shade
/// off the page; the dark half the same steps in graphite. Light or dark follows macOS (or the
/// person's Mode); nothing else is chosen.
const BENCH_LIGHT: Palette = Palette {
    background: 0xFFFFFF,
    foreground: 0x1B1C1F,
    sidebar: 0xF6F6F7,
    sidebar_foreground: 0x1B1C1F,
    sidebar_border: 0xE2E3E7,
    sidebar_accent: 0xE6E7EB,
    sidebar_accent_foreground: 0x1B1C1F,
    chrome: 0xF6F6F7,
    chrome_border: 0xE2E3E7,
    border: 0xE2E3E7,
    input: 0xDCDDE2,
    window_border: 0xD4D5DA,
    muted: 0xF3F3F5,
    muted_foreground: 0x5D6068,
    secondary: 0xF3F3F5,
    secondary_hover: 0xEDEDF0,
    secondary_active: 0xE6E7EB,
    secondary_foreground: 0x1B1C1F,
    accent: 0xE6E7EB,
    accent_foreground: 0x1B1C1F,
    list_hover: 0xEDEDF0,
    popover: 0xFFFFFF,
    popover_foreground: 0x1B1C1F,
    thumb: 0x1B1C1F,
    danger: 0xC93C3C,
    danger_foreground: 0xFFFFFF,
    success: 0x1E8A4C,
    warning: 0xB7791F,
};

const BENCH_DARK: Palette = Palette {
    background: 0x1A1B1F,
    foreground: 0xE4E5E8,
    sidebar: 0x202126,
    sidebar_foreground: 0xE4E5E8,
    sidebar_border: 0x303238,
    sidebar_accent: 0x30333A,
    sidebar_accent_foreground: 0xE4E5E8,
    chrome: 0x202126,
    chrome_border: 0x303238,
    border: 0x303238,
    input: 0x3A3C43,
    window_border: 0x101114,
    muted: 0x25272C,
    muted_foreground: 0xA0A3AB,
    secondary: 0x25272C,
    secondary_hover: 0x2A2C31,
    secondary_active: 0x30333A,
    secondary_foreground: 0xE4E5E8,
    accent: 0x30333A,
    accent_foreground: 0xE4E5E8,
    list_hover: 0x2A2C31,
    popover: 0x25272C,
    popover_foreground: 0xE4E5E8,
    thumb: 0xE4E5E8,
    danger: 0xE5736F,
    danger_foreground: 0x1A1B1F,
    success: 0x6CC28A,
    warning: 0xE5B95C,
};

/// The one scheme. A list still, so a saved name of an older scheme falls back to it (`find`).
pub const SCHEMES: &[Scheme] = &[Scheme {
    name: "Bench",
    halves: "Light and Dark",
    swatch: 0x2F6BED,
    light: BENCH_LIGHT,
    dark: BENCH_DARK,
    syntax_light: syntax!("bench-light"),
    syntax_dark: syntax!("bench-dark"),
}];

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
                // Every role the kit's `SyntaxColors` has: a role a theme leaves out is not
                // "less colourful", it is the kit's own default colour under this theme.
                const ROLES: [&str; 41] = [
                    "attribute",
                    "boolean",
                    "comment",
                    "comment_doc",
                    "constant",
                    "constructor",
                    "embedded",
                    "emphasis",
                    "emphasis.strong",
                    "enum",
                    "function",
                    "hint",
                    "keyword",
                    "label",
                    "link_text",
                    "link_uri",
                    "number",
                    "operator",
                    "predictive",
                    "preproc",
                    "primary",
                    "property",
                    "punctuation",
                    "punctuation.bracket",
                    "punctuation.delimiter",
                    "punctuation.list_marker",
                    "punctuation.special",
                    "string",
                    "string.escape",
                    "string.regex",
                    "string.special",
                    "string.special.symbol",
                    "tag",
                    "tag.doctype",
                    "text.code.span",
                    "text.literal",
                    "title",
                    "type",
                    "variable",
                    "variable.special",
                    "variant",
                ];
                for role in ROLES {
                    assert!(roles.contains_key(role), "{} ({half}): {role}", s.name);
                }
                assert_eq!(roles.len(), ROLES.len(), "{} ({half})", s.name);
                // The status colours (the editor's diagnostics, a field's error) come from the
                // theme too; the kit blends their backgrounds and borders itself.
                for status in ["error", "warning", "success", "info", "hint"] {
                    assert!(
                        v["style"][status].is_string(),
                        "{} ({half}): {status}",
                        s.name
                    );
                }
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
