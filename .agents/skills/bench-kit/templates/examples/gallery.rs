//! A screen that uses every block of `src/ui/kit.rs` (picture: gallery.png next to this file).
//! Compiled and checked inside a template app: it was `src/ui/gallery.rs`, with
//! `pub demo: crate::ui::gallery::Demo` in `MainApp` (`demo: Default::default()` in `new`) and
//! `render_gallery_sidebar` / `render_gallery_main` used in place of the sidebar and the main
//! column in `src/ui/mod.rs`. Copy the calls you need, not the whole file.

use std::rc::Rc;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use super::kit;
use crate::app::MainApp;
use crate::icons::Ico;

#[derive(Default)]
pub struct Demo {
    pub collapsed: bool,
    pub selected: usize,
    pub armed: bool,
    pub tab: usize,
    pub choice: usize,
}

impl MainApp {
    pub(super) fn render_gallery_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let files = ["main.rs", "app.rs", "theme.rs"];
        let mut list: Vec<AnyElement> = vec![
            kit::group_header(
                "g-src",
                Ico::Folder,
                "src",
                self.demo.collapsed,
                true,
                cx,
                Box::new(cx.listener(|this, _, _, cx| {
                    this.demo.collapsed = !this.demo.collapsed;
                    cx.notify();
                })),
                Some(Box::new(cx.listener(|_, _, _, _| {}))),
            )
            .into_any_element(),
        ];
        if !self.demo.collapsed {
            for (k, name) in files.iter().enumerate() {
                let armed = self.demo.armed && self.demo.selected == k;
                list.push(
                    kit::list_row(
                        format!("f-{k}"),
                        *name,
                        Some("12 KB · edited today".into()),
                        self.demo.selected == k,
                        Some(k + 1 == files.len()),
                        Some(
                            kit::arm_delete_button(
                                ("del", k),
                                armed,
                                cx,
                                Box::new(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.demo.armed = !this.demo.armed;
                                    this.demo.selected = k;
                                    cx.notify();
                                })),
                            )
                            .into_any_element(),
                        ),
                        cx,
                        Box::new(cx.listener(move |this, _, _, cx| {
                            this.demo.selected = k;
                            this.demo.armed = false;
                            cx.notify();
                        })),
                    )
                    .into_any_element(),
                );
            }
        }
        list.push(
            kit::group_header(
                "g-notes",
                Ico::FileText,
                "Notes",
                false,
                false,
                cx,
                Box::new(cx.listener(|_, _, _, _| {})),
                None,
            )
            .into_any_element(),
        );
        list.push(
            kit::list_row(
                "n-0",
                "Ideas",
                None,
                false,
                Some(true),
                None,
                cx,
                Box::new(cx.listener(|_, _, _, _| {})),
            )
            .into_any_element(),
        );
        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(kit::sidebar_top(vec![]))
            .child(kit::search_band(&self.search, cx))
            .child(
                v_flex()
                    .id("g-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(list),
            )
            .child(kit::bottom_bar(
                kit::primary_button(
                    "g-open",
                    Ico::Plus,
                    "Open",
                    cx,
                    Box::new(cx.listener(|_, _, _, _| {})),
                ),
                cx,
            ))
            .into_any_element()
    }

    pub(super) fn render_gallery_main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let tabs = ["main.rs", "app.rs", "theme.rs"]
            .iter()
            .enumerate()
            .map(|(k, n)| (SharedString::from(*n), k == self.demo.tab, k == 1))
            .collect();
        let (v1, v2) = (view.clone(), view.clone());
        let right = h_flex()
            .gap_1()
            .child(kit::icon_button(
                "g-pin",
                Ico::Pin,
                "Pin",
                true,
                cx,
                Box::new(cx.listener(|_, _, _, _| {})),
            ))
            .child(kit::icon_button(
                "g-copy",
                Ico::Copy,
                "Copy",
                false,
                cx,
                Box::new(cx.listener(|_, _, _, _| {})),
            ))
            .into_any_element();
        v_flex()
            .size_full()
            .child(kit::header_bar("Gallery", right, cx))
            .child(kit::tab_strip(
                "g-tabs",
                tabs,
                cx,
                Rc::new(move |k, _, cx| {
                    v1.update(cx, |this, cx| {
                        this.demo.tab = k;
                        cx.notify();
                    })
                }),
                Rc::new(move |_, _, cx| v2.update(cx, |_, cx| cx.notify())),
            ))
            .child(
                v_flex().px_4().child(
                    v_flex()
                        .max_w(px(640.))
                        .child(kit::settings_row(
                            "Wrap lines",
                            "Long lines wrap instead of scrolling sideways.",
                            kit::segmented(
                                "g-seg",
                                &["Off", "On", "Auto"],
                                self.demo.choice,
                                cx,
                                Rc::new(move |k, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.demo.choice = k;
                                        cx.notify();
                                    })
                                }),
                            )
                            .into_any_element(),
                            cx,
                        ))
                        .child(kit::settings_row(
                            "Font",
                            "Menlo, 12px — the code font.",
                            div().text_sm().child("Menlo").into_any_element(),
                            cx,
                        )),
                ),
            )
            .child(div().flex_1().child(kit::empty_state("Open a file to start", cx)))
            .into_any_element()
    }
}
