//! What the gear's server cards share: a provider and an evaluator server are a name, an address,
//! a model and a key, with the last check's word and the same small actions. The card itself is
//! `bench_ui::settings_card`.

use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;

/// The fields every server of the gear has.
pub struct ServerForm {
    /// "" for one not saved yet.
    pub id: String,
    pub name: Entity<InputState>,
    pub url: Entity<InputState>,
    pub model: Entity<InputState>,
    pub key: Entity<InputState>,
    /// The key as shown when it was read, to send only a change.
    pub key_value: String,
    /// The last check: (ok, what it said).
    pub check: Option<(bool, String)>,
}

impl ServerForm {
    pub fn saved(&self) -> bool {
        !self.id.is_empty()
    }

    /// What the agent's REST takes of it; the key apart (`changed_key`), as each side treats an
    /// empty one its own way.
    pub fn body(&self, cx: &App) -> Value {
        json!({
            "id": self.id,
            "name": text(&self.name, cx),
            "base_url": text(&self.url, cx),
            "model": text(&self.model, cx),
        })
    }

    /// The key when it differs from what was read.
    pub fn changed_key(&self, cx: &App) -> Option<String> {
        let key = text(&self.key, cx);
        (key != self.key_value).then_some(key)
    }

    /// The word the agent's answer to a check came back with.
    pub fn check_message(v: &Value) -> String {
        v["message"].as_str().unwrap_or("Doesn't answer").to_string()
    }

    /// The last check as the card shows it.
    pub fn check_line(&self) -> Option<(bool, SharedString)> {
        self.check.clone().map(|(ok, msg)| (ok, msg.into()))
    }
}

pub(super) fn field(
    window: &mut Window,
    cx: &mut Context<BenchApp>,
    text: &str,
    hint: &str,
) -> Entity<InputState> {
    let text = text.to_string();
    let hint = hint.to_string();
    cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder(hint);
        s.set_value(text, window, cx);
        s
    })
}

pub(super) fn text(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_string()
}

/// A row of a card: its title and hint, the field.
pub(super) fn input_row(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    input: &Entity<InputState>,
    cx: &App,
) -> AnyElement {
    bench_ui::settings_field(title, hint, Input::new(input).into_any_element(), cx).into_any_element()
}

/// A card's action: the small bordered button; the main one filled (`primary`).
pub(super) fn card_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    on: impl Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>) + 'static,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    (if primary { bench_ui::primary_button } else { bench_ui::secondary_button })(id, label)
        .small()
        .on_click(cx.listener(move |this, _, w, cx| on(this, w, cx)))
        .into_any_element()
}
