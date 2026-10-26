//! SSH mode views: the mode switch beside the window buttons, the server list (folders from
//! `~/.ssh/config`), the header with the server's agent state, the server form.

use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;

impl BenchApp {
    /// The header's right end: a new window.
    pub(crate) fn render_header_tail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().flex_none().gap_1().items_center().child(
            bench_ui::icon_button("new-window", Ico::AppWindow, "New window  ⌘⇧N", false, cx)
                .on_click(cx.listener(|this, _, _, cx| this.open_new_window(cx))),
        )
    }

    /// A new (or edited) server, inside the thread: saved in ~/.ssh/config, then attached to it.
    pub(crate) fn render_server_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.ssh.form.clone().flatten();
        let f = |e: &Entity<InputState>| Input::new(e).into_any_element();
        let rows = vec![
            ("Name", f(&self.ssh.f_alias)),
            ("Host", f(&self.ssh.f_host)),
            ("User", f(&self.ssh.f_user)),
            ("Port", f(&self.ssh.f_port)),
            ("Key file", f(&self.ssh.f_key)),
            ("Folder", f(&self.ssh.f_path)),
        ];
        let actions = vec![
            crate::ui::form_action(
                "srv-save",
                if editing.is_some() {
                    "Save and connect"
                } else {
                    "Add and connect"
                },
                true,
                cx,
            )
            .on_click(cx.listener(|this, _, window, cx| this.save_server(window, cx)))
            .into_any_element(),
            crate::ui::form_action("srv-key", "Choose a key…", false, cx)
                .on_click(cx.listener(|this, _, window, cx| this.pick_key_file(window, cx)))
                .into_any_element(),
            crate::ui::form_action("srv-cancel", "Cancel", false, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.ssh.form = None;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        crate::ui::inline_form(
            if editing.is_some() { "Server" } else { "New server" },
            rows,
            Some("Saved in ~/.ssh/config (ssh signs in with your keys; no passwords are kept). Empty port = 22, empty folder = home.".into()),
            actions,
            cx,
        )
    }
}
