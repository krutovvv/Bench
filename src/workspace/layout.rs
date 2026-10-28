//! The window as one element: the title bar over the center, the docks around it
//! (`render_layout`); the keys for the whole window see every
//! key first.

use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::{Center, DockPosition};

/// The window's dock dividers draw no line at rest: the sidebar is darker than the chat, so
/// the edge is its color (an old-school rule there was the eyesore). Hovered or dragged, the
/// divider shows as an accent line, so it can still be found and pulled. The IDE panel draws
/// its own left rule (content meets content there). `vertical` is for the divider above the
/// bottom dock: the line lies down instead of standing.
fn quiet_divider(vertical: bool) -> gpui_kit::base::ResizeHandleRenderer {
    std::rc::Rc::new(move |handle, _, cx| {
        let rest = matches!(handle.state(), gpui_kit::base::ResizeHandleState::Idle);
        let line = div().flex_none().bg(if rest {
            gpui_kit::transparent_black()
        } else {
            cx.theme().primary.opacity(0.6)
        });
        Some(
            if vertical {
                line.h(px(bench_ui::HAIRLINE)).w_full()
            } else {
                line.w(px(bench_ui::HAIRLINE)).h_full()
            }
            .into_any_element(),
        )
    })
}

impl BenchApp {
    /// The demo's hooks on the window (`BENCH_DEMO=new-project`, `BENCH_DEMO_RENAME`,
    /// `BENCH_DEMO_SEARCH`), once.
    fn demo_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.subject_opened && std::env::var("BENCH_DEMO").as_deref() == Ok("new-project") {
            self.subject_opened = true;
            self.new_project_form(window, cx);
        }
        // `BENCH_DEMO_BESIDE=<thread id>`: that thread beside the open one.
        static BESIDE: std::sync::Once = std::sync::Once::new();
        if let Ok(id) = std::env::var("BENCH_DEMO_BESIDE") {
            BESIDE.call_once(|| self.open_beside(id.into(), window, cx));
        }
        // `BENCH_DEMO_SEARCH=1|<words>`: ⌘K open, with those words typed.
        static SEARCH: std::sync::Once = std::sync::Once::new();
        if let Ok(q) = std::env::var("BENCH_DEMO_SEARCH") {
            SEARCH.call_once(|| {
                self.open_search(window, cx);
                if q != "1" {
                    self.sidebar.search.update(cx, |s, cx| s.set_value(q.clone(), window, cx));
                    self.search_threads(q, window, cx);
                }
            });
        }
        // `BENCH_DEMO_RENAME=<thread id>`: that thread being renamed.
        static RENAME: std::sync::Once = std::sync::Once::new();
        if let Ok(id) = std::env::var("BENCH_DEMO_RENAME") {
            RENAME.call_once(|| {
                self.start_rename(crate::sidebar::items::Item::Thread(id.into()), window, cx)
            });
        }
    }
}

impl Render for BenchApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The prompts offered with `@` follow the open thread's project.
        let project = self.thread(cx).project.clone();
        if !self.demo
            && self.local_conn().is_some()
            && self.prompts_for.as_deref() != Some(project.as_str())
        {
            self.prompts_for = Some(project);
            self.load_prompts(window, cx);
        }
        if self.demo {
            self.demo_hooks(window, cx);
            self.drop_probe(window, cx);
        }
        // The server browser's stream follows what is on screen (no frames for a hidden thread).
        #[cfg(not(target_family = "wasm"))]
        {
            let at = self.thread_ref(cx);
            self.update_browser_stream(&at);
        }
        let theme = cx.theme().clone();
        let body = match self.center {
            Center::Chat => div()
                .size_full()
                .child(self.active_thread.clone())
                .into_any_element(),
            Center::Settings => self.render_settings(cx),
            Center::NewProject => self.render_new_project_page(cx),
            Center::Doc => self.render_doc(window, cx),
        };

        // The window's one top bar, edge to edge over the docks: room for the window's buttons,
        // the sidebar's fold, a short rule, then the center's header (`titlebar/`).
        let header = h_flex()
            .flex_none()
            .w_full()
            .child(self.render_window_lead(cx))
            .child(div().flex_1().min_w_0().child(self.render_header(cx)));
        let main = v_flex()
            .size_full()
            .min_w_0()
            .child(div().flex_1().min_h_0().child(body));

        let content = self.render_layout(main, window, cx);
        // The menu bar's items act on this window (`app/menu.rs`).
        let base = crate::app::menu::listen(
            v_flex()
                .relative()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground),
            cx,
        )
            // Shortcuts for the whole window (app/keys.rs) see every key first.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if this.global_key(&ev.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(header)
            .child(div().flex_1().min_h_0().child(content))
            // ⌘K's list, over everything while it is open (`sidebar/search.rs`).
            .children(self.render_search(cx));
        base
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
        if self.layout.synced != Some(shape) {
            self.layout.synced = Some(shape);
            self.layout.across = cx.new(|_| Default::default());
            self.layout.bottom = cx.new(|_| Default::default());
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
                    .with_state(&self.layout.bottom)
                    .with_handle_appearance(quiet_divider(true))
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
        // The Code view: its status bar under the documents and the terminal, not under the
        // docks beside them; the chat has none.
        let center = if self.center == Center::Doc {
            v_flex()
                .size_full()
                .child(div().flex_1().min_h_0().child(center))
                .child(self.render_code_statusbar(cx))
                .into_any_element()
        } else {
            center
        };
        // The left dock: its panel, under the window's top bar, a hairline between it and the
        // center as a Mac's sidebar has (as the right dock's on its other side).
        let left = left.map(|panel| {
            v_flex()
                .size_full()
                .bg(theme.sidebar)
                .border_r_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_panel(panel, window, cx)),
                )
                // Under whichever panel it shows: the agent and the gear (`sidebar/agent.rs`).
                .child(self.render_foot(cx))
        });
        let right = right.map(|panel| self.render_panel(panel, window, cx));
        // Fixed: a window resize goes to the center; the docks keep their sizes.
        h_resizable("layout")
            .with_state(&self.layout.across)
            .with_handle_appearance(quiet_divider(false))
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

/// The docks' dividers: across the window, and over the bottom dock; which docks were drawn when
/// they last took their sizes (another set takes them again).
pub struct LayoutState {
    pub across: Entity<gpui_kit::component::resizable::ResizableState>,
    pub bottom: Entity<gpui_kit::component::resizable::ResizableState>,
    pub synced: Option<(bool, bool, bool)>,
}

impl LayoutState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            across: cx.new(|_| Default::default()),
            bottom: cx.new(|_| Default::default()),
            synced: None,
        }
    }
}
