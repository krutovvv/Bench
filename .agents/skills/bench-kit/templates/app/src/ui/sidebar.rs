//! Left panel: the top strip (mode toggles) beside the window buttons, the search band, the
//! list, a "New" button filling the bottom bar.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use super::kit;
use crate::app::MainApp;
use crate::icons::Ico;

impl MainApp {
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // Rows and the button come from kit.rs: the house look without styling anything here.
        let rows: Vec<AnyElement> = self
            .visible()
            .into_iter()
            .map(|i| {
                let delete = kit::icon_button(
                    ("delete", i),
                    Ico::Trash,
                    "Delete",
                    false,
                    cx,
                    Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.delete_item(i, cx)
                    })),
                )
                .into_any_element();
                kit::list_row(
                    format!("row-{i}"),
                    self.items[i].title.clone(),
                    None,
                    self.selected == Some(i),
                    None,
                    Some(delete),
                    cx,
                    Box::new(cx.listener(move |this, _, _, cx| this.select(i, cx))),
                )
                .into_any_element()
            })
            .collect();

        let new_item = kit::primary_button(
            "new-item",
            Ico::Plus,
            "New",
            cx,
            Box::new(cx.listener(|this, _, _, cx| this.new_item(cx))),
        );

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            // Beside the window buttons: a mode's toggle goes here (kit::mode_toggle), e.g.
            // `kit::mode_toggle("mode-tools", Ico::Wrench, "Tools  ⌘3", self.tools_on, cx, ..)`.
            .child(kit::sidebar_top(vec![]))
            .child(kit::search_band(&self.search, cx))
            .child(
                v_flex()
                    .id("list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(rows),
            )
            .child(kit::bottom_bar(new_item, cx))
    }
}
