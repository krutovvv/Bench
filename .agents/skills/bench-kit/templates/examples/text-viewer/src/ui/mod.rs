//! Rendering. Each file adds `render_*` methods to `MainApp`.

/// House-style building blocks (buttons, rows, group headers, tabs, bars, settings rows).
pub mod kit;
mod settings;
mod sidebar;
mod statusbar;

use gpui_kit::component::input::Editor;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::{MainApp, Panel};

impl MainApp {
    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match (self.panel, &self.file) {
            (Panel::Settings, _) => "Settings".to_string(),
            (Panel::Main, Some(f)) => f
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            (Panel::Main, None) => String::new(),
        };
        kit::header_bar(title, h_flex().gap_1().into_any_element(), cx)
    }

    fn render_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.panel == Panel::Settings {
            return self.render_settings(cx).into_any_element();
        }
        if self.file.is_none() {
            return kit::empty_state("Open a file to start", cx).into_any_element();
        }
        Editor::new(&self.editor)
            .appearance(false) // no border of its own: the panel is the frame
            .h_full()
            .into_any_element()
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
