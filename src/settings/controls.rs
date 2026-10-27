//! The gear's own controls: a heading, a row (title and hint left, control right), a field, a
//! button, the page they scroll on.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

/// What a gear button does when clicked.
pub(super) type Act = Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>;

impl BenchApp {

    pub(super) fn providers_page(page: Vec<AnyElement>) -> AnyElement {
        v_flex()
            .id("providers")
            .size_full()
            .overflow_y_scroll()
            // the thread's inset all around: as far from the edge as its avatar
            .pl(px(bench_ui::CHAT_LEFT))
            .pr(px(bench_ui::CHAT_LEFT))
            .pb(px(bench_ui::CHAT_LEFT))
            .child(v_flex().flex_1().children(page))
            .into_any_element()
    }
}

impl BenchApp {
    pub(crate) fn pv_heading(&self, text: impl Into<SharedString>, cx: &App) -> Div {
        div()
            .pt_6()
            .pb_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
    }

    /// Title and hint on the left, the control on the right, a rule below.
    pub(crate) fn pv_row(
        &self,
        title: impl Into<SharedString>,
        hint: impl Into<SharedString>,
        control: AnyElement,
        cx: &App,
    ) -> Div {
        let theme = cx.theme();
        h_flex()
            .gap_4()
            .py_2p5()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .w(px(220.))
                    .flex_none()
                    .gap_0p5()
                    .child(div().text_base().child(title.into()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(hint.into()),
                    ),
            )
            .child(div().flex_1().min_w_0().child(control))
    }

    pub(crate) fn pv_input(&self, input: &Entity<InputState>) -> AnyElement {
        Input::new(input).into_any_element()
    }

    /// A bordered text button; `primary` fills it with the accent.
    pub(crate) fn pv_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        primary: bool,
        cx: &mut Context<Self>,
        on: Act,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        div()
            .id(id)
            .flex_none()
            .h(rems(bench_ui::BAR_CONTROL_REMS))
            .px_3()
            .flex()
            .items_center()
            .text_base()
            .rounded_md()
            .cursor_pointer()
            .when(primary, |d| {
                d.bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .hover(|s| s.bg(theme.primary_hover))
            })
            .when(!primary, |d| {
                d.border_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(theme.list_hover))
            })
            .child(label.into())
            .on_click(cx.listener(move |this, _, window, cx| on(this, window, cx)))
            .into_any_element()
    }
}
