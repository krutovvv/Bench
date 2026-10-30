//! The Code view's status bar, as an IDE's: under its documents and the terminal below them,
//! never under the thread beside them, and none in the chat. On its right the open file's caret,
//! indent and language (`pane/cursor.rs`), the thread beside (⌘J) and the terminal (⌃`).

use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::workspace::{DockPosition, Panel};
use bench_ui::icons::Ico;

impl BenchApp {
    pub(crate) fn render_code_statusbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let file = self.explorer.active.clone().and_then(|key| match &key {
            crate::explorer::DocKey::File(f) => self
                .explorer
                .docs
                .get(&key)
                .filter(|d| !d.binary)
                .map(|d| (d.editor.clone(), f.path.clone())),
            _ => None,
        });
        self.cursor_item.update(cx, |c, cx| c.show(file, cx));
        let beside_open = self.shown_panel(DockPosition::Right).is_some();
        let terminal_open = self.bottom_dock.shows(Panel::Terminal);
        h_flex()
            .flex_none()
            .w_full()
            .h(px(bench_ui::STATUS_H))
            .px(Gap::Xs)
            .gap(Gap::Xs)
            .items_center()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            .text_size(px(bench_ui::BAR_TEXT))
            .child(div().flex_1())
            .child(div().h_full().child(self.cursor_item.clone()))
            .child(
                bench_ui::icon_button(
                    "thread-beside",
                    Ico::Sparkles,
                    if beside_open {
                        "Close the thread beside  ⌘J"
                    } else {
                        "The thread beside  ⌘J"
                    },
                    beside_open,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx))),
            )
            .child(
                bench_ui::icon_button("terminal-panel", Ico::PanelBottom, "Terminal  ⌃`", terminal_open, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal_panel(window, cx))),
            )
            .into_any_element()
    }
}
