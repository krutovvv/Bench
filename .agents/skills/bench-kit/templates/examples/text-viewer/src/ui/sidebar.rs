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
        // Rows and the button come from kit.rs: the house look without styling anything here.
        let rows: Vec<AnyElement> = self
            .visible()
            .into_iter()
            .map(|i| {
                let path = &self.recent[i];
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let dir = path
                    .parent()
                    .map(|d| {
                        d.to_string_lossy()
                            .replace(&std::env::var("HOME").unwrap_or_default(), "~")
                    })
                    .unwrap_or_default();
                let forget = kit::icon_button(
                    ("forget", i),
                    Ico::X,
                    "Remove from Recent",
                    false,
                    cx,
                    Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.forget(i, cx)
                    })),
                )
                .into_any_element();
                let open = path.clone();
                kit::list_row(
                    format!("row-{i}"),
                    name,
                    Some(dir.into()),
                    self.file.as_ref() == Some(path),
                    None,
                    Some(forget),
                    cx,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        this.load_file(open.clone(), window, cx)
                    })),
                )
                .into_any_element()
            })
            .collect();

        let new_item = kit::primary_button(
            "open-file",
            Ico::Folder,
            "Open",
            cx,
            Box::new(cx.listener(|this, _, window, cx| this.open_file_dialog(window, cx))),
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
