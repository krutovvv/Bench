//! Rendering. Each file adds `render_*` methods to `HermesApp`.

mod chat;
mod composer;
mod panels;
mod sidebar;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
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

impl Render for HermesApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let title: SharedString = match self.panel {
            Panel::Skills => "Скиллы".into(),
            Panel::Mcp => "MCP-серверы".into(),
            Panel::Chat => self
                .session
                .as_ref()
                .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Новая сессия".into())
                .into(),
        };

        let body = match self.panel {
            Panel::Chat => v_flex()
                .size_full()
                .child(div().flex_1().min_h_0().child(self.render_chat(cx)))
                .child(self.render_composer(window, cx))
                .into_any_element(),
            Panel::Skills => self.render_skills(cx).into_any_element(),
            Panel::Mcp => self.render_mcp(cx).into_any_element(),
        };

        h_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_sidebar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(
                        h_flex()
                            .h(px(36.))
                            .flex_none()
                            .px_4()
                            .bg(theme.title_bar)
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(title),
                            ),
                    )
                    .child(div().flex_1().min_h_0().child(body)),
            )
    }
}
