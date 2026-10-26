//! Rendering. Each file adds `render_*` methods to `BenchApp`.

mod asks;
mod chat;
mod composer;
pub(crate) mod inserts;
mod media;
mod panels;
mod project_list;
mod providers;
mod remote_files;
mod servers;
mod sidebar;
mod sites;
mod statusbar;
mod tasks;
#[cfg(not(target_family = "wasm"))]
mod terminal;
#[cfg(target_family = "wasm")]
#[path = "terminal_web.rs"]
mod terminal;
mod tools;

pub fn terminal_palette(theme: &gpui_kit::component::Theme) -> crate::term::Palette {
    terminal::palette(theme)
}

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{BenchApp, Panel};

/// A sidebar's search: a transparent band on the sidebar's own color, no rules, no frame; the
/// magnifier right over the group icons. Always there (⌘F focuses it).
pub fn search_band(input: &Entity<gpui_kit::component::input::InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .w_full()
        .h(bar_h(cx))
        // Measured: the input's own padding plus the icon's side bearing put it 2.5 right of
        // the group icons at 6.
        .pl(px(3.5))
        .pr_2()
        .items_center()
        .child(
            gpui_kit::component::input::Input::new(input)
                .appearance(false)
                .cleanable(true)
                .prefix(
                    crate::icons::Ico::Search
                        .icon()
                        .small()
                        .text_color(theme.muted_foreground),
                ),
        )
}

/// Height of every bar (search band, "New session", the composer, a tag strip): the header's 36
/// less the 1px rule the bar draws (or shares) — 35 at the 14px font, following the
/// Appearance size. A bar with its rule is `bar_h + 1`.
pub fn bar_h(cx: &App) -> Pixels {
    cx.theme().font_size * 2.5
}

/// Rows under a group header (sessions of a project, servers of a folder) start where the
/// header's name does: 1rem inset + 1rem icon + 0.375rem gap, less the row's 2px stripe and
/// 2px the icon box is narrower than 1rem (measured).
impl BenchApp {
    /// The strip beside the window buttons at the top of every sidebar, with the mode toggles
    /// (Tools, then Tasks) in its top-right corner (the sidebar's own corner, as SSH has the
    /// window's).
    pub(super) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // A mode's toggle: its icon, lit while it is on; the tooltip says how to get back.
        let toggle = |id: &'static str, icon: crate::icons::Ico, name: &'static str, on: bool| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(if on {
                    theme.primary
                } else {
                    theme.muted_foreground
                })
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| {
                    let tip = toggle_tip(name, on);
                    gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                })
        };
        // 35 + the search band's top rule = the header's 36 with its bottom rule: the two rules
        // are one line across the window.
        h_flex()
            .h(px(35.))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            // The icon's 4px box padding + 12 = the sidebar's 16px inset.
            .pr(px(12.))
            // "+": a new item of the mode (a chat, a server, a task), then folding the sidebar
            // away (⌘\\), then the two utilities. One style everywhere: no big buttons.
            .when(!self.tools.on, |d| {
                let tip = if self.tasks.on {
                    "New task  ⌘N"
                } else if self.ssh.on {
                    "New server"
                } else {
                    "New session  ⌘N"
                };
                let _ = tip;
                // "+": a new project (the sidebar holds projects only).
                d.child(
                    toggle("new-item", crate::icons::Ico::Plus, "New project", false).on_click(
                        cx.listener(|this, _, window, cx| this.new_project_form(window, cx)),
                    ),
                )
            })
            .child(
                toggle(
                    "sidebar-hide",
                    crate::icons::Ico::PanelLeft,
                    "Hide sidebar  ⌘\\",
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .into_any_element()
    }
}

