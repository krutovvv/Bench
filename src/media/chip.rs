//! A file going with a message, as a chip: a small picture or its icon, and its name.

use crate::app::BenchApp;
use crate::media::{self, Kind, file_name};
use bench_ui::icons::Ico;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl BenchApp {
    /// A file going with a message: a small picture for an image, else its icon, and the name.
    /// `remove`: its index in the composer (shows ×); otherwise a click opens it.
    pub(crate) fn attachment_chip(
        &self,
        id: SharedString,
        path: &str,
        remove: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let local = std::path::PathBuf::from(path);
        let here = local.is_file();
        let open = local.clone();
        let name = file_name(path);
        // The ✕ that takes a file back: a corner badge on a picture's square (the square is the
        // label, so nothing may crowd it), an end of the row on a named chip.
        let close = |id: SharedString, k: usize, badge: bool| {
            let b = div()
                .id(SharedString::from(format!("{id}-x")))
                .flex_none()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child(Ico::X.icon().xsmall())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.remove_attachment(k, cx);
                }));
            if badge {
                b.absolute()
                    .top(px(3.))
                    .right(px(3.))
                    .size(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .bg(theme.background.opacity(0.92))
                    .border_1()
                    .border_color(theme.border)
            } else {
                b.p_0p5()
            }
        };
        // A picture shows as a square of itself, without its name: the name tells nothing about
        // what is inside, and every square is the same size, so a line of them reads as one.
        if here && media::kind(path) == Kind::Image {
            let square = div()
                .id(id.clone())
                .relative()
                .size(px(bench_ui::THUMB))
                .flex_none()
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.border_color(theme.accent))
                .child(
                    img(local.clone())
                        .size(px(bench_ui::THUMB))
                        .object_fit(ObjectFit::Cover),
                )
                .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
                .on_click(move |_, _, cx| cx.open_with_system(&open));
            match remove {
                Some(k) => square.child(close(id, k, true)).into_any_element(),
                None => square.into_any_element(),
            }
        } else {
            let icon = match media::kind(path) {
                Kind::Image => Ico::Image,
                Kind::Audio => Ico::Music,
                Kind::Video => Ico::Film,
                Kind::Markdown => Ico::FileText,
                Kind::Doc | Kind::File => Ico::File,
            };
            let pill = h_flex()
                .id(id.clone())
                .max_w(px(260.))
                .h(px(bench_ui::CHIP_H))
                .pl_1()
                .pr_2()
                .gap_1p5()
                .items_center()
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                .bg(theme.background)
                .text_base()
                .child(
                    div()
                        .size(px(24.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .child(icon.icon().xsmall()),
                )
                .child(div().min_w_0().truncate().child(name))
                .map(|d| match remove {
                    Some(k) => d.child(close(id.clone(), k, false)),
                    None if here => d
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .on_click(move |_, _, cx| cx.open_with_system(&open)),
                    None => d,
                });
            pill.into_any_element()
        }
    }
}
