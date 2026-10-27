//! The window as one element: the title bar over the center, the docks around it
//! (`render_layout`), the status bar under everything; the keys for the whole window see every
//! key first.

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;

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

impl Render for BenchApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The open chat's unsent text in the composer (app.rs, drafts).
        self.sync_draft(window, cx);
        // A new chat is a code editor's empty file: the caret alone (no avatars yet), focused.
        // The composer never has a placeholder: it is the thread's next text, not a form field.
        let fresh = self.thread.is_empty() && self.session.is_none();
        if fresh != self.composer_fresh {
            self.composer_fresh = fresh;
            if fresh && self.center == Center::Chat {
                self.composer.update(cx, |s, cx| s.focus(window, cx));
            }
        }
        // The server browser's stream follows what is on screen (no frames for a hidden thread).
        #[cfg(not(target_family = "wasm"))]
        self.update_browser_stream();
        let theme = cx.theme().clone();
        let body = match self.center {
            // Files dropped anywhere on the chat or the input are attached.
            Center::Chat => v_flex()
                .id("chat-drop")
                .size_full()
                .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active))
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.add_attachments(paths.paths().to_vec(), cx)
                }))
                .child(div().flex_1().min_h_0().child(self.render_chat(window, cx)))
                .into_any_element(),
            Center::Providers => self.render_providers(cx),
            Center::NewProject => self.render_project_form(cx),
            Center::Doc => self.render_doc(window, cx),
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
                        .h(bench_ui::header_h(cx))
                        .flex_none()
                        .pl(px(bench_ui::WINDOW_BUTTONS_W))
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
                                .child(bench_ui::icons::Ico::PanelLeft.icon().small())
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
            // The app menu's Settings… (⌘,) acts on this window (`app/menu.rs`).
            .on_action(cx.listener(BenchApp::on_open_settings))
            .on_action(cx.listener(BenchApp::on_new_window))
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
    /// The workspace's docks around the center (`workspace/mod.rs`): the sidebar | the center |
    /// the right dock's panel (the thread beside a document), the docks resizable and their widths kept.
    fn render_layout(
        &mut self,
        main: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hidden = self.sidebar_hidden;
        let beside = self.render_right(window, cx);
        let shape = (false, beside.is_some(), hidden);
        if self.layout_synced != Some(shape) {
            self.layout_synced = Some(shape);
            self.layout_state = cx.new(|_| Default::default());
        }
        let state = self.layout_state.clone();
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
                // The right dock's width, when it is drawn (the sizes are its, not the center's).
                if let (Some(w), Some(_)) = (sizes.last(), this.right_shown()) {
                    if sizes.len() > 1 {
                        this.right_w = Some(*w);
                    }
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
            .when(!hidden, |d| {
                d.child(left.child(self.render_sidebar(window, cx)))
            })
            .child(resizable_panel().child(main))
            .when_some(beside, |d, pane| {
                let w = self.right_w.unwrap_or(px(bench_ui::BESIDE_W));
                d.child(
                    resizable_panel()
                        .size(w)
                        .size_range(px(bench_ui::BESIDE_MIN_W)..px(bench_ui::BESIDE_MAX_W))
                        .fixed()
                        .child(
                            div()
                                .size_full()
                                .border_l_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background)
                                .child(pane),
                        ),
                )
            })
            .into_any_element()
    }
}
