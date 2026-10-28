//! A thread's row in the sidebar: its title, its state (running, unread, waiting), its mark, and
//! what it does on a click, a drag or a key.

use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::menu::item_menu;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use crate::workspace::Center;
use bench_ui::SIDEBAR_TEXT;
use bench_ui::icons::Ico;

impl BenchApp {
    pub(super) fn render_thread_row(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let accent = theme.primary;
        let faint = bench_ui::faint(&theme);
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
        // One line: the title, and on its right only what is not plain — a paused run, or the
        // branch of a chat that works in a branch folder of its own. A working run says so by its
        // title's shimmer; a count of messages told nothing worth the second line.
        let row_at = crate::thread::ThreadRef {
            host: s.host.clone(),
            id: Some(s.id.clone()),
            ..Default::default()
        };
        let branch = self
            .repo_of(&row_at)
            .filter(|p| p.in_branch_folder() && !p.branch.is_empty())
            .map(|p| p.branch.clone());
        // Its run waits for the person (a question, an approval): a dot in the accent, the one
        // thing in the list that asks for a click. The open thread knows it first-hand.
        let waiting = if open {
            !self.thread(cx).journal.asks().is_empty()
        } else {
            s.waiting
        };
        let meta: Option<String> = if s.running && paused {
            Some("paused".into())
        } else {
            branch
        };
        let (open_id, del_id) = (s.id.clone(), s.id.clone());
        // Deleting asks once (⌫, or the menu): the row shows Delete · Cancel; ⌫ again deletes.
        let armed = self.armed.is(&crate::app::armed::thread_key(s.id.as_str()));

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
        // A chosen color is a small dot after the title, as a Finder tag: the row itself stays
        // the list's. A paused run stands still: no shimmer, as nothing moves.
        let working = !paused && (s.running || (open && self.thread(cx).live.is_some()));
        let tag = self
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
            // A rounded row set in from the sidebar's edges (the open one a soft pill, as a
            // Mac's source list); its title starts where the project's name does, so a project
            // and its threads read as one column (a Mac's sidebar: rows flush with their header).
            .mx(px(bench_ui::SIDEBAR_ROW_INSET))
            .rounded_md()
            .pl(px(bench_ui::SIDEBAR_TEXT_X - bench_ui::SIDEBAR_ROW_INSET))
            .pr_2()
            .py(px(bench_ui::THREAD_ROW_PY))
            .cursor_pointer()
            // The open chat: the sidebar's selected ground; hovered: the list's.
            .when(selected, |d| d.bg(theme.sidebar_accent).text_color(theme.foreground))
            .when(!selected, |d| d.hover(|h| h.bg(theme.list_hover)))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    // A working session's title carries a moving highlight, to find it at a glance.
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .items_center()
                            .child(match self.renaming_field(&Item::Thread(s.id.clone())) {
                                // Renamed in place: the field where the title was.
                                // Over the title, in its place and size: the title stays (unseen) and
                                // keeps the row's height, so nothing moves.
                                Some(field) => div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(SIDEBAR_TEXT))
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
                                                        SIDEBAR_TEXT / 0.875,
                                                    )))
                                                    .px_0(),
                                            ),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .min_w_0()
                                    .text_size(px(SIDEBAR_TEXT))
                                    .truncate()
                                    .map(|d| {
                                        if working {
                                            d.child(ShimmerText::new(s.title.clone()))
                                        } else {
                                            d.child(s.title.clone())
                                        }
                                    })
                                    .into_any_element(),
                            })
                            .when_some(tag, |d, c| d.child(bench_ui::tag_dot(c)))
                            // Pinned: a small pin after the name, with the row's other marks.
                            .when(pinned, |d| {
                                d.child(Ico::Pin.icon().xsmall().text_color(faint))
                            }),
                    )
                    // Waiting for an answer: two words in the accent.
                    .when(waiting, |d| {
                        d.child(
                            div()
                                .flex_none()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(accent)
                                .child("needs you"),
                        )
                    })
                    // A paused run, or the branch of a chat in its own branch folder.
                    .when_some(meta, |d, meta| {
                        d.child(div().flex_none().text_xs().text_color(faint).child(meta))
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
