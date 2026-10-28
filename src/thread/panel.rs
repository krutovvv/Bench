//! The right dock's thread (`Panel::Thread`): a thread's title and its buttons over its view. Beside
//! a document it is the open thread; beside the open thread it is a second one (`beside_thread`,
//! `beside.rs`).

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;
use bench_ui::icons::Ico;

impl BenchApp {
    /// A thread's title as the sidebar has it.
    pub(crate) fn thread_title(&self, id: Option<&protocol::ThreadId>) -> SharedString {
        id.and_then(|id| self.threads.list.iter().find(|s| &s.id == id))
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
        let title = self.thread_title(view.read(cx).thread.id.as_ref());
        // Beside a thread: no head of its own — its title and close are in the window's bar,
        // after the open thread's (`titlebar`), so two threads are not two bars.
        if self.center == Center::Chat {
            return div().size_full().child(view).into_any_element();
        }
        // A document in the middle: a new thread is the compose button Mail and Notes have,
        // its words in the tooltip. Closing is the status bar's sparkle pressed off (⌘J), not a
        // cross of this panel's own.
        let head = bench_ui::ide_head(
            title,
            None,
            vec![
                bench_ui::icon_button("thread-new", Ico::SquarePen, "New thread  ⌘N", false, cx)
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
            .drag_over::<crate::explorer::pane::tabs::DraggedTab>(|s, _, _, cx| {
                s.bg(cx.theme().list_active)
            })
            .on_drop(
                cx.listener(|this, d: &crate::explorer::pane::tabs::DraggedTab, window, cx| {
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
