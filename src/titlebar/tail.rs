//! The title bar's right end, the window's top-right corner: Chat | Code (⌘E) and nothing else,
//! as Claude's. The Code view's toggles (the thread beside, the terminal) are its own status bar
//! (`explorer/statusbar.rs`); the chat has none.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Panel;
use bench_ui::icons::Ico;

impl BenchApp {
    pub(super) fn render_header_tail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ide = self.left_dock.shows(Panel::Explorer);
        let app = cx.entity().downgrade();
        h_flex()
            .flex_none()
            .items_center()
            // The thread, or its project as an IDE shows it — one hop either way, and which one
            // is on is plain to see.
            .child(bench_ui::mode_switch(
                "mode",
                &[
                    (Ico::MessageSquare, "Chat", "The threads  ⌘E"),
                    (Ico::Code, "Code", "The project's files  ⌘E"),
                ],
                usize::from(ide),
                cx,
                std::rc::Rc::new(move |k, window, cx| {
                    if (k == 1) != ide {
                        let _ = app.update(cx, |this, cx| this.toggle_side(window, cx));
                    }
                }),
            ))
            .into_any_element()
    }
}
