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
        .px_2()
        .py_1()
        .border_1()
        .rounded_md()
        .border_color(theme.input)
        .cursor_pointer()
        .hover(|s| s.border_color(theme.primary).bg(theme.list_hover))
}

/// A file's icon and color by its name, as an editor's tree shows them.
pub fn file_icon(name: &str, dir: bool, theme: &Theme) -> (Ico, Hsla) {
    if dir {
        return (Ico::Folder, theme.blue);
    }
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" | "go" | "c" | "h" | "cpp" | "hpp" | "swift" | "kt" | "java" | "cs" | "zig"
        | "scala" => (Ico::FileCode, theme.red),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" | "svelte" | "astro" => {
            (Ico::FileCode, theme.yellow)
        }
        "py" | "rb" | "php" | "lua" | "pl" | "ex" | "exs" => (Ico::FileCode, theme.blue),
        "html" | "htm" | "css" | "scss" | "sass" | "less" | "xml" | "svg" => {
            (Ico::FileCode, theme.magenta)
        }
        "json" | "jsonc" | "json5" => (Ico::FileJson, theme.yellow),
        "yml" | "yaml" | "toml" | "ini" | "conf" | "cfg" | "env" | "properties" | "plist" => {
            (Ico::FileCog, theme.cyan)
        }
        "md" | "mdx" | "txt" | "rst" | "org" | "csv" | "log" => {
            (Ico::FileText, theme.muted_foreground)
        }
        "sh" | "bash" | "zsh" | "fish" | "ps1" => (Ico::FileTerminal, theme.green),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "tiff" | "heic" => {
            (Ico::FileImage, theme.magenta)
        }
        "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => (Ico::FileAudio, theme.cyan),
        "mp4" | "mov" | "mkv" | "webm" | "avi" => (Ico::FileVideo, theme.cyan),
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "tar" | "zst" => {
            (Ico::FileArchive, theme.yellow)
        }
        "lock" | "pem" | "key" | "crt" => (Ico::FileLock, theme.muted_foreground),
        _ if matches!(lower.as_str(), "dockerfile" | "makefile") => (Ico::FileCog, theme.cyan),
        // A dotfile is a program's settings (.zshrc, .gitconfig).
        _ if lower.starts_with('.') && !lower[1..].contains('.') => (Ico::FileCog, theme.cyan),
        _ => (Ico::File, theme.muted_foreground),
    }
}
