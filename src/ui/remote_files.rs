//! A file host's block at the top of its thread: the folder, its entries (folders first), and
//! the open file in an editor under the list.

use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;

/// The editor's height in a titled block (in an insert the open file is as tall as its lines).
const EDITOR_H: f32 = 420.;

fn size_label(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.0} KB", n as f64 / 1024.),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.),
    }
}

impl BenchApp {
    pub(super) fn render_files_block(&mut self, id: &str, titled: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_remote_listing(id, window, cx);
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let f = &self.files_remote;
        let host = f.hosts.iter().find(|h| h.id == id).cloned();
        let title = host
            .as_ref()
            .map(|h| format!("{} · {}", h.name, h.kind.to_uppercase()))
            .unwrap_or_else(|| "Files".into());
        let (path, entries) = (f.path.clone(), f.entries.clone());
        let mut rows: Vec<AnyElement> = vec![];
        if path != "/" && !path.is_empty() {
            rows.push(self.file_row(0, "..".into(), true, String::new(), String::new(), cx));
        }
        for (k, e) in entries.into_iter().enumerate() {
            let size = if e.dir { String::new() } else { size_label(e.size) };
            rows.push(self.file_row(k + 1, e.name, e.dir, size, e.modified, cx));
        }
        let f = &self.files_remote;
        let note = if let Some(e) = &f.error {
            // A refused login: a password field, or the one Transmit keeps in the keychain.
            let login = ["login", "530", "501", "auth", "password"].iter().any(|w| e.to_lowercase().contains(w));
            Some(
                v_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .child(div().text_base().text_color(theme.danger).child(e.clone()))
                    .when(login, |d| {
                        d.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().w(px(280.)).child(gpui_kit::component::input::Input::new(&self.files_remote.password).small()))
                                .child(
                                    gpui_kit::component::button::Button::new("pw-save")
                                        .small()
                                        .map(|b| { use gpui_kit::component::button::ButtonVariants as _; b.primary() })
                                        .label("Connect")
                                        .on_click(cx.listener(|this, _, window, cx| this.save_host_password(window, cx))),
                                )
                                .child(
                                    gpui_kit::component::button::Button::new("pw-keychain")
                                        .small()
                                        .map(|b| { use gpui_kit::component::button::ButtonVariants as _; b.ghost() })
                                        .label("Password from Keychain")
                                        .on_click(cx.listener(|this, _, window, cx| this.password_from_keychain(window, cx))),
                                ),
                        )
                    })
                    .into_any_element(),
            )
        } else if f.loading && f.entries.is_empty() {
            Some(div().px_3().py_2().child(ShimmerText::new("Connecting…").text_base().text_color(muted)).into_any_element())
        } else if f.entries.is_empty() {
            Some(div().px_3().py_2().text_base().text_color(muted).child("Empty folder").into_any_element())
        } else {
            None
        };
        let open = f.file.clone();
        let dirty = f.dirty;
        v_flex()
            .w_full()
            .gap_2()
            .when(titled, |d| {
                d.child(
                    h_flex()
                        .gap_3()
                        .items_baseline()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(div().flex_1().min_w_0().truncate().text_base().text_color(muted).child(f.path.clone())),
                )
            })
            // As long as the folder is: the thread scrolls, not the list (no scroll inside a
            // scroll), and no frame inside the insert's frame.
            .child(v_flex().id("remote-list").mx(px(-12.)).children(rows).children(note))
            .when_some(open.filter(|_| titled), |d, path| {
                d.child(
                    v_flex()
                        .border_1().rounded_md()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .child(
                            super::block_head(&theme, true)
                                .child(Ico::File.icon().small().text_color(muted))
                                .child(div().flex_1().min_w_0().truncate().text_color(theme.foreground).child(path))
                                .child(
                                    gpui_kit::component::button::Button::new("remote-save")
                                        .small()
                                        .map(|b| {
                                            use gpui_kit::component::button::ButtonVariants as _;
                                            if dirty { b.primary() } else { b.ghost() }
                                        })
                                        .label("Save")
                                        .on_click(cx.listener(|this, _, window, cx| this.save_host_file(window, cx))),
                                ),
                        )
                        .child(div().h(px(EDITOR_H)).child(Editor::new(&self.files_remote.editor).appearance(false).h_full())),
                )
            })
            .into_any_element()
    }

    /// The open file as a block of its own, under the insert: its path, Save and Close on top,
    /// the editor at the file's full height (the thread scrolls, not the editor).
    pub(super) fn render_open_file(&mut self, close_id: String, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let path = self.files_remote.file.clone()?;
        let dirty = self.files_remote.dirty;
        let lines = self.files_remote.editor.read(cx).value().lines().count().max(1);
        let head = super::block_head(&theme, true)
            .child(Ico::File.icon().small().text_color(muted))
            .child(div().flex_1().min_w_0().truncate().text_color(theme.foreground).child(path))
            .child(
                gpui_kit::component::button::Button::new("remote-save")
                    .small()
                    .map(|b| {
                        use gpui_kit::component::button::ButtonVariants as _;
                        if dirty { b.primary() } else { b.ghost() }
                    })
                    .label("Save")
                    .on_click(cx.listener(|this, _, window, cx| this.save_host_file(window, cx))),
            )
            .child(super::inserts::icon_button(&close_id, Ico::X, "Close the file", &theme).on_click(cx.listener(|this, _, _, cx| {
                this.click_claimed = true;
                if !this.files_remote.dirty {
                    this.files_remote.file = None;
                }
                cx.notify();
            })));
        let body = div()
            .h(px(lines as f32 * super::editor_line(cx) + 12.))
            .child(Editor::new(&self.files_remote.editor).appearance(false).h_full())
            .into_any_element();
        Some(super::inserts::card(head, body, &theme))
    }

    fn file_row(&mut self, k: usize, name: String, dir: bool, size: String, modified: String, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let n = name.clone();
        h_flex()
            .id(("remote-row", k))
            .px_3()
            .py_1()
            .gap_2()
            .items_center()
            .text_base()
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_hover))
            .child(
                if dir { Ico::Folder } else { Ico::File }
                    .icon()
                    .small()
                    .text_color(muted),
            )
            .child(div().flex_1().min_w_0().truncate().child(name))
            .child(div().flex_none().text_base().text_color(muted).child(size))
            .child(div().flex_none().w(px(130.)).text_base().text_color(muted).child(modified))
            .on_click(cx.listener(move |this, _, window, cx| this.open_remote_entry(n.clone(), dir, window, cx)))
            .into_any_element()
    }
}

impl BenchApp {
    /// A new file host, inside the thread: into the agent's store (this project's), then attached.
    pub(super) fn render_host_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let f = &self.files_remote;
        let kind = super::tools::segmented("host-kind", &["FTP", "FTPS", "SFTP"], f.f_kind, cx, |this, k, _, cx| {
            this.files_remote.f_kind = k;
            cx.notify();
        });
        let field = |e: &Entity<gpui_kit::component::input::InputState>| Input::new(e).into_any_element();
        let rows = vec![
            ("Name", field(&f.f_name)),
            ("Protocol", kind),
            ("Host", field(&f.f_host)),
            ("Port", field(&f.f_port)),
            ("User", field(&f.f_user)),
            ("Password", field(&f.f_password)),
            ("Folder", field(&f.f_path)),
        ];
        let actions = vec![
            super::form_action("host-save", "Add and open", true, cx)
                .on_click(cx.listener(|this, _, window, cx| this.save_host(window, cx)))
                .into_any_element(),
            super::form_action("host-cancel", "Cancel", false, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.files_remote.form = false;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        super::inline_form("New file host", rows, Some("Empty name = the address; empty port = 21 (FTP, FTPS) or 22 (SFTP). The password is kept in the agent's store.".into()), actions, cx)
    }
}
