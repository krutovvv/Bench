//! The center's tabs: dragged to another place (or onto the thread, which takes the file), a
//! menu each (close this, others, to the left, to the right, the saved, all; copy the path; open
//! beside; add to the thread), and what is sent from the code to the thread — the selection or
//! the whole file (⌘L) — as a reference (`app/attachments.rs` `CodeRef`).

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::*;

use super::{DocKey, FileRef};
use crate::app::{BenchApp, Center, RightPanel};
use crate::icons::Ico;

/// A tab being dragged: what it holds, and what the ghost under the pointer says.
#[derive(Clone)]
pub struct DraggedTab {
    pub key: DocKey,
    pub name: SharedString,
}

impl Render for DraggedTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .px_3()
            .py_1()
            .gap_1p5()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_sm()
            .child(Ico::File.icon().small())
            .child(self.name.clone())
    }
}

/// Which tabs a menu's close takes, from the one it was opened on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseTabs {
    Others,
    Left,
    Right,
    Saved,
    All,
}

impl BenchApp {
    /// A tab dropped on another: it takes that one's place.
    pub fn move_tab(&mut self, key: &DocKey, onto: &DocKey, cx: &mut Context<Self>) {
        let tabs = &mut self.explorer.tabs;
        let (Some(from), Some(to)) = (
            tabs.iter().position(|t| t == key),
            tabs.iter().position(|t| t == onto),
        ) else {
            return;
        };
        let t = tabs.remove(from);
        tabs.insert(to, t);
        cx.notify();
    }

    /// The menu's closes. A tab with unsaved changes is kept (its own × asks).
    pub fn close_tabs(&mut self, which: CloseTabs, from: &DocKey, cx: &mut Context<Self>) {
        let Some(at) = self.explorer.tabs.iter().position(|t| t == from) else {
            return;
        };
        let doomed: Vec<DocKey> = self
            .explorer
            .tabs
            .iter()
            .enumerate()
            .filter(|(k, _)| match which {
                CloseTabs::Others => *k != at,
                CloseTabs::Left => *k < at,
                CloseTabs::Right => *k > at,
                CloseTabs::Saved | CloseTabs::All => true,
            })
            .map(|(_, t)| t.clone())
            .collect();
        for key in doomed {
            if !self.explorer.docs.get(&key).is_some_and(|d| d.dirty) {
                self.close_doc(key, cx);
            }
        }
        cx.notify();
    }

    /// ⌘L: the selection of the file with the caret (or the whole file) goes to the thread's
    /// composer, and the thread shows beside the code.
    pub fn send_selection_to_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(DocKey::File(f)) = self.focused_doc(window, cx) else {
            return;
        };
        let Some(d) = self.explorer.docs.get(&DocKey::File(f.clone())) else {
            return;
        };
        let s = d.editor.read(cx);
        let range = s.selected_range();
        let r = if range.is_empty() {
            crate::app::attachments::CodeRef {
                path: f.path.clone(),
                lines: None,
                text: None,
            }
        } else {
            let text = s.value().to_string();
            crate::app::attachments::CodeRef {
                path: f.path.clone(),
                lines: Some(crate::app::attachments::lines_of(&text, range)),
                text: Some(s.selected_value().to_string()),
            }
        };
        self.add_reference(r, cx);
        self.show_thread_beside(window, cx);
    }

    /// A whole file to the thread (a tab's menu, a tab dropped on the thread).
    pub fn send_file_to_thread(
        &mut self,
        f: &FileRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.add_reference(
            crate::app::attachments::CodeRef {
                path: f.path.clone(),
                lines: None,
                text: None,
            },
            cx,
        );
        self.show_thread_beside(window, cx);
    }

    /// The thread in the right dock (unless it is the center already), its composer focused.
    fn show_thread_beside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.center != Center::Chat {
            if let Some(RightPanel::File(f)) = self.right.take() {
                self.right = Some(RightPanel::Thread);
                self.drop_unused(&DocKey::File(f));
            } else {
                self.right = Some(RightPanel::Thread);
            }
        }
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// The path copied, as the agent reaches it.
    fn copy_path(&mut self, f: &FileRef, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(f.path.clone()));
        self.toast(NotificationType::Info, "The path is copied", window, cx);
    }
}

/// A tab's menu.
pub fn tab_menu(
    view: Entity<BenchApp>,
    key: DocKey,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let act = |f: Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>| {
            let view = view.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| f(this, window, cx))
            }
        };
        let k = || key.clone();
        let closes = [
            ("Close", None),
            ("Close others", Some(CloseTabs::Others)),
            ("Close to the left", Some(CloseTabs::Left)),
            ("Close to the right", Some(CloseTabs::Right)),
            ("Close saved", Some(CloseTabs::Saved)),
            ("Close all", Some(CloseTabs::All)),
        ];
        let mut menu = menu;
        for (label, which) in closes {
            let key = k();
            menu = menu.item(PopupMenuItem::new(label).on_click(act(Box::new(
                move |this, _, cx| match which {
                    None => this.close_doc(key.clone(), cx),
                    Some(w) => this.close_tabs(w, &key, cx),
                },
            ))));
        }
        let DocKey::File(f) = key.clone() else {
            return menu;
        };
        let (f1, f2, f3) = (f.clone(), f.clone(), f);
        menu.separator()
            .item(
                PopupMenuItem::new("Add to the thread  ⌘L")
                    .icon(Ico::MessagesSquare.icon().small())
                    .on_click(act(Box::new(move |this, w, cx| {
                        this.send_file_to_thread(&f1, w, cx)
                    }))),
            )
            .item(
                PopupMenuItem::new("Open beside")
                    .icon(Ico::PanelRight.icon().small())
                    .on_click(act(Box::new(move |this, w, cx| {
                        this.open_beside(f2.clone(), w, cx)
                    }))),
            )
            .item(
                PopupMenuItem::new("Copy path")
                    .icon(Ico::Copy.icon().small())
                    .on_click(act(Box::new(move |this, w, cx| this.copy_path(&f3, w, cx)))),
            )
    }
}
