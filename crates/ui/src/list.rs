//! Lists: group headers, rows, the guide that nests rows under a group, file icons.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::nest_pl;

/// A head's icon in a fixed 16 px square, so names after icons of any drawing start at one x.
pub fn head_icon(icon: Ico, cx: &App) -> impl IntoElement {
    head_icon_in(icon, None, cx)
}

/// `head_icon` in a color of the person's choosing (a marked project), else muted.
pub fn head_icon_in(icon: Ico, color: Option<Hsla>, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            icon.icon()
                .small()
                .text_color(color.unwrap_or(cx.theme().muted_foreground)),
        )
}

/// A state's dot in `color` (an agent up, a terminal running, a thread waiting for an answer).
pub fn dot(color: Hsla) -> Div {
    div().flex_none().size(px(crate::metrics::DOT)).rounded_full().bg(color)
}

/// The room a head's "+" takes, kept when it has none, so every chevron sits at one x.
pub fn head_plus_slot() -> Div {
    div()
        .flex_none()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
}

/// A group's fold chevron: always shown when folded, on hover of `group` when open.
pub fn group_chevron(collapsed: bool, group: SharedString, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .p_0p5()
        .text_color(cx.theme().muted_foreground)
        .opacity(if collapsed { 1. } else { 0. })
        .group_hover(group, |s| s.opacity(1.))
        .child(
            if collapsed {
                Ico::ChevronRight
            } else {
                Ico::ChevronDown
            }
            .icon()
            .xsmall(),
        )
}

/// The 1 px guide under a group header's icon; in the group's `last` row it stops at the text.
pub fn nest_guide(cx: &App, last: bool) -> impl IntoElement {
    div()
        .absolute()
        .top_0()
        .when(last, |d| d.bottom(cx.theme().font_size * 0.5))
        .when(!last, |d| d.bottom_0())
        .left(cx.theme().font_size * 1.5 - px(2.5))
        .w(px(1.))
        .bg(cx.theme().border)
}

/// A list row: title, optional muted meta, a 2 px accent stripe when selected, `actions` shown on
/// hover. `nested`: under a group, `Some(is_last)`.
pub fn list_row(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    meta: Option<SharedString>,
    selected: bool,
    nested: Option<bool>,
    actions: Option<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-row").into();
    h_flex()
        .id(ElementId::Name(id))
        .group(group.clone())
        .relative()
        .gap_1()
        .pl(if nested.is_some() {
            nest_pl(cx)
        } else {
            px(14.)
        })
        .pr_3()
        .py_2()
        .cursor_pointer()
        .border_l_2()
        .border_color(if selected {
            theme.primary
        } else {
            theme.transparent
        })
        .when(selected, |d| d.bg(theme.sidebar_accent))
        .hover(|s| s.bg(theme.sidebar_accent))
        .when_some(nested, |d, last| d.child(nest_guide(cx, last)))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_sm().truncate().child(title.into()))
                .when_some(meta, |d, meta| {
                    d.child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(meta),
                    )
                }),
        )
        .when_some(actions, |d, actions| {
            d.child(
                div()
                    .flex_none()
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(actions),
            )
        })
}

/// A row of a picker inside a block (a server, a host): icon, label, muted detail.
pub fn pick_row(
    id: impl Into<SharedString>,
    icon: Ico,
    label: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    h_flex()
        .id(ElementId::Name(id.into()))
        .px_2()
        .py_1()
        .gap_2()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(icon.icon().small().text_color(theme.muted_foreground))
        .child(div().flex_none().child(label.into()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(detail.into()),
        )
}

/// A whole answer of its own in a card (a `clarify` choice): as wide as the card, its text
/// wrapped over lines. A button cannot be this: its label is one line, and it would push the
/// card past the thread's column.
pub fn choice_row(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .w_full()
        .min_w_0()
        .px_2p5()
        .py_1p5()
        .text_size(px(crate::metrics::THREAD_META))
        .rounded_md()
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|s| s.border_color(theme.primary))
}

