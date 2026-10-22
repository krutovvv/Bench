//! Bottom bar across the window: where / which profile on the left; context, reasoning effort,
//! model, gateway restart and settings on the right.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{compact, plural};
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;
use crate::model::EFFORTS;

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
        let view = cx.entity();

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

        // A label as wide as its text that opens a menu of choices.
        let picker = |id: &'static str,
                      icon: Ico,
                      label: String,
                      items: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)>,
        )>| {
            let items = std::rc::Rc::new(items);
            let view = view.clone();
            h_flex()
                .h_full()
                .items_center()
                .pl_2()
                .child(icon.icon().xsmall().text_color(muted))
                .child(Button::new(id).ghost().xsmall().label(label).dropdown_menu(
                    move |menu, _, _| {
                        let mut menu = menu;
                        for (k, (text, checked, _)) in items.iter().enumerate() {
                            let (items, view) = (items.clone(), view.clone());
                            menu = menu.item(
                                PopupMenuItem::new(text.clone()).checked(*checked).on_click(
                                    move |_, window, cx| {
                                        view.update(cx, |this, cx| (items[k].2)(this, window, cx))
                                    },
                                ),
                            );
                        }
                        menu
                    },
                ))
        };

        let targets = self
            .targets
            .iter()
            .map(|t| {
                let t2 = t.clone();
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, window, cx| {
                        if this.target != t2 || this.conn.is_none() {
                            this.connect(t2.clone(), window, cx)
                        }
                    });
                (t.label(), *t == self.target, f)
            })
            .collect();
        let profiles = self
            .profiles
            .iter()
            .map(|p| {
                let name = p.name.clone();
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, window, cx| this.select_profile(name.clone(), window, cx));
                (p.label.clone(), self.profile.as_deref() == Some(&p.name), f)
            })
            .collect();
        let efforts = EFFORTS
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, _, cx| this.set_effort(i, cx));
                (e.to_string(), i == self.effort, f)
            })
            .collect();
        let current_model = self.model_name();
        let models: Vec<_> = self
            .profile_models
            .iter()
            .enumerate()
            .map(|(i, (m, p))| {
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, _, cx| this.set_model(i, cx));
                let checked = self.model_pick.map_or(*m == current_model, |k| k == i);
                let tag = if i == 0 { "primary" } else { "fallback" };
                let label = if p.is_empty() {
                    format!("{m}  ·  {tag}")
                } else {
                    format!("{m}  ·  {p}, {tag}")
                };
                (label, checked, f)
            })
            .collect();
        let profile_label = self
            .profiles
            .iter()
            .find(|p| Some(&p.name) == self.profile.as_ref())
            .map(|p| p.label.clone())
            .unwrap_or_else(|| "Profile".into());

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
                Some(format!("context {} of {}", u.context_used, u.context_max)),
                (u.total_tokens > 0).then(|| format!("total tokens {}", u.total_tokens)),
                (u.compressions > 0).then(|| {
                    plural(
                        u.compressions as usize,
                        "compression",
                        "compressions",
                        "compressions",
                    )
                }),
                u.cache_hit_pct.map(|c| format!("cache {c}%")),
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
            .child(picker("target", Ico::Server, self.target.label(), targets))
            .when(!self.profiles.is_empty(), |d| {
                d.child(picker("profile", Ico::UserRound, profile_label, profiles))
            })
            .child(div().flex_1())
            .children(context)
            .child(picker(
                "effort",
                Ico::Brain,
                EFFORTS[self.effort].to_string(),
                efforts,
            ))
            .map(|d| {
                if models.is_empty() || self.session.is_some() {
                    d.child(
                        item("model")
                            .child(Ico::Cpu.icon().xsmall())
                            .child(current_model),
                    )
                } else {
                    d.child(picker("model", Ico::Cpu, current_model, models))
                }
            })
            .child(
                button(
                    "restart",
                    Ico::RefreshCw,
                    "Restart the Hermes gateway",
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| this.restart_gateway(window, cx))),
            )
            .child(
                button(
                    "settings",
                    Ico::Settings,
                    "Settings",
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
