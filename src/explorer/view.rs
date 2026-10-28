//! The middle in `Center::Doc`: the open document as its `Item` draws it (`item.rs`), the tabs in
//! their own bar under the title bar, ⌘P's list over the middle. An editor fills its pane and
//! scrolls itself, as in an IDE.

use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The middle in `Center::Doc`: the active document.
    pub(crate) fn render_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.explorer.active.clone() {
            Some(key) => key.item().render(self, window, cx),
            None => note("Open a file from the tree, or find one with ⌘P", cx),
        };
        let theme = cx.theme().clone();
        v_flex()
            .size_full()
            // The center's tab bar, as a Zed pane's: under the window's title bar, which drags
            // the window (a tab dragged there moved it).
            .child(
                div()
                    .flex_none()
                    .h(bench_ui::bar_h(cx))
                    .bg(theme.title_bar)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.render_doc_tabs(cx)),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(body)
                    .children(self.render_quick(cx)),
            )
            .into_any_element()
    }

    /// The header in `Center::Doc`: a tab per open document.
    pub(crate) fn render_doc_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut tabs = vec![];
        for (n, key) in self.explorer.tabs.clone().into_iter().enumerate() {
            let open = self.explorer.active.as_ref() == Some(&key);
            let item = key.item();
            let (name, icon) = item.tab(self, cx);
            let dirty = item.dirty(self, cx);
            let armed = self.armed.is(&format!("close:{key:?}"));
            let (k1, k2) = (key.clone(), key.clone());
            let close = bench_ui::head_icon_button(
                &format!("doc-tab-x-{n}"),
                Ico::X,
                if armed {
                    "Close without saving"
                } else {
                    "Close  ⌘W"
                },
                cx,
            )
            .when(armed, |d| d.text_color(theme.danger))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.close_doc(k1.clone(), cx)
            }));
            let (drag, onto) = (
                super::tabs::DraggedTab {
                    key: key.clone(),
                    name: name.clone().into(),
                },
                key.clone(),
            );
            let primary = theme.primary;
            tabs.push(
                bench_ui::doc_tab(format!("doc-tab-{n}"), icon, name, open, dirty, close, cx)
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_doc(k2.clone(), window, cx)
                        }),
                    )
                    // Dragged onto another tab it takes that place; onto the thread, the file
                    // goes to it.
                    .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<super::tabs::DraggedTab>(move |s, _, _, _| {
                        s.border_l_2().border_color(primary)
                    })
                    .on_drop(
                        cx.listener(move |this, d: &super::tabs::DraggedTab, _, cx| {
                            this.move_tab(&d.key, &onto, cx)
                        }),
                    )
                    .context_menu(super::tabs::tab_menu(cx.entity(), key.clone()))
                    .into_any_element(),
            );
        }
        h_flex()
            .id("doc-tabs")
            .size_full()
            .overflow_x_scroll()
            .children(tabs)
            .into_any_element()
    }

    /// ⌘P over the middle: the field and the files matching it.
    fn render_quick(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let q = self.explorer.quick.as_ref()?;
        let theme = cx.theme().clone();
        let input = q.input.clone();
        let (sel, listed) = (q.sel, q.files.is_some());
        let matches = self.quick_matches(cx);
        let rows = matches.into_iter().enumerate().map(|(n, rel)| {
            let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
            let (ico, color) = bench_ui::file_icon(&name, false, &theme);
            let pick = n;
            h_flex()
                .id(("quick", n))
                .px_3()
                .py_1()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .when(n == sel, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(ico.icon().text_color(color))
                .child(div().flex_none().child(name))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(rel.clone()),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(q) = this.explorer.quick.as_mut() {
                        q.sel = pick;
                    }
                    this.quick_open_chosen(window, cx)
                }))
                .into_any_element()
        });
        Some(
            div()
                .absolute()
                .top_2()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    v_flex()
                        .id("quick")
                        .w(px(bench_ui::QUICK_W))
                        .max_h(px(bench_ui::QUICK_MAX_H))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .shadow_lg()
                        .overflow_hidden()
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            match ev.keystroke.key.as_str() {
                                "down" => this.quick_move(1, cx),
                                "up" => this.quick_move(-1, cx),
                                _ => return,
                            }
                            cx.stop_propagation();
                        }))
                        .child(div().p_2().child(Input::new(&input)))
                        .child(
                            div()
                                .id("quick-list")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .pb_1()
                                .when(!listed, |d| d.child(note("Listing the folder…", cx)))
                                .children(rows),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// One muted line, padded.
pub(super) fn note(text: &str, cx: &App) -> AnyElement {
    div()
        .p_3()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}