/// A file's icon and color by its name, as an editor's tree shows them: by name when the name
/// itself says what the thing is (README, Cargo.toml, node_modules…), else by extension.
/// Editors keep a large collection of these; this is the useful core of one, in one place.
pub fn file_icon(name: &str, dir: bool, theme: &Theme) -> (Ico, Hsla) {
    let (ico, tone) = file_kind(&name.to_lowercase(), dir);
    let color = match tone {
        Tone::Red => theme.red,
        Tone::Yellow => theme.yellow,
        Tone::Blue => theme.blue,
        Tone::Magenta => theme.magenta,
        Tone::Cyan => theme.cyan,
        Tone::Green => theme.green,
        Tone::Muted => theme.muted_foreground,
    };
    (ico, color)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tone {
    Red,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Green,
    Muted,
}

/// The kind of a name: its icon and its tone (resolved against a theme by `file_icon`, so the
/// map itself is testable without one). `lower` is the name, already lowered.
fn file_kind(lower: &str, dir: bool) -> (Ico, Tone) {
    if dir {
        return match lower {
            // Third-party code: a setting away from the work.
            "node_modules" | "vendor" | "bower_components" | "deps" => {
                (Ico::FolderCog, Tone::Muted)
            }
            // The source lives here.
            "src" | "lib" | "libs" | "app" | "cmd" | "internal" => (Ico::FolderCode, Tone::Blue),
            "scripts" | "tools" => (Ico::FolderCode, Tone::Green),
            "test" | "tests" | "spec" | "specs" | "__tests__" | "e2e" => {
                (Ico::FolderGit, Tone::Green)
            }
            "docs" | "doc" | "wiki" | "guides" => (Ico::FolderTree, Tone::Blue),
            // What a build makes: never the work itself.
            "dist" | "build" | "out" | "target" | "obj" | ".next" | ".nuxt" | "coverage" => {
                (Ico::FolderArchive, Tone::Yellow)
            }
            "__pycache__" | ".venv" | "venv" | ".cache" | ".tox" | ".gradle" | ".terraform"
            | ".parcel-cache" | "tmp" | "temp" => (Ico::FolderLock, Tone::Muted),
            ".github" | ".gitlab" | ".circleci" | ".husky" | "ci" | "workflows" => {
                (Ico::FolderCog, Tone::Cyan)
            }
            // Pictures and what ships to the browser.
            "assets" | "static" | "public" | "images" | "img" | "media" | "resources" | "fonts"
            | "icons" | "theme" | "themes" => (Ico::Folder, Tone::Magenta),
            // A hidden folder is somebody else's bookkeeping.
            _ if lower.starts_with('.') => (Ico::Folder, Tone::Muted),
            _ => (Ico::Folder, Tone::Blue),
        };
    }
    // Names that say what the file is, wherever it lives.
    if lower.starts_with("readme") {
        return (Ico::FileText, Tone::Blue);
    }
    if lower.starts_with("changelog") || lower.starts_with("changes") {
        return (Ico::FileText, Tone::Cyan);
    }
    if lower.starts_with("contributing") {
        return (Ico::FileText, Tone::Muted);
    }
    if matches!(lower, "license" | "licence" | "copying")
        || lower.starts_with("license.")
        || lower.starts_with("licence.")
    {
        return (Ico::Scale, Tone::Muted);
    }
    // Words for the agent are its own kind of file.
    if matches!(lower, "agents.md" | "claude.md" | "system.md") {
        return (Ico::Bot, Tone::Cyan);
    }
    if lower == "skill.md" {
        return (Ico::Wrench, Tone::Green);
    }
    // A manifest names its project; a lock freezes it.
    if matches!(lower, "package.json" | "composer.json" | "deno.json")
        || lower.starts_with("package.")
    {
        return (Ico::Package, Tone::Yellow);
    }
    if matches!(
        lower,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "npm-shrinkwrap.json"
            | "composer.lock"
            | "cargo.lock"
            | "go.sum"
            | "uv.lock"
            | "poetry.lock"
            | "pipfile.lock"
            | "gemfile.lock"
    ) {
        return (Ico::FileLock, Tone::Muted);
    }
    if matches!(
        lower,
        "cargo.toml"
            | "go.mod"
            | "requirements.txt"
            | "pyproject.toml"
            | "setup.py"
            | "setup.cfg"
            | "gemfile"
            | "podfile"
            | "build.gradle"
            | "pom.xml"
            | "build.zig"
            | "meson.build"
    ) {
        return (Ico::FileCog, Tone::Red);
    }
    // How a thing is built or shipped.
    if matches!(
        lower,
        "makefile" | "gnumakefile" | "justfile" | "rakefile" | "jenkinsfile"
    ) || lower.starts_with("taskfile.")
    {
        return (Ico::FileTerminal, Tone::Green);
    }
    if matches!(lower, "dockerfile" | "containerfile") {
        return (Ico::FileCog, Tone::Cyan);
    }
    // A tool's own settings, by the tool's name.
    if lower.starts_with("tsconfig")
        || lower.starts_with("jsconfig")
        || lower.ends_with(".config.js")
        || lower.ends_with(".config.ts")
        || lower.ends_with(".config.json")
        || lower.ends_with(".config.mjs")
        || matches!(
            lower,
            "biome.json" | ".eslintrc" | ".prettierrc" | ".editorconfig"
        )
    {
        return (Ico::FileCog, Tone::Cyan);
    }
    // Secrets, and git's own say-so (.gitignore, .gitattributes): muted, not a general setting.
    if lower.starts_with(".env") {
        return (Ico::FileCog, Tone::Green);
    }
    if lower.starts_with(".git") {
        return (Ico::FileCog, Tone::Muted);
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" | "go" | "c" | "h" | "cpp" | "hpp" | "swift" | "kt" | "java" | "cs" | "zig"
        | "scala" => (Ico::FileCode, Tone::Red),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" | "svelte" | "astro" => {
            (Ico::FileCode, Tone::Yellow)
        }
        "py" | "rb" | "php" | "lua" | "pl" | "ex" | "exs" => (Ico::FileCode, Tone::Blue),
        "html" | "htm" | "css" | "scss" | "sass" | "less" | "xml" | "svg" | "hbs" => {
            (Ico::FileCode, Tone::Magenta)
        }
        "json" | "jsonc" | "json5" => (Ico::FileJson, Tone::Yellow),
        "yml" | "yaml" | "toml" | "ini" | "conf" | "cfg" | "env" | "properties" | "plist" => {
            (Ico::FileCog, Tone::Cyan)
        }
        "md" | "mdx" | "txt" | "rst" | "org" | "csv" | "log" => (Ico::FileText, Tone::Muted),
        "sh" | "bash" | "zsh" | "fish" | "ps1" => (Ico::FileTerminal, Tone::Green),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "tiff" | "heic" => {
            (Ico::FileImage, Tone::Magenta)
        }
        "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => (Ico::FileAudio, Tone::Cyan),
        "mp4" | "mov" | "mkv" | "webm" | "avi" => (Ico::FileVideo, Tone::Cyan),
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "tar" | "zst" => {
            (Ico::FileArchive, Tone::Yellow)
        }
        "lock" | "pem" | "key" | "crt" => (Ico::FileLock, Tone::Muted),
        // A dotfile is a program's settings (.zshrc, .gitconfig).
        _ if lower.starts_with('.') => (Ico::FileCog, Tone::Cyan),
        _ => (Ico::File, Tone::Muted),
    }
}

