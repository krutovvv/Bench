//! Settings → Prompts: the user's text snippets, kept per agent profile as Markdown files
//! (`<profile home>/prompts/<title>.md`, like skills) and inserted from the composer with `@`.

use gpui_kit::*;

use super::HermesApp;
use crate::rest;

impl HermesApp {
    /// Read the profile's prompts (on profile switch and when the tab opens).
    pub(super) fn load_prompts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(window, cx, rest::prompts, |this, r, _window, cx| {
            if let Ok(list) = r {
                this.prompts = list;
                cx.notify();
            }
        });
    }
}
