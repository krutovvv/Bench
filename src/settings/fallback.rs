//! The gear's fallback model: the one that answers when the main model fails or goes silent, for
//! every thread of the agent's projects (`agent/src/runtime/fallback.ts`).

use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    /// The fallback model: one (provider, model), or none.
    pub fn set_fallback(
        &mut self,
        pick: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let list = match &pick {
            Some((p, m)) => json!([{"provider": p, "model": m}]),
            None => json!([]),
        };
        self.gear.providers.fallback = pick;
        cx.notify();
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::merge_config(conn, json!({"fallback_providers": list})),
            |this, r, window, cx| match r {
                Ok(()) => this.load_agent_settings(window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Fallback: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// A heading and its one row: a pull-down of what the agent offers, by provider.
    pub(crate) fn render_fallback(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let current = self.gear.providers.fallback.clone();
        let label = current
            .as_ref()
            .map(|(p, m)| {
                let group = self
                    .defaults.models
                    .iter()
                    .find(|c| c.provider == *p)
                    .map_or(p.clone(), |c| c.group.clone());
                format!("{m}  ·  {group}")
            })
            .unwrap_or_else(|| "None".into());
        let mut choices: Vec<(Option<String>, String, Option<(String, String)>)> =
            vec![(None, "None".into(), None)];
        let mut last_group = String::new();
        for c in self.defaults.models.iter().filter(|c| c.tag != "main") {
            let header = (c.group != last_group).then(|| c.group.clone());
            last_group = c.group.clone();
            choices.push((
                header,
                c.model.clone(),
                Some((c.provider.clone(), c.model.clone())),
            ));
        }
        let view = cx.entity();
        let picker = Button::new("fallback")
            .outline()
            .small()
            .label(label)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for (header, text, pick) in &choices {
                    if let Some(h) = header {
                        menu = menu.separator().label(h.clone());
                    }
                    let (view, pick2) = (view.clone(), pick.clone());
                    let checked = *pick == current;
                    menu = menu.item(PopupMenuItem::new(text.clone()).checked(checked).on_click(
                        move |_, window, cx| {
                            let pick = pick2.clone();
                            view.update(cx, |this, cx| this.set_fallback(pick, window, cx))
                        },
                    ));
                }
                menu
            });
        vec![
            bench_ui::settings_heading("Fallback", vec![], cx).into_any_element(),
            bench_ui::settings_field(
                "Fallback model",
                "Answers when the main model fails or goes silent",
                h_flex().child(picker).into_any_element(),
                cx,
            )
            .into_any_element(),
        ]
    }
}
