//! Rendering. Each file adds `render_*` methods to `BenchApp`.

mod asks;
mod composer;
pub(crate) mod media;
mod panels;
mod providers;
mod sidebar;
mod statusbar;
pub(crate) mod thread;

pub fn terminal_palette(
    theme: &gpui_kit::component::Theme,
) -> crate::inserts::terminal::session::Palette {
    crate::inserts::terminal::view::palette(theme)
}

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{BenchApp, Panel};

pub use bench_ui::{
    AVATAR, BAR_CONTROL_REMS, BLOCK_HEAD_H, CHAT_LEFT, CHAT_RIGHT, CHAT_TEXT, CHIP_H, FIELD_H,
    LEAD, RING_CLEAR, STATUS_H, THUMB, TURN_Y, avatar, block_head, choice_row, clip, clock,
    compact, editor_height, editor_line, file_icon, form_action, group_chevron, head_icon_in,
    head_plus_slot, inline_form, nest_guide, nest_pl, plural, search_band,
};

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: "+" (a new project) and the fold.
    /// 35 + the search band's top rule = the header's 36: the two rules are one line.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(px(35.))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            // The icon's 4 px box padding + 12 = the sidebar's 16 px inset.
            .pr(px(12.))
            .child(
                bench_ui::icon_button(
                    "new-item",
                    crate::icons::Ico::Plus,
                    "New project",
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.new_project_form(window, cx))),
            )
            .child(
                bench_ui::icon_button(
                    "sidebar-hide",
                    crate::icons::Ico::PanelLeft,
                    "Hide sidebar  ⌘\\",
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .into_any_element()
    }
}

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
        let fresh = self.thread.is_empty() && self.session.is_none();
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
        let body = match self.panel {
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
