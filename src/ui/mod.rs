//! Rendering. Each file adds `render_*` methods to `HermesApp`.

mod chat;
mod composer;
mod panels;
mod sidebar;
mod statusbar;

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{HermesApp, Panel};

pub fn plural(n: usize, one: &str, few: &str, many: &str) -> String {
    let (m10, m100) = (n % 10, n % 100);
    let word = if m10 == 1 && m100 != 11 {
        one
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        few
    } else {
        many
    };
    format!("{n} {word}")
}

pub fn clock(secs: u64) -> String {
    if secs >= 60 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{}k", n / 1000),
        n => n.to_string(),
    }
}

pub fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

impl HermesApp {
    /// Header strip: the session title, or the settings tabs.
    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bar = h_flex()
            .id("header")
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .items_center()
            .border_b_1()
            .border_color(theme.border);

        if !self.panel.is_settings() {
            let title: SharedString = self
                .session
                .as_ref()
                .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Новая сессия".into())
                .into();
            return bar.px_4().child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(title),
            );
        }

        let current = self.panel;
        let tabs = [
            ("tab-skills", "Скиллы", Panel::Skills),
            ("tab-mcp", "MCP", Panel::Mcp),
            ("tab-config", "Конфиг", Panel::Config),
            ("tab-cron", "Cron", Panel::Cron),
            ("tab-tg", "Telegram", Panel::Telegram),
        ]
        .map(|(id, label, panel)| {
            let active = panel == current;
            div()
                .id(id)
                .h_full()
                .px_4()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_r_1()
                .border_color(theme.border)
                .when(active, |d| {
                    d.bg(theme.background).text_color(theme.foreground)
                })
                .when(!active, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                })
                .child(label)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.open_panel(panel, window, cx)),
                )
        });
        bar.children(tabs).child(div().flex_1()).child(
            div()
                .id("settings-close")
                .px_3()
                .h_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child(crate::icons::Ico::X.icon().small())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.panel = Panel::Chat;
                    cx.notify();
                })),
        )
    }
}

impl Render for HermesApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = match self.panel {
            Panel::Chat => v_flex()
                .size_full()
                .child(div().flex_1().min_h_0().child(self.render_chat(cx)))
                .child(self.render_composer(window, cx))
                .into_any_element(),
            Panel::Skills => self.render_skills(cx).into_any_element(),
            Panel::Mcp => self.render_mcp(cx).into_any_element(),
            Panel::Config => self.render_config(cx).into_any_element(),
            Panel::Cron => self.render_cron(cx).into_any_element(),
            Panel::Telegram => self.render_telegram(cx).into_any_element(),
        };

        let main = v_flex()
            .size_full()
            .min_w_0()
            .child(self.render_header(cx))
            .child(div().flex_1().min_h_0().child(body));

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("layout")
                        .child(
                            resizable_panel()
                                .size(px(280.))
                                .size_range(px(200.)..px(480.))
                                .child(self.render_sidebar(cx)),
                        )
                        .child(main.into_any_element()),
                ),
            )
            .child(self.render_statusbar(window, cx))
    }
}
