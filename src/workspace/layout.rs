//! The window as one element: the title bar over the center, the docks around it
//! (`render_layout`), the status bar under everything; the keys for the whole window see every
//! key first.

use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::{Center, DockPosition};

/// The window's dock dividers draw no line at rest: the sidebar is darker than the chat, so
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
    /// The demo's hooks on the window (`BENCH_DEMO=new-project`, `BENCH_DEMO_RENAME`), once.
    fn demo_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.subject_opened && std::env::var("BENCH_DEMO").as_deref() == Ok("new-project") {
            self.subject_opened = true;
            self.new_project_form(window, cx);
        }
        // `BENCH_DEMO_RENAME=<thread id>`: that thread being renamed.
        static RENAME: std::sync::Once = std::sync::Once::new();
        if let Ok(id) = std::env::var("BENCH_DEMO_RENAME") {
            RENAME.call_once(|| {
                self.start_rename(crate::sidebar::items::Item::Thread(id), window, cx)
            });
        }
    }
}

impl Render for BenchApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The prompts offered with `@` follow the open thread's project.
        let project = self.thread(cx).thread_project.clone();
        if !self.demo
            && self.local_conn().is_some()
            && self.prompts_for.as_deref() != Some(project.as_str())
        {
            self.prompts_for = Some(project);
            self.load_prompts(window, cx);
        }
        if self.demo {
            self.demo_hooks(window, cx);
        }
        // The server browser's stream follows what is on screen (no frames for a hidden thread).
        #[cfg(not(target_family = "wasm"))]
        {
            let at = self.thread_ref(cx);
            self.update_browser_stream(&at);
        }
        let theme = cx.theme().clone();
        let body = match self.center {
            // Files dropped anywhere on the chat or the input are attached.
            Center::Chat => v_flex()
                .id("chat-drop")
                .size_full()
                .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active))
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    let paths = paths.paths().to_vec();
                    this.active_thread
                        .update(cx, |v, cx| v.add_attachments(paths, cx))
                }))
                .child(div().flex_1().min_h_0().child(self.active_thread.clone()))
                .into_any_element(),
            Center::Providers => self.render_providers(cx),
            Center::NewProject => self.render_project_form(cx),
            Center::Doc => self.render_doc(window, cx),
        };

        // The sidebar folded away: its corner joins the header — room for the window's buttons
        // and the toggle that brings it back.
        let header = self.render_header(cx);
        let header = if self.shown_panel(DockPosition::Left).is_none() {
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
    /// The workspace's docks around the center (`workspace/dock.rs`): the left dock | the center
    /// over the bottom dock | the right dock, each drawn with the panel it shows; their dividers
    /// dragged, a dock keeps its size.
    fn render_layout(
        &mut self,
        main: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let left = self.shown_panel(DockPosition::Left);
        let right = self.shown_panel(DockPosition::Right);
        let bottom = self.shown_panel(DockPosition::Bottom);
        let shape = (left.is_some(), right.is_some(), bottom.is_some());
        if self.layout_synced != Some(shape) {
            self.layout_synced = Some(shape);
            self.layout_state = cx.new(|_| Default::default());
            self.bottom_state = cx.new(|_| Default::default());
        }
        let theme = cx.theme().clone();
        // Every drag of a divider is its docks' size from now on (the center takes the rest).
        let view = cx.entity();
        let remember = move |state: &Entity<gpui_kit::component::resizable::ResizableState>,
                             _: &mut Window,
                             cx: &mut App| {
            let sizes = state.read(cx).sizes().clone();
            view.update(cx, |this, _| {
                let (l, r) = (shape.0, shape.1);
                if let (true, Some(w)) = (l, sizes.first()) {
                    this.left_dock.size = Some(*w);
                }
                if let (true, Some(w), true) = (r, sizes.last(), sizes.len() > 1) {
                    this.right_dock.size = Some(*w);
                }
            });
        };
        let center = match bottom {
            Some(panel) => {
                let view = cx.entity();
                let remember_bottom =
                    move |state: &Entity<gpui_kit::component::resizable::ResizableState>,
                          _: &mut Window,
                          cx: &mut App| {
                        let sizes = state.read(cx).sizes().clone();
                        view.update(cx, |this, _| {
                            if let (Some(h), true) = (sizes.last(), sizes.len() > 1) {
                                this.bottom_dock.size = Some(*h);
                            }
                        });
                    };
                let pane = self.render_panel(panel, window, cx);
                v_resizable("center-bottom")
                    .with_state(&self.bottom_state)
                    .on_resize(remember_bottom)
                    .child(resizable_panel().child(main))
                    .child(
                        resizable_panel()
                            .size(self.bottom_dock.size())
                            .size_range(self.bottom_dock.size_range())
                            .fixed()
                            .child(
                                div()
                                    .size_full()
                                    .border_t_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(pane),
                            ),
                    )
                    .into_any_element()
            }
            None => main.into_any_element(),
        };
        // The left dock: its strip beside the window's buttons (`sidebar/top.rs`), its panel.
        let left = left.map(|panel| {
            v_flex()
                .size_full()
                .bg(theme.sidebar)
                .child(self.render_sidebar_top(cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_panel(panel, window, cx)),
                )
        });
        let right = right.map(|panel| self.render_panel(panel, window, cx));
        // Fixed: a window resize goes to the center; the docks keep their sizes.
        h_resizable("layout")
            .with_state(&self.layout_state)
            .with_handle_appearance(quiet_divider())
            .on_resize(remember)
            .when_some(left, |d, pane| {
                d.child(
                    resizable_panel()
                        .size(self.left_dock.size())
                        .size_range(self.left_dock.size_range())
                        .fixed()
                        .child(pane),
                )
            })
            .child(resizable_panel().child(center))
            .when_some(right, |d, pane| {
                d.child(
                    resizable_panel()
                        .size(self.right_dock.size())
                        .size_range(self.right_dock.size_range())
                        .fixed()
                        .child(
                            div()
                                .size_full()
                                .border_l_1()
                                .border_color(theme.border)
                                .bg(theme.background)
                                .child(pane),
                        ),
                )
            })
            .into_any_element()
    }
}
