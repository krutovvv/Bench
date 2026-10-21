//! Bottom bar across the window: where / which profile on the left; context, reasoning effort,
//! model, gateway restart and settings on the right.

use gpui_kit::component::select::Select;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{compact, plural};
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;

impl HermesApp {
    /// Model: live usage, else the open session, else the profile, else the default profile.
    fn model_name(&self) -> String {
        let non_empty = |s: &String| !s.is_empty();
        self.usage
            .as_ref()
            .map(|u| u.model.clone())
            .filter(non_empty)
            .or_else(|| {
                let id = self.session.as_ref()?;
                self.sessions
                    .iter()
                    .find(|s| &s.id == id)
                    .map(|s| s.model.clone())
            })
            .filter(non_empty)
            .or_else(|| {
                let p = self.profile.as_ref()?;
                self.profiles
                    .iter()
                    .find(|x| &x.name == p)
                    .map(|x| x.model.clone())
            })
            .filter(non_empty)
            .or_else(|| {
                self.profiles
                    .iter()
                    .find(|x| x.name == "default")
                    .map(|x| x.model.clone())
            })
            .filter(non_empty)
            .unwrap_or_else(|| "—".into())
    }

    pub(super) fn render_statusbar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        let item = |id: &'static str| {
            h_flex()
                .id(id)
                .h_full()
                .px_2()
                .gap_1()
                .items_center()
                .text_color(muted)
        };
        let button = |id: &'static str, icon: Ico, tip: &'static str, active: bool| {
            item(id)
                .cursor_pointer()
                .when(active, |d| d.text_color(theme.primary))
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().xsmall())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        let usage = self.usage.clone().filter(|u| u.context_max > 0);
        let context = usage.as_ref().map(|u| {
            let color = if u.context_percent > 85. {
                theme.danger
            } else if u.context_percent > 60. {
                theme.warning
            } else {
                muted
            };
            let details: SharedString = [
                Some(format!("контекст {} из {}", u.context_used, u.context_max)),
                (u.total_tokens > 0).then(|| format!("всего токенов {}", u.total_tokens)),
                (u.compressions > 0)
                    .then(|| plural(u.compressions as usize, "сжатие", "сжатия", "сжатий")),
                u.cache_hit_pct.map(|c| format!("кэш {c}%")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n")
            .into();
            item("context")
                .text_color(color)
                .child(Ico::MemoryStick.icon().xsmall())
                .child(format!(
                    "{} / {} · {:.0}%",
                    compact(u.context_used),
                    compact(u.context_max),
                    u.context_percent
                ))
                .tooltip(move |window, cx| Tooltip::new(details.clone()).build(window, cx))
        });

        h_flex()
            .h(px(28.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .child(
                item("target").child(Ico::Server.icon().xsmall()).child(
                    div()
                        .w(px(150.))
                        .child(Select::new(&self.target_choice).appearance(false).xsmall()),
                ),
            )
            .child(
                item("profile").child(Ico::UserRound.icon().xsmall()).child(
                    div().w(px(150.)).child(
                        Select::new(&self.profile_choice)
                            .appearance(false)
                            .xsmall()
                            .placeholder("Профиль"),
                    ),
                ),
            )
            .when(self.connecting, |d| {
                d.child(item("connecting").child("подключаюсь…"))
            })
            .child(div().flex_1())
            .children(context)
            .child(
                item("effort")
                    .child(Ico::Brain.icon().xsmall())
                    .child(
                        div()
                            .w(px(84.))
                            .child(Select::new(&self.effort_choice).appearance(false).xsmall()),
                    )
                    .tooltip(|window, cx| Tooltip::new("Глубина рассуждений").build(window, cx)),
            )
            .child(
                item("model")
                    .child(Ico::Cpu.icon().xsmall())
                    .child(self.model_name()),
            )
            .child(
                button(
                    "restart",
                    Ico::RefreshCw,
                    "Перезапустить шлюз Hermes",
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| this.restart_gateway(window, cx))),
            )
            .child(
                button(
                    "settings",
                    Ico::Settings,
                    "Настройки",
                    self.panel.is_settings(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.panel.is_settings() {
                        this.panel = Panel::Chat;
                        cx.notify();
                    } else {
                        this.open_panel(Panel::Skills, window, cx);
                    }
                })),
            )
    }
}