/// A mode toggle's tooltip: its name and shortcut, or "Back" and the shortcut while it is on.
/// Split at the double space, never by bytes: ⌘ is 3 bytes, and a byte slice through it panics,
/// which aborts the app (it did, on hovering the lit-up Tools toggle).
fn toggle_tip(name: &str, on: bool) -> String {
    match (on, name.rsplit_once("  ")) {
        (true, Some((_, keys))) => format!("Back  {keys}"),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn toggle_tip_keeps_the_shortcut_whole() {
        assert_eq!(super::toggle_tip("Tools  ⌘3", false), "Tools  ⌘3");
        assert_eq!(super::toggle_tip("Tools  ⌘3", true), "Back  ⌘3");
        assert_eq!(super::toggle_tip("Tasks  ⌘4", true), "Back  ⌘4");
        assert_eq!(super::toggle_tip("New window", true), "New window");
    }
}

pub fn nest_pl(cx: &App) -> Pixels {
    cx.theme().font_size * 2.375 - px(4.)
}

/// The group's indent guide: a 1px line under the center of the header's icon (1.5rem), less
/// the row's stripe. In a group's `last` row it stops where the text does, not in the row's
/// bottom padding (`py_2` = 0.5rem).
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

/// A group header's fold chevron, at the right before its "+": shown on a folded group, and
/// on hover of an open one.
/// A sidebar head's icon in a fixed square, centered: icons of any drawing size line up, and
/// the names after them start at one x.
pub fn head_icon(icon: crate::icons::Ico, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(icon.icon().small().text_color(cx.theme().muted_foreground))
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

pub fn group_chevron(collapsed: bool, group: SharedString, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .p_0p5()
        .text_color(cx.theme().muted_foreground)
        .opacity(if collapsed { 1. } else { 0. })
        .group_hover(group, |s| s.opacity(1.))
        .child(
            if collapsed {
                crate::icons::Ico::ChevronRight
            } else {
                crate::icons::Ico::ChevronDown
            }
            .icon()
            .xsmall(),
        )
}

/// A 2-rem control's height (a filled button outside a bar, e.g. "Add server"). The bars
/// themselves are `bar_h`: a button that fills one takes `.h_full()` instead.
pub const BAR_CONTROL_REMS: f32 = 2.;

/// The window's column dividers draw no line at rest: the sidebar is darker than the chat, so
/// the edge is its color (an old-school rule there was the eyesore). Hovered or dragged, the
/// divider shows as an accent line, so it can still be found and pulled. The IDE panel draws
/// its own left rule (content meets content there).
fn quiet_divider() -> gpui_kit::base::ResizeHandleRenderer {
    std::rc::Rc::new(|handle, _, cx| {
        let rest = matches!(handle.state(), gpui_kit::base::ResizeHandleState::Idle);
        Some(
            div()
                .flex_none()
                .w(px(1.))
                .h_full()
                .bg(if rest {
                    gpui_kit::transparent_black()
                } else {
                    cx.theme().primary.opacity(0.6)
                })
                .into_any_element(),
        )
    })
}

/// The chat: who speaks is an avatar in a left gutter, not a colored band.
/// The avatar sits `CHAT_LEFT` from the column's edge; text starts at `CHAT_TEXT`; the right
/// side keeps `CHAT_RIGHT` (more than the left: a line ends well before the edge).
/// The thread's side room, equal to the room above its first turn (the user asked for both alike).
pub const CHAT_LEFT: f32 = 38.;
pub const AVATAR: f32 = 24.;
pub const CHAT_TEXT: f32 = CHAT_LEFT + AVATAR + 14.;
pub const CHAT_RIGHT: f32 = 38.;
/// Every row of the thread (the question, the answer, the composer) pads its top and bottom by
/// this, so any two rows are 2 × TURN_Y apart, whoever speaks.
pub const TURN_Y: f32 = 14.;

/// The head of every block (an insert, a code block, a tool's pane, an open file): one height,
/// one faint ground, small muted text. `over_body`: a line under it, when something follows.
pub const BLOCK_HEAD_H: f32 = 32.;
/// A file's icon and its color, by what it is, as an editor's file tree shows them: folders in
/// blue, code in its own colors, config, data, pictures, media, archives each told apart.
pub fn file_icon(
    name: &str,
    dir: bool,
    theme: &gpui_kit::component::Theme,
) -> (crate::icons::Ico, Hsla) {
    use crate::icons::Ico;
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
        // A dotfile is a program's settings (.zshrc, .gitconfig, .npmrc).
        _ if lower.starts_with('.') && !lower[1..].contains('.') => (Ico::FileCog, theme.cyan),
        _ => (Ico::File, theme.muted_foreground),
    }
}

/// A text field's height (gpui-kit's default input, measured: 28 pt): a form's other controls —
/// switches, pickers, its buttons — match it, so nothing in a form changes size.
pub const FIELD_H: f32 = 28.;

/// The status bar's height with the rule over it: the window's height is the thread's viewport
/// plus this.
pub const STATUS_H: f32 = 29.;

/// The thread's leading, in lines: one place, so the composer and the messages cannot drift
/// apart — a textarea by itself is tighter (1.23 measured), and that read as a different rhythm
/// between what is typed and what was sent.
pub const LEAD: f32 = 1.5;

/// The height of every small row and chip in the thread: an attachment's chip, a message that
/// waits in the queue, a plan's head. One height, so these rows do not jump about each other.
/// A field is as tall as a chip (`FIELD_H` = `CHIP_H`): nothing small grows past either.
pub const CHIP_H: f32 = FIELD_H;

/// A picture's square, in the composer and in a sent message: big enough to tell at a glance
/// what is inside — the file's name says nothing — and always the same size, so a line of them
/// reads as one line.
pub const THUMB: f32 = 64.;

/// A code editor's line (gpui-kit's editor: the mono font × 1.5): what a block as tall as its
/// text counts in.
pub fn editor_line(cx: &App) -> f32 {
    f32::from(gpui_kit::component::ActiveTheme::theme(cx).mono_font_size) * 1.5
}

/// An editor showing all of its `lines` with room to spare: the text never overflows it, so
/// the editor shows no scrollbar and the wheel scrolls the thread, not the editor.
pub fn editor_height(lines: usize, cx: &App) -> f32 {
    (lines.max(1) + 1) as f32 * editor_line(cx) + 16.
}
pub fn block_head(theme: &gpui_kit::component::Theme, over_body: bool) -> Div {
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_2()
        .items_center()
        .bg(theme.muted.opacity(0.35))
        .when(over_body, |d| d.border_b_1().border_color(theme.border))
        .text_sm()
        .text_color(theme.muted_foreground)
}

/// Who speaks, in the chat's left gutter: the person a dark disc with a figure, the agent the
/// accent's tint with a bot — told apart at a glance. The composer wears the person's: the next
/// message is the next turn.
pub fn avatar(agent: bool, theme: &gpui_kit::component::Theme) -> Div {
    div()
        .flex_none()
        .size(px(AVATAR))
        // Its center on the first line's (measured).
        .mt(px(-0.5))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if agent {
                // Both discs solid: a pale one reads smaller than a dark one of the same size.
                d.bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .child(crate::icons::Ico::Bot.icon().xsmall())
            } else {
                d.bg(theme.foreground.opacity(0.78))
                    .text_color(theme.background)
                    .child(crate::icons::Ico::UserRound.icon().xsmall())
            }
        })
}

