//! The gear's voice, for an agent: OpenAI-compatible servers for speech to text and text to
//! speech, and Bench's own choice of showing the text while speaking.

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::*;
use bench_ui::Gap;
use serde_json::{Value, json};

use super::server_form::{field, input_row, text};
use crate::app::BenchApp;
use crate::client::rest;

pub struct Voice {
    pub stt_url: Entity<InputState>,
    pub stt_model: Entity<InputState>,
    pub stt_lang: Entity<InputState>,
    pub stt_key: Entity<InputState>,
    pub tts_url: Entity<InputState>,
    pub tts_model: Entity<InputState>,
    pub tts_voice: Entity<InputState>,
}

impl Voice {
    /// Its fields from the agent's config (`stt.openai`, `tts.openai`).
    pub(super) fn from_config(yaml: &Value, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let y = |path: &[&str]| {
            let mut v = yaml;
            for p in path {
                v = &v[*p];
            }
            v.as_str().unwrap_or_default().to_string()
        };
        Voice {
            stt_url: field(window, cx, &y(&["stt", "openai", "base_url"]), "http://host:port/v1"),
            stt_model: field(window, cx, &y(&["stt", "openai", "model"]), "e.g. whisper-1"),
            stt_lang: field(window, cx, &y(&["stt", "openai", "language"]), "e.g. en, ru; empty = auto"),
            stt_key: field(window, cx, &y(&["stt", "openai", "api_key"]), "If the server asks for one"),
            tts_url: field(window, cx, &y(&["tts", "openai", "base_url"]), "http://host:port/v1"),
            tts_model: field(window, cx, &y(&["tts", "openai", "model"]), "e.g. gpt-4o-mini-tts"),
            tts_voice: field(window, cx, &y(&["tts", "openai", "voice"]), "e.g. alloy"),
        }
    }
}

impl BenchApp {
    pub fn save_voice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = &self.gear.providers.voice else {
            return;
        };
        let mut stt = json!({
            "base_url": text(&v.stt_url, cx),
            "model": text(&v.stt_model, cx),
            "language": text(&v.stt_lang, cx),
        });
        stt["api_key"] = json!(text(&v.stt_key, cx));
        let tts = json!({
            "base_url": text(&v.tts_url, cx),
            "model": text(&v.tts_model, cx),
            "voice": text(&v.tts_voice, cx),
        });
        let config = json!({
            "stt": {"provider": "openai", "openai": stt},
            "tts": {"provider": "openai", "openai": tts},
        });
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::merge_config(conn, config),
            |this, r, window, cx| match r {
                Ok(()) => this.toast(NotificationType::Success, "Voice saved", window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("Voice: {e:#}"), window, cx),
            },
        );
    }

    /// A heading, a row per field, the live-dictation switch, Save.
    pub(crate) fn render_voice(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(v) = &self.gear.providers.voice else {
            return vec![];
        };
        vec![
            bench_ui::settings_heading("Voice · OpenAI-compatible servers", vec![], cx).into_any_element(),
            input_row("Speech to text", "Address of the server", &v.stt_url, cx),
            input_row("Model", "", &v.stt_model, cx),
            input_row("Language", "e.g. en, ru; empty = auto", &v.stt_lang, cx),
            input_row("API key", "If the server asks for one", &v.stt_key, cx),
            input_row("Text to speech", "Address of the server", &v.tts_url, cx),
            input_row("Model", "", &v.tts_model, cx),
            input_row("Voice", "", &v.tts_voice, cx),
            // Bench's own choice, not the agent's: saved at once, as a switch is.
            bench_ui::settings_field(
                "Text while speaking",
                "Each pause in dictation sends what was said, so the words come in as you speak (local Bench)",
                div()
                    .flex()
                    .justify_end()
                    .child(
                        gpui_kit::component::switch::Switch::new("voice-live")
                            .checked(self.media.live_pref)
                            .on_click(cx.listener(|this, on: &bool, _, cx| {
                                this.media.live_pref = *on;
                                crate::media::state::DictationPrefs { live: *on }.save();
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
            h_flex()
                .py(Gap::M)
                .child(div().flex_1())
                .child(
                    bench_ui::primary_button("voice-save", "Save")
                        .small()
                        .on_click(cx.listener(|this, _, w, cx| this.save_voice(w, cx)))
                        .into_any_element(),
                )
                .into_any_element(),
        ]
    }
}
