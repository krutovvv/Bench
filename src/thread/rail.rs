//! The ticks along the right edge: one per message of the person; the one being read is lit,
//! hover shows the message, a click brings it to the top.

use ::thread::Row;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use super::ThreadView;
use super::View;
use bench_ui::clip;

impl ThreadView {
    pub(crate) fn render_rail(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let users: Vec<(usize, String)> = self
            .thread
            .tv
            .rows
            .iter()
            .enumerate()
            .filter_map(|(k, r)| match r {
                Row::User { text, .. } => Some((k, text.clone())),
                _ => None,
            })
            .collect();
        if users.len() < 2 {
            return None;
        }
        let theme = cx.theme().clone();
        let top = self.thread.tv.list.logical_scroll_top().item_ix;
        let current = users
            .iter()
            .rposition(|(k, _)| View::item_of(*k) <= top)
            .unwrap_or(0);
        let pitch = (240. / users.len() as f32).clamp(3., 8.);
        let ticks = users.into_iter().enumerate().map(|(n, (k, text))| {
            let on = n == current;
            let tip: SharedString = clip(text.trim(), 120).into();
            let group = SharedString::from(format!("rail-tick-{n}"));
            div()
                .id(("rail", n))
                .h(px(pitch))
                .w_full()
                .flex()
                .items_center()
                .justify_end()
                .cursor_pointer()
                .group(group.clone())
                .child(
                    div()
                        .h(px(bench_ui::RAIL_TICK_H))
                        .w(px(bench_ui::RAIL_TICK_W))
                        .bg(if on {
                            theme.primary
                        } else {
                            theme.muted_foreground.opacity(0.35)
                        })
                        .group_hover(group, |s| s.bg(theme.foreground)),
                )
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.claim_click(cx);
                    this.thread.tv.list.scroll_to(ListOffset {
                        item_ix: View::item_of(k),
                        offset_in_item: px(-8.),
                    });
                    cx.notify();
                }))
        });
        Some(
            v_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right(px(bench_ui::RAIL_INSET))
                .w(px(bench_ui::RAIL_W))
                .justify_center()
                .child(v_flex().children(ticks))
                .into_any_element(),
        )
    }
}