/// "1 action", "3 actions" (the `few` form is kept for call-site symmetry).
pub fn plural(n: usize, one: &str, _few: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn clock(secs: u64) -> String {
    if secs >= 60 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{}k", n / 1000),
        n => n.to_string(),
    }
}

pub fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

impl BenchApp {
    /// "app · main": the thread works in a git repository (its folder's name and branch). Git
    /// itself is the agent's: ask it to commit, or to undo.
    fn render_project_chip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.project.clone()?;
        let label = if p.branch.is_empty() {
            p.name()
        } else {
            format!("{} · {}", p.name(), p.branch)
        };
        let theme = cx.theme();
        Some(
            h_flex()
                .flex_none()
                .gap_1()
                .px_1()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(crate::icons::Ico::GitBranch.icon().small())
                .child(label)
                .into_any_element(),
        )
    }

    /// Header strip: the session title, or the settings tabs.
    fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.tasks.on && self.panel == Panel::Chat {
            return self.render_tasks_header(cx);
        }
        if self.tools.on && self.panel == Panel::Chat {
            return self.render_tools_header(cx);
        }
        if self.ssh.on && self.panel == Panel::Chat {
            return self.render_ssh_header(cx);
        }
        let theme = cx.theme().clone();
        let bar = h_flex()
            .id("header")
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .items_center()
            .border_b_1()
            .border_color(theme.border);

        if !self.panel.is_settings() {
            let title: SharedString = self
                .session
                .as_ref()
                .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty())
                // A thread about a task list is named after it until it has a title of its own.
                .or_else(|| self.subject_title())
                .unwrap_or_else(|| "New session".into())
                .into();
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(title),
                )
                .children(self.render_project_chip(cx))
                .child(self.render_header_tail(cx))
                .into_any_element();
        }

        let title = if self.panel == Panel::NewProject {
            "New project"
        } else {
            "Settings"
        };
        bar.child(
            div()
                .px_4()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("settings-close")
                .px_3()
                .h_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child(crate::icons::Ico::X.icon().small())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.panel = Panel::Chat;
                    this.project_form = None;
                    cx.notify();
                })),
        )
        .into_any_element()
    }
}

