//! The right dock's thread (`Panel::Thread`): a thread's title and its buttons over its view. Beside
//! a document it is the open thread; beside the open thread it is a second one (`beside_thread`).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::v_flex;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// A thread's title as the sidebar has it.
    pub(crate) fn session_title(&self, session: Option<&str>) -> SharedString {
        session
            .and_then(|id| self.sessions.iter().find(|s| s.id == id))
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "New thread".into())
            .into()
    }

    /// The thread as the right dock's panel, beside a document: its title and close over the same
    /// view the center shows in the threads view; a tab dropped on it sends its file.
    pub(crate) fn render_thread_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let view = self.active_thread.clone();
        let title = self.session_title(view.read(cx).thread.session.as_deref());
        let head = bench_ui::ide_head(
            title,
            None,
            vec![
                bench_ui::icon_button("thread-new", Ico::Plus, "New thread  ⌘N", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_session(cx);
                        this.focus_composer(window, cx);
                    }))
                    .into_any_element(),
                bench_ui::icon_button("thread-close", Ico::X, "Close  ⌘J", false, cx)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx)),
                    )
                    .into_any_element(),
            ],
            cx,
        )
        .h(bench_ui::bar_h(cx))
        .bg(theme.title_bar);
        v_flex()
            .size_full()
            .drag_over::<crate::explorer::tabs::DraggedTab>(|s, _, _, cx| {
                s.bg(cx.theme().list_active)
            })
            .on_drop(
                cx.listener(|this, d: &crate::explorer::tabs::DraggedTab, window, cx| {
                    if let crate::explorer::DocKey::File(f) = &d.key {
                        this.send_file_to_thread(f, window, cx)
                    }
                }),
            )
            .child(head)
            .child(div().flex_1().min_h_0().child(view))
            .into_any_element()
    }
}
