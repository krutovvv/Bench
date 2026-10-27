//! The tree's context menus, as an IDE has them: a file's (open, add to the thread, rename,
//! delete, copy the path, show in Finder, open in Zed), a folder's (new file, new folder, and the
//! same), the project folder's (also find in it, read it again), the prompts' and skills' (a new
//! one). The rows themselves carry no buttons.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;

use super::{DocKey, FileRef, Named};
use crate::app::BenchApp;
use bench_ui::icons::Ico;

type Act = Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>;

/// A menu item's click, run on the app.
fn act(view: &Entity<BenchApp>, f: Act) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let view = view.clone();
    move |_, window, cx| view.update(cx, |this, cx| f(this, window, cx))
}

/// What a file or folder of the tree offers. `editable`: it may be renamed and deleted (a skill
/// of the folder or of this Mac is changed where it lives, not from another project).
pub fn entry_menu(
    view: Entity<BenchApp>,
    f: FileRef,
    dir: bool,
    editable: bool,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, cx| {
        let mut menu = menu;
        let here = view.read(cx).is_this_mac(&f);
        if dir {
            if editable {
                let (a, b) = (f.clone(), f.clone());
                menu = menu
                    .item(
                        PopupMenuItem::new("New file")
                            .icon(Ico::FilePlus.icon())
                            .on_click(act(
                                &view,
                                Box::new(move |this, w, cx| {
                                    this.start_naming(Named::File(a.clone()), w, cx)
                                }),
                            )),
                    )
                    .item(
                        PopupMenuItem::new("New folder")
                            .icon(Ico::FolderPlus.icon())
                            .on_click(act(
                                &view,
                                Box::new(move |this, w, cx| {
                                    this.start_naming(Named::Folder(b.clone()), w, cx)
                                }),
                            )),
                    )
                    .separator();
            }
        } else {
            let (a, b) = (f.clone(), f.clone());
            menu = menu
                .item(PopupMenuItem::new("Open").on_click(act(
                    &view,
                    Box::new(move |this, w, cx| this.open_doc(DocKey::File(a.clone()), w, cx)),
                )))
                .item(
                    PopupMenuItem::new("Add to the thread")
                        .icon(Ico::MessagesSquare.icon())
                        .on_click(act(
                            &view,
                            Box::new(move |this, w, cx| this.send_file_to_thread(&b, w, cx)),
                        )),
                )
                .separator();
        }
        if editable {
            let (a, b) = (f.clone(), f.clone());
            menu = menu
                .item(
                    PopupMenuItem::new("Rename")
                        .icon(Ico::Pencil.icon())
                        .on_click(act(
                            &view,
                            Box::new(move |this, w, cx| {
                                this.start_naming(Named::Rename(a.clone()), w, cx)
                            }),
                        )),
                )
                .item(
                    PopupMenuItem::new("Delete…")
                        .icon(Ico::Trash.icon())
                        .on_click(act(
                            &view,
                            Box::new(move |this, w, cx| this.delete_entry(b.clone(), w, cx)),
                        )),
                )
                .separator();
        }
        place_items(menu, &view, &f, here)
    }
}

/// What the project's folder offers, on its heading.
pub fn root_menu(
    view: Entity<BenchApp>,
    root: FileRef,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, cx| {
        let here = view.read(cx).is_this_mac(&root);
        let (a, b) = (root.clone(), root.clone());
        let menu = menu
            .item(
                PopupMenuItem::new("New file")
                    .icon(Ico::FilePlus.icon())
                    .on_click(act(
                        &view,
                        Box::new(move |this, w, cx| {
                            this.start_naming(Named::File(a.clone()), w, cx)
                        }),
                    )),
            )
            .item(
                PopupMenuItem::new("New folder")
                    .icon(Ico::FolderPlus.icon())
                    .on_click(act(
                        &view,
                        Box::new(move |this, w, cx| {
                            this.start_naming(Named::Folder(b.clone()), w, cx)
                        }),
                    )),
            )
            .separator()
            .item(
                PopupMenuItem::new("Find in the folder  ⌘⇧F")
                    .icon(Ico::Search.icon())
                    .on_click(act(&view, Box::new(|this, w, cx| this.open_find(w, cx)))),
            )
            .item(
                PopupMenuItem::new("Problems  ⌘⇧M")
                    .icon(Ico::CircleAlert.icon())
                    .on_click(act(&view, Box::new(|this, w, cx| this.open_problems(w, cx)))),
            )
            .item(
                PopupMenuItem::new("Read it again")
                    .icon(Ico::RefreshCw.icon())
                    .on_click(act(
                        &view,
                        Box::new(|this, w, cx| this.refresh_explorer(w, cx)),
                    )),
            )
            .separator();
        place_items(menu, &view, &root, here)
    }
}

/// A group of the agent's texts: a new one (`Named::Prompt` or `Named::Skill`).
pub fn new_menu(
    view: Entity<BenchApp>,
    what: Named,
    label: &'static str,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let what = what.clone();
        menu.item(
            PopupMenuItem::new(label)
                .icon(Ico::FilePlus.icon())
                .on_click(act(
                    &view,
                    Box::new(move |this, w, cx| this.start_naming(what.clone(), w, cx)),
                )),
        )
    }
}

/// Where else an entry is: its path copied, Finder (on this Mac), Zed (on its machine).
fn place_items(menu: PopupMenu, view: &Entity<BenchApp>, f: &FileRef, here: bool) -> PopupMenu {
    let (a, b, c) = (f.clone(), f.clone(), f.clone());
    let menu = menu.item(
        PopupMenuItem::new("Copy path")
            .icon(Ico::Copy.icon())
            .on_click(act(
                view,
                Box::new(move |this, w, cx| this.copy_path(&a, w, cx)),
            )),
    );
    let menu = if here {
        menu.item(
            PopupMenuItem::new("Show in Finder")
                .icon(Ico::FolderOpen.icon())
                .on_click(act(
                    view,
                    Box::new(move |this, w, cx| this.reveal_in_finder(&b.path, w, cx)),
                )),
        )
    } else {
        menu
    };
    menu.item(
        PopupMenuItem::new("Open in Zed")
            .icon(Ico::Zed.icon())
            .on_click(act(
                view,
                Box::new(move |this, _, cx| {
                    let host = this.zed_host(&c);
                    this.open_in_zed(&c.path, None, host.as_deref(), cx)
                }),
            )),
    )
}

impl BenchApp {
    /// Whether a file is on this Mac (Finder can show it).
    pub fn is_this_mac(&self, f: &FileRef) -> bool {
        self.zed_host(f).is_none()
    }

    /// The machine Zed opens a file on: the project's over SSH, or the agent's server; None here.
    pub fn zed_host(&self, f: &FileRef) -> Option<String> {
        let at = self.location(&f.project);
        f.machine
            .clone()
            .or_else(|| (!at.agent.is_empty()).then(|| at.agent.clone()))
    }
}
