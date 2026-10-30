//! The menu of a sidebar item (its "⋯" and a right click), the same for a thread and a project,
//! with the colors' swatches.

use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

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
    // The menu reserves an icon column on the left (the other items have icons), which
    // pushes this row off-centre; pull it back by that width so the swatches sit symmetric.
    h_flex()
        .flex_1()
        .justify_center()
        .gap(Gap::M)
        .ml(px(-bench_ui::MENU_ICON_COL))
        .py(Gap::Xs)
        .children((0..bench_ui::MARK_COUNT).map(|c| swatch(Some(c))))
        .child(swatch(None))
        .into_any_element()
}

/// The menu of a sidebar item (its "⋯" and a right click), ordered as Finder's: what is done
/// most first (a project's new thread, its Code view, its Settings), then its name and pin, its
/// color, and its removal last. A thread has the same tail. (A thread opened beside another —
/// `open_beside` — is not offered here: two threads at once ask more of a local model than most
/// have; the Code view's thread panel is where a thread sits beside something.)
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
        // A thread is pinned; a project takes its place by a drag (`order.rs`). An agent's Host
        // keeps its name and is never removed (a server's agent leaves Bench in the gear).
        let project = match &item {
            Item::Project(id) => Some(crate::app::model::ProjectKey::parse(id)),
            Item::Thread(_) => None,
        };
        let host = project.as_ref().is_some_and(|k| k.is_host());
        let pinnable = matches!(item, Item::Thread(_));
        let menu = match &item {
            Item::Project(id) => {
                let (new_in, code, settings) = (id.clone(), id.clone(), id.clone());
                menu.item(
                    PopupMenuItem::new("New Thread")
                        .icon(Ico::Plus.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.new_thread_in(new_in.clone(), cx);
                            this.focus_composer(window, cx);
                        }))),
                )
                // Its files, prompts, skills and tools: the Code view, named as the window's
                // Chat | Code names it.
                .item(
                    PopupMenuItem::new("Code")
                        .icon(Ico::Code.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.show_explorer(code.clone(), window, cx)
                        }))),
                )
                // Its name, where it lives, model, Telegram.
                .item(
                    PopupMenuItem::new("Settings…")
                        .icon(Ico::Settings.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.open_project_settings(settings.clone(), window, cx)
                        }))),
                )
                .separator()
            }
            Item::Thread(_) => menu,
        };
        menu.when(!host, |menu| {
            menu.item(
                PopupMenuItem::new("Rename…")
                    .icon(Ico::Pencil.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_rename(rename.clone(), window, cx)
                    }))),
            )
        })
        .when(pinnable, |menu| {
            menu.item(
                PopupMenuItem::new(if pinned { "Unpin" } else { "Pin" })
                    .icon(Ico::Pin.icon())
                    .on_click(act(Box::new(move |this, _, cx| {
                        this.toggle_pin(pin_key.clone(), cx)
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
                    Item::Project(_) => "Remove Project…",
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
