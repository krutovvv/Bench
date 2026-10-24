//! Bottom bar: 28px, one muted tone for every label and icon. Facts on the left; on the right
//! only the window's own buttons (New window, Settings) — mode toggles live over the sidebar.

use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;

use super::kit;
use crate::app::{MainApp, Panel};
use crate::icons::Ico;

impl MainApp {
    pub(super) fn render_statusbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let settings_on = self.panel == Panel::Settings;
        h_flex()
            .h(px(28.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .text_color(muted)
            .child(kit::status_text(match self.items.len() {
                1 => "1 item".to_string(),
                n => format!("{n} items"),
            }))
            .child(div().flex_1())
            .child(kit::status_button(
                "settings",
                Ico::Settings,
                "Settings",
                settings_on,
                cx,
                Box::new(cx.listener(|this, _, _, cx| this.toggle_settings(cx))),
            ))
    }
}
