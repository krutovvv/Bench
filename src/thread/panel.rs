//! The right dock's thread (`Panel::Thread`): a thread's title and its buttons over its view. Beside
//! a document it is the open thread; beside the open thread it is a second one (`beside_thread`,
//! `beside.rs`).

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, v_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;
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

    /// The thread view the right dock shows: beside a thread, the second one; beside anything
    /// else, the open thread.
    pub(crate) fn panel_thread(&self) -> Option<Entity<crate::thread::ThreadView>> {
        if self.center == Center::Chat {
            self.beside_thread.clone()
        } else {
            Some(self.active_thread.clone())
        }
    }

    /// The thread as the right dock's panel: its title and buttons over its view (the center's
    /// own beside a document, a second one beside a thread); a tab dropped on it sends its file.
    pub(crate) fn render_thread_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(view) = self.panel_thread() else {
            return div().into_any_element();
        };
        let title = self.session_title(view.read(cx).thread.session.as_deref());
        // Beside a thread: only Close (a new thread is the center's).
        if self.center == Center::Chat {
            let head = bench_ui::ide_head(
                title,
                None,
                vec![
                    bench_ui::icon_button("beside-close", Ico::X, "Close  ⌘J", false, cx)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_beside(cx);
                            this.right_dock.open = false;
                        }))
                        .into_any_element(),
                ],
                cx,
            )
            .h(bench_ui::bar_h(cx))
            .bg(theme.title_bar);
            return v_flex()
                .size_full()
                .child(head)
                .child(div().flex_1().min_h_0().child(view))
                .into_any_element();
        }
        // A document in the middle: the new thread is spelled out, as "New project" is —
        // a bare plus said nothing. Closing is the status bar's sparkle pressed off (⌘J),
        // not a cross of this panel's own.
        let head = bench_ui::ide_head(
            title,
            None,
            vec![
                Button::new("thread-new")
                    .ghost()
                    .small()
                    .child(div().text_xs().child("New Thread"))
                    .text_color(theme.muted_foreground)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_thread_here(cx);
                        this.focus_composer(window, cx);
                    }))
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
