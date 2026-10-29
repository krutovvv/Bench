//! The agent's defaults for what Bench shows before a thread says its own: the reasoning level
//! a new chat starts on, the models the picker offers, the approval mode, the context window.

use gpui_kit::component::IndexPath;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    /// Read this Mac's agent's defaults; a new chat starts on its reasoning level.
    pub(crate) fn load_agent_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_thread
            .update(cx, |v, _| v.thread.effort_pending = false);
        self.defaults.ready = false;
        self.rest(window, cx, |c| rest::agent_defaults(c, ""), |this, r, window, cx| {
            this.defaults.ready = true;
            let Ok((effort, model, models, approval, context)) = r else {
                return;
            };
            this.defaults.model = model;
            this.defaults.approval_mode = Some(approval);
            this.defaults.context_length = context;
            this.defaults.models = models;
            this.add_picker_models();
            let level = effort.and_then(|v| this.defaults.efforts.iter().position(|e| *e == v));
            if let Some(i) = level {
                this.defaults.effort = i;
            }
            // The default is for new chats: an open thread already has its own.
            if let Some(i) = level.filter(|_| this.thread(cx).id.is_none()) {
                this.active_thread.update(cx, |v, _| {
                    v.thread.effort = i;
                    v.thread.effort_other = None;
                    v.thread.effort_pending = false;
                });
                this.effort_choice.update(cx, |s, cx| {
                    s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                });
            }
        });
    }
}

/// What this Mac's agent says before a thread says its own: where a new chat's reasoning starts,
/// the levels offered, the models the picker offers, the approval mode, the context window, the
/// folder threads start in, the slash commands.
pub struct AgentDefaults {
    /// They have arrived: until then nothing shows them (defaults would only mislead).
    pub ready: bool,
    /// The reasoning level a new chat starts on (index into `efforts`).
    pub effort: usize,
    /// The reasoning levels offered (the presets'; `EFFORTS` until they are read).
    pub efforts: Vec<String>,
    /// The main model; None: none is set, and a new thread says nothing can answer yet
    /// (`thread/rows/welcome.rs`).
    pub model: Option<String>,
    /// The primary model and its fallbacks, as (model, provider).
    pub models: Vec<crate::client::rest::ModelChoice>,
    /// `approvals.mode`, once read.
    pub approval_mode: Option<String>,
    /// `model.context_length` (0 when not set).
    pub context_length: u64,
    /// The default working folder (a thread started there has no folder of its own).
    pub cwd: Option<String>,
    /// The slash commands threads offer.
    pub commands: Vec<crate::thread::composer::slash::SlashCommand>,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        use crate::app::model::EFFORTS;
        Self {
            ready: false,
            effort: EFFORTS.len() - 1,
            efforts: EFFORTS.iter().map(|e| e.to_string()).collect(),
            model: None,
            models: vec![],
            approval_mode: None,
            context_length: 0,
            cwd: None,
            commands: vec![],
        }
    }
}
