//! The chat area while Hermes is missing on this Mac (an offer to install it) or while a setup
//! runs in a terminal (the install, a Claude login).

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::HermesApp;
use crate::app::install::SetupKind;
use crate::connection;
use crate::icons::Ico;

impl HermesApp {
    /// The setup's terminal under a strip: what runs, Try again, Close.
    pub(super) fn render_setup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let kind = self.setup.as_ref()?.kind;
        let ix = self.setup_tab()?;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let exited = self.setup_exited();
        let status: SharedString = if exited {
            self.ssh.tabs[ix]
                .session
                .exit_note
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_default()
                .into()
        } else {
            "Running · if it stops on an error, the log is here".into()
        };
        let button = |id: &'static str, label: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .text_xs()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| match id {
                    "setup-again" => this.start_setup(kind, window, cx),
                    _ => this.close_setup(cx),
                }))
        };
        let strip = h_flex()
            .flex_none()
            .h(px(36.))
            .px_3()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .text_xs()
            .child(div().text_color(theme.foreground).child(kind.title()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(muted)
                    .child(status),
            )
            .child(button("setup-again", "Try again", cx))
            .child(button("setup-close", "Close", cx));
        Some(
            v_flex()
                .size_full()
                .child(strip)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_terminal(ix, window, cx)),
                )
                .into_any_element(),
        )
    }

    /// No Hermes on this Mac: what it is, the button, the command it runs.
    pub(super) fn render_hermes_missing(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .px_6()
            .child(Ico::CircleAlert.icon().text_color(muted))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Hermes Agent is not installed on this Mac"),
            )
            .child(
                div()
                    .max_w(px(520.))
                    .text_sm()
                    .text_color(muted)
                    .text_center()
                    .child(
                        "Bench talks to the Hermes agent. Install it here: the official installer \
                         runs in a terminal below, with its whole log, and brings what it needs \
                         (Python, Node, ripgrep, ffmpeg). Then set the model in Settings → Providers.",
                    ),
            )
            .child(
                h_flex()
                    .id("install-hermes-main")
                    .mt_2()
                    .h(rems(super::BAR_CONTROL_REMS))
                    .px_4()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.primary_hover))
                    .child(Ico::Download.icon().small())
                    .child("Install Hermes Agent")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.start_setup(SetupKind::Hermes, window, cx)
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .font_family("Menlo")
                    .text_color(muted)
                    .child(connection::INSTALL_HERMES),
            )
            .into_any_element()
    }
}
