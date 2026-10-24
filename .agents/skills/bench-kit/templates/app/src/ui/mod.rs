//! Rendering. Each file adds `render_*` methods to `MainApp`.

/// House-style building blocks (buttons, rows, group headers, tabs, bars, settings rows).
pub mod kit;
mod settings;
mod sidebar;
mod statusbar;

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::{MainApp, Panel};

impl MainApp {
    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.panel {
            Panel::Settings => "Settings".to_string(),
            Panel::Main => self
                .selected
                .and_then(|i| self.items.get(i))
                .map(|it| it.title.clone())
                .unwrap_or_default(),
        };
        // Icon buttons for the right side go in this h_flex (kit::icon_button).
        kit::header_bar(title, h_flex().gap_1().into_any_element(), cx)
    }

    fn render_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.panel == Panel::Settings {
            return self.render_settings(cx).into_any_element();
        }
        let theme = cx.theme().clone();
        match self.selected.and_then(|i| self.items.get(i)) {
            Some(item) => v_flex()
                .id("content")
                .size_full()
                .overflow_y_scroll()
                .px_4()
                .py_4()
                .text_sm()
                .child(TextView::markdown("content-md", item.body.clone()))
                .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Nothing selected")
                .into_any_element(),
        }
    }
}

impl Render for MainApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let main = v_flex()
            .size_full()
            .min_w_0()
            .child(self.render_header(cx))
            .child(div().flex_1().min_h_0().child(self.render_body(cx)));
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
                        .child(resizable_panel().child(main)),
                ),
            )
            .child(self.render_statusbar(cx))
    }
}
