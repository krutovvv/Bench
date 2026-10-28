//! The agent asks (the `clarify` tool): answer with a choice, several, your own words, or decline.

use gpui_kit::*;
use protocol::{Ask, AskKind};

use super::ThreadView;

impl ThreadView {
    /// The question the run waits for now, if any.
    pub(crate) fn live_clarify(&self) -> Option<&Ask> {
        self.thread
            .journal
            .asks()
            .iter()
            .find(|a| a.kind == AskKind::Clarify)
    }

    /// `answer` empty = declined (the agent goes on without it).
    pub fn answer_clarify(&mut self, answer: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.live_clarify().map(|a| a.id.clone()) else {
            return;
        };
        self.thread.clarify_picked.clear();
        self.clarify_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.answer_ask(ask, answer, cx);
    }

    /// Multi-select: tick or untick a choice.
    pub fn toggle_clarify_choice(&mut self, k: usize, cx: &mut Context<Self>) {
        match self.thread.clarify_picked.iter().position(|x| *x == k) {
            Some(at) => {
                self.thread.clarify_picked.remove(at);
            }
            None => self.thread.clarify_picked.push(k),
        }
        cx.notify();
    }

    /// Multi-select: send the ticked choices, comma-separated, in their order.
    pub fn send_clarify_picks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.live_clarify() else {
            return;
        };
        let mut picked = self.thread.clarify_picked.clone();
        picked.sort();
        let answer = picked
            .iter()
            .filter_map(|k| ask.choices.get(*k).cloned())
            .collect::<Vec<_>>()
            .join(", ");
        if !answer.is_empty() {
            self.answer_clarify(answer, window, cx);
        }
    }
}