#[cfg(test)]
mod tests {
    use super::{Tone, file_kind};
    use crate::icons::Ico;

    /// `file_kind` reads a name already lowered, as `file_icon` hands it over.
    fn kind(name: &str, dir: bool) -> (Ico, Tone) {
        file_kind(&name.to_lowercase(), dir)
    }

    #[test]
    fn a_name_that_says_what_it_is_outranks_its_extension() {
        assert_eq!(kind("README.md", false), (Ico::FileText, Tone::Blue));
        assert_eq!(kind("readme", false), (Ico::FileText, Tone::Blue));
        assert_eq!(kind("CHANGELOG.md", false), (Ico::FileText, Tone::Cyan));
        assert_eq!(kind("LICENSE", false), (Ico::Scale, Tone::Muted));
        assert_eq!(kind("AGENTS.md", false), (Ico::Bot, Tone::Cyan));
        assert_eq!(kind("SYSTEM.md", false), (Ico::Bot, Tone::Cyan));
        assert_eq!(kind("SKILL.md", false), (Ico::Wrench, Tone::Green));
        assert_eq!(kind("Cargo.toml", false), (Ico::FileCog, Tone::Red));
        assert_eq!(kind("Cargo.lock", false), (Ico::FileLock, Tone::Muted));
        assert_eq!(kind("package.json", false), (Ico::Package, Tone::Yellow));
        assert_eq!(
            kind("package-lock.json", false),
            (Ico::FileLock, Tone::Muted)
        );
        assert_eq!(kind("pnpm-lock.yaml", false), (Ico::FileLock, Tone::Muted));
        assert_eq!(kind("Makefile", false), (Ico::FileTerminal, Tone::Green));
        assert_eq!(kind("Dockerfile", false), (Ico::FileCog, Tone::Cyan));
        assert_eq!(kind("tsconfig.json", false), (Ico::FileCog, Tone::Cyan));
        assert_eq!(kind("vite.config.ts", false), (Ico::FileCog, Tone::Cyan));
        assert_eq!(kind(".env.local", false), (Ico::FileCog, Tone::Green));
        assert_eq!(kind(".gitignore", false), (Ico::FileCog, Tone::Muted));
        // Without a known name, the extension still speaks.
        assert_eq!(kind("main.rs", false), (Ico::FileCode, Tone::Red));
        assert_eq!(kind("welcome.md", false), (Ico::FileText, Tone::Muted));
        assert_eq!(kind("post.hbs", false), (Ico::FileCode, Tone::Magenta));
        assert_eq!(kind(".zshrc", false), (Ico::FileCog, Tone::Cyan));
    }

    #[test]
    fn a_folder_says_where_the_work_is() {
        assert_eq!(kind("node_modules", true), (Ico::FolderCog, Tone::Muted));
        assert_eq!(kind("src", true), (Ico::FolderCode, Tone::Blue));
        assert_eq!(kind("tests", true), (Ico::FolderGit, Tone::Green));
        assert_eq!(kind("docs", true), (Ico::FolderTree, Tone::Blue));
        assert_eq!(kind("dist", true), (Ico::FolderArchive, Tone::Yellow));
        assert_eq!(kind("__pycache__", true), (Ico::FolderLock, Tone::Muted));
        assert_eq!(kind(".github", true), (Ico::FolderCog, Tone::Cyan));
        assert_eq!(kind("assets", true), (Ico::Folder, Tone::Magenta));
        assert_eq!(kind(".agents", true), (Ico::Folder, Tone::Muted));
        // A folder of one's own is plain.
        assert_eq!(kind("content", true), (Ico::Folder, Tone::Blue));
    }
}