impl Render for BenchApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The open chat's unsent text in the composer (app.rs, drafts).
        self.sync_draft(window, cx);
        // A new chat is a code editor's empty file: the caret alone (no avatars yet), focused.
        // The composer never has a placeholder: it is the thread's next text, not a form field.
        let fresh = self.turns.is_empty() && self.session.is_none();
        if fresh != self.composer_fresh {
            self.composer_fresh = fresh;
            if fresh && self.panel == Panel::Chat {
                self.composer.update(cx, |s, cx| s.focus(window, cx));
            }
        }
        // The server browser's stream follows what is on screen (no frames for a hidden thread).
        #[cfg(not(target_family = "wasm"))]
        self.update_browser_stream();
        let theme = cx.theme().clone();
        // The server form (new or edited server) takes the chat area while open.
        let ssh = self.ssh.on && self.panel == Panel::Chat;
        let body = match self.panel {
            Panel::Chat if self.tasks.on => self.render_tasks_body(cx),
            Panel::Chat if self.tools.on => self.render_tools_body(cx),
            Panel::Chat if ssh => self.render_ssh_body(window, cx),
            // Files dropped anywhere on the chat or the input are attached.
            Panel::Chat => v_flex()
                .id("chat-drop")
                .size_full()
                .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active))
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.add_attachments(paths.paths().to_vec(), cx)
                }))
                .child(div().flex_1().min_h_0().child(self.render_chat(window, cx)))
                .into_any_element(),
            Panel::Providers => self.render_providers(cx),
            Panel::NewProject => self.render_project_form(cx),
        };

        // The sidebar folded away: its corner joins the header — room for the window's buttons
        // and the toggle that brings it back.
        let header = self.render_header(cx);
        let header = if self.sidebar_hidden {
            let theme = cx.theme().clone();
            h_flex()
                .flex_none()
                .child(
                    h_flex()
                        .h(px(36.))
                        .flex_none()
                        .pl(px(78.))
                        .pr_1()
                        .items_center()
                        .bg(theme.title_bar)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .id("sidebar-show")
                                .p_1()
                                .cursor_pointer()
                                .text_color(theme.muted_foreground)
                                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                                .child(crate::icons::Ico::PanelLeft.icon().small())
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("Show sidebar  ⌘\\")
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                )
                .child(div().flex_1().min_w_0().child(header))
                .into_any_element()
        } else {
            header
        };
        let main = v_flex()
            .size_full()
            .min_w_0()
            .child(header)
            .child(div().flex_1().min_h_0().child(body));

        let content = self.render_layout(main, window, cx);
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Shortcuts for the whole window (app/keys.rs) see every key first.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if this.global_key(&ev.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(div().flex_1().min_h_0().child(content))
            .child(self.render_statusbar(window, cx))
    }
}

impl BenchApp {
    /// Sidebar | main, the sidebar resizable (its width kept). Code opens in Zed: no panel of
    /// files here.
    fn render_layout(
        &mut self,
        main: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hidden = self.sidebar_hidden;
        let shape = (false, false, hidden);
        if self.layout_synced != Some(shape) {
            self.layout_synced = Some(shape);
            self.layout_state = cx.new(|_| Default::default());
        }
        let state = self.layout_state.clone();
        let _ = window;
        // Every drag of the divider is the sidebar's width from now on.
        let view = cx.entity();
        let remember = move |state: &Entity<gpui_kit::component::resizable::ResizableState>,
                             _: &mut Window,
                             cx: &mut App| {
            let sizes = state.read(cx).sizes().clone();
            view.update(cx, |this, _| {
                if let (Some(w), false) = (sizes.first(), this.sidebar_hidden) {
                    this.sidebar_w = *w;
                }
            });
        };
        // Fixed: a window resize goes to the thread; the sidebar keeps its width.
        let left = resizable_panel()
            .size(self.sidebar_w)
            .size_range(px(200.)..px(480.))
            .fixed();
        h_resizable("layout")
            .with_state(&state)
            .with_handle_appearance(quiet_divider())
            .on_resize(remember)
            .when(!hidden, |d| d.child(left.child(self.render_sidebar(cx))))
            .child(resizable_panel().child(main))
            .into_any_element()
    }
}

/// A form inside the thread, as the environment block's rows: a muted label, the field; a short
/// note; then its actions (the primary one filled). No page of its own, no hints column.
pub fn inline_form(
    title: &'static str,
    rows: Vec<(&'static str, AnyElement)>,
    note: Option<SharedString>,
    actions: Vec<AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap_1()
        .child(div().pb_1().font_weight(FontWeight::MEDIUM).child(title))
        .children(rows.into_iter().map(|(label, field)| {
            h_flex()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .w(px(110.))
                        .flex_none()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(div().flex_1().max_w(px(560.)).child(field))
        }))
        .children(note.map(|n| div().pt_1().text_color(theme.muted_foreground).child(n)))
        .child(h_flex().pt_2().gap_3().children(actions))
        .into_any_element()
}

/// A form's action: the primary one filled, the others quiet text.
pub fn form_action(
    id: &'static str,
    label: &'static str,
    primary: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let (fg, bg, hover, muted) = (
        theme.primary_foreground,
        theme.primary,
        theme.primary_hover,
        theme.muted_foreground,
    );
    let text = theme.foreground;
    div()
        .id(id)
        .h(px(FIELD_H))
        .flex()
        .items_center()
        .cursor_pointer()
        .when(primary, |d| {
            d.px_3()
                .rounded_md()
                .bg(bg)
                .text_color(fg)
                .hover(move |s| s.bg(hover))
        })
        .when(!primary, |d| {
            d.text_color(muted).hover(move |s| s.text_color(text))
        })
        .child(label)
}
