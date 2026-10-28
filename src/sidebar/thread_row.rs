//! A thread's row in the sidebar: its title, its state (running, unread, waiting), its mark, and
//! what it does on a click, a drag or a key.

use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SESSION_TITLE;
use super::menu::item_menu;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use crate::workspace::Center;
use bench_ui::icons::Ico;
use bench_ui::plural;

impl BenchApp {
    /// `guide`: under a group header, indented on the group's guide line (`Some(last row)`).
    pub(super) fn render_thread_row(
        &self,
        i: usize,
        guide: Option<bool>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let s = &self.threads.list[i];
        let selected =
            self.center == Center::Chat && self.thread(cx).id.as_ref() == Some(&s.id);
        let group: SharedString = format!("srow{i}").into();
        // The open thread knows its hold first-hand; the others by the agent's list and the
        // holds their views heard.
        let open = self.thread(cx).id.as_ref() == Some(&s.id);
        let paused = if open {
            self.thread(cx).journal.paused()
        } else {
            s.paused
        };
        let count = if s.running && paused {
            "paused".to_string()
        } else if s.running {
            "working…".to_string()
        } else {
            plural(s.messages as usize, "message", "messages", "messages")
        };
        // A chat in a branch of its own says which.
        let row_at = crate::thread::ThreadRef {
            host: s.host.clone(),
            id: Some(s.id.clone()),
            ..Default::default()
        };
        let meta = match self.repo_of(&row_at).filter(|p| p.in_branch_folder()) {
            Some(p) if !p.branch.is_empty() => format!("{} · {count}", p.branch),
            _ => count,
        };
        let (open_id, del_id) = (s.id.clone(), s.id.clone());
        // Deleting asks once (⌫, or the menu): the row shows Delete · Cancel; ⌫ again deletes.
        let armed = self.armed.is(&crate::app::armed::thread_key(s.id.as_str()));
        let compact = self.appearance.compact_sidebar;

        let delete = if armed {
            bench_ui::confirm_buttons(
                format!("del-{i}"),
                "Delete",
                Box::new(cx.listener(move |this, _, _, cx| this.delete_listed(del_id.clone(), cx))),
                Box::new(cx.listener(|this, _, _, cx| this.cancel_delete(cx))),
                cx,
            )
            .into_any_element()
        } else {
            // "⋯" on hover: pin, color, rename, delete.
            let view = cx.entity();
            let id = s.id.clone();
            let mark = self.mark_of(s.id.as_str());
            div()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                // Opening the menu must not also open the chat.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    bench_ui::row_icon_button(("more", i), Ico::Ellipsis)
                        .dropdown_menu(item_menu(
                            view.clone(),
                            Item::Thread(id.clone()),
                            mark.pinned,
                        )),
                )
                .into_any_element()
        };
        // A chosen color marks the row with a quiet stripe (the open chat keeps the accent).
        // A paused run stands still: no shimmer, as nothing moves.
        let working = !paused && (s.running || (open && self.thread(cx).live.is_some()));
        let stripe = self
            .mark_of(s.id.as_str())
            .color
            .and_then(bench_ui::mark_color);

        let pinned = self.mark_of(s.id.as_str()).pinned;
        let menu_id = s.id.clone();
        h_flex()
            .id(("thread", i))
            .group(group)
            .relative()
            .gap_1()
            // The title starts where the project's name does (the row's 2 px border is part
            // of that inset), so a project and its threads read as one column.
            .pl(px(bench_ui::PROJECT_NAME_X - bench_ui::ROW_STRIPE))
            .when_some(guide, |d, last| d.child(bench_ui::nest_guide(cx, last)))
            .pr_3()
            // The compact list packs rows closer; the ordinary one keeps a breath around
            // each title.
            .map(|d| if compact { d.py(px(bench_ui::THREAD_ROW_PY_COMPACT)) } else { d.py(px(bench_ui::THREAD_ROW_PY)) })
            .cursor_pointer()
            .border_l_2()
            // A chosen color wins over the accent; it also tints the row lightly.
            .border_color(stripe.unwrap_or(if selected {
                theme.primary
            } else {
                theme.transparent
            }))
            .when_some(stripe, |d, c| {
                d.bg(Hsla {
                    a: if selected { 0.22 } else { 0.12 },
                    ..c
                })
            })
            // The open chat in the accent's own tint, not one more grey.
            .when(selected && stripe.is_none(), |d| {
                d.bg(theme.primary.opacity(0.12))
                    .text_color(theme.foreground)
            })
            // Hover keeps a chosen color, half a step denser; plain rows get the usual grey.
            .hover(move |h| match stripe {
                Some(c) => h.bg(Hsla {
                    a: if selected { 0.30 } else { 0.20 },
                    ..c
                }),
                None => h.bg(theme.sidebar_accent),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    // A working session's title carries a moving highlight, to find it at a glance.
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(pinned, |d| {
                                d.child(Ico::Pin.icon().xsmall().text_color(muted))
                            })
                            .child(match self.renaming_field(&Item::Thread(s.id.clone())) {
                                // Renamed in place: the field where the title was.
                                // Over the title, in its place and size: the title stays (unseen) and
                                // keeps the row's height, so nothing moves.
                                Some(field) => div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_color(gpui_kit::transparent_black())
                                            .child(s.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .child(
                                                bench_ui::bare_field(&field)
                                                    // Input draws at 0.875 of its size: the title's.
                                                    .with_size(gpui_kit::component::Size::Size(px(
                                                        SESSION_TITLE / 0.875,
                                                    )))
                                                    .px_0(),
                                            ),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .truncate()
                                    .map(|d| {
                                        if working {
                                            d.child(ShimmerText::new(s.title.clone()))
                                        } else {
                                            d.child(s.title.clone())
                                        }
                                    })
                                    .into_any_element(),
                            }),
                    )
                    // Compact: the title alone (a working session still shimmers).
                    .when(!compact, |d| {
                        d.child(div().text_xs().text_color(muted).child(meta))
                    }),
            )
            .child(delete)
            // Opened, and the sidebar takes the keys: ↑ ↓ to the next, ⌫ deletes it.
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_listed(open_id.clone(), cx);
                window.focus(&this.sidebar.focus, cx);
            }))
            .context_menu(item_menu(cx.entity(), Item::Thread(menu_id), pinned))
    }
}
