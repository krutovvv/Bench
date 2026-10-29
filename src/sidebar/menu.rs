//! The menu of a sidebar item (its "⋯" and a right click), the same for a thread and a project,
//! with the colors' swatches.

use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::sidebar::items::Item;
use bench_ui::icons::Ico;

/// The swatches row of a session's menu: the colors and "none".
pub(super) fn color_row(
    view: &Entity<BenchApp>,
    id: &str,
    current: Option<usize>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let swatch = |k: Option<usize>| {
        let (view, id) = (view.clone(), id.to_string());
        let on = current == k;
        div()
            .id(SharedString::from(format!("swatch-{k:?}")))
            .size(px(bench_ui::SWATCH_CELL))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .border_1()
            .rounded_md()
            .border_color(if on {
                theme.foreground
            } else {
                theme.transparent
            })
            .map(|d| match k {
                Some(c) => d.child(
                    div()
                        .size(px(bench_ui::SWATCH))
                        .bg(bench_ui::mark_color(c).unwrap_or_default()),
                ),
                None => d
                    .text_color(theme.muted_foreground)
                    .child(Ico::X.icon().xsmall()),
            })
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                view.update(cx, |this, cx| this.set_color(id.clone(), k, cx));
            })
    };
    // The menu reserves a 16px icon column on the left (the other items have icons), which
    // pushes this row off-centre; pull it back by that width so the swatches sit symmetric.
    h_flex()
        .flex_1()
        .justify_center()
        .gap_2()
        .ml(px(-16.))
        .py_1()
        .children((0..bench_ui::MARK_COUNT).map(|c| swatch(Some(c))))
        .child(swatch(None))
        .into_any_element()
}

/// The menu of a sidebar item (its "⋯" and a right click), the same for a thread and a
/// project: a project's own entries (its Code view, its Settings), then pin, color, rename, delete.
pub(super) fn item_menu(
    view: Entity<BenchApp>,
    item: Item,
    pinned: bool,
) -> impl Fn(
    gpui_kit::component::menu::PopupMenu,
    &mut Window,
    &mut Context<gpui_kit::component::menu::PopupMenu>,
) -> gpui_kit::component::menu::PopupMenu
+ 'static {
    move |menu, _, _| {
        let act = |f: Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>| {
            let view = view.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| f(this, window, cx))
            }
        };
        let key = item.mark_key();
        let (pin_key, swatch_view, swatch_key) = (key.clone(), view.clone(), key.clone());
        let (rename, del) = (item.clone(), item.clone());
        let menu = match &item {
            Item::Project(id) => {
                // Its files, prompts, skills and tools: the Code view (the window's Chat | Code,
                // named as it is there); its Settings (name, where it lives, model, Telegram) —
                // the gear the row had, kept in its menu.
                menu.item({
                    let id = id.clone();
                    PopupMenuItem::new("Code")
                        .icon(Ico::Code.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.show_explorer(id.clone(), window, cx)
                        })))
                })
                .item({
                    let id = id.clone();
                    PopupMenuItem::new("Settings")
                        .icon(Ico::Settings.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.open_project_settings(id.clone(), window, cx)
                        })))
                })
                .separator()
            }
            // A second thread beside the open one (or beside the document in the center).
            Item::Thread(id) => {
                let id = id.clone();
                menu.item(
                    PopupMenuItem::new("Open beside")
                        .icon(Ico::PanelRight.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.open_beside(id.clone(), window, cx)
                        }))),
                )
                .separator()
            }
        };
        // This Mac's Host is always first: no pin. An agent's Host is never removed (a server's
        // agent leaves Bench in the gear).
        let key_of = |i: &Item| match i {
            Item::Project(id) => Some(crate::app::model::ProjectKey::parse(id)),
            Item::Thread(_) => None,
        };
        let host = key_of(&item).is_some_and(|k| k.is_host());
        let pinnable = !key_of(&item).is_some_and(|k| k.is_host() && k.agent.is_empty());
        menu.when(pinnable, |menu| {
            menu.item(
                PopupMenuItem::new(if pinned { "Unpin" } else { "Pin" })
                    .icon(Ico::Pin.icon())
                    .on_click(act(Box::new(move |this, _, cx| {
                        this.toggle_pin(pin_key.clone(), cx)
                    }))),
            )
        })
        // A thread is renamed here; a project in its environment (its Name).
        .when(matches!(rename, Item::Thread(_)), |menu| {
            menu.item(
                PopupMenuItem::new("Rename")
                    .icon(Ico::Pencil.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_rename(rename.clone(), window, cx)
                    }))),
            )
        })
        .separator()
        // Reads the color live, so a click shows at once in the open menu.
        .item(PopupMenuItem::element(move |_, cx| {
            let current = swatch_view.read(cx).mark_of(&swatch_key).color;
            color_row(&swatch_view, &swatch_key, current, cx)
        }))
        .when(!host, |menu| {
            menu.separator().item(
                PopupMenuItem::new(match del {
                    Item::Thread(_) => "Delete…",
                    Item::Project(_) => "Remove project…",
                })
                .icon(Ico::Trash.icon())
                .on_click(act(Box::new(move |this, window, cx| match &del {
                    Item::Thread(id) => this.arm_delete(id.to_string(), cx),
                    Item::Project(id) => this.delete_project(id.clone(), window, cx),
                }))),
            )
        })
    }
}
