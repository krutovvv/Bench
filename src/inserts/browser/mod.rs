//! `/browser`: the thread's own tab in the server's browser, live, instead of a screenshot: the
//! person watches the agent work, clicks and types in it, then says "continue". Every thread has
//! its tab (one site here, another in another thread); its state: {tab, url}. The tab the agent
//! works in while it answers here becomes the thread's (`app::sites`); a tab that went away is
//! opened again at its address, or the agent opens a new one.

pub mod gateway;
pub mod state;
pub mod view;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::InsertKind;
use crate::model::Insert;

pub struct Browser;

/// The thread's tab, when open: (id, url, title).
fn tab_of(app: &BenchApp, ins: &Insert) -> Option<(String, String, String)> {
    let tab = ins.get("tab");
    if app.demo {
        return (!tab.is_empty()).then(|| (tab, ins.get("url"), "New post".into()));
    }
    let b = app.sites.browser.as_ref()?;
    let st = b.state.lock().unwrap();
    let page = st.pages.iter().find(|p| p.id == tab)?;
    Some((tab, page.url.clone(), page.title.clone()))
}

impl InsertKind for Browser {
    fn owns(&self, key: &str) -> bool {
        key == "browser"
    }

    fn title(&self, app: &BenchApp, ins: &Insert) -> String {
        match tab_of(app, ins) {
            Some((_, _, t)) if !t.trim().is_empty() => format!("Browser · {t}"),
            _ => "Browser".into(),
        }
    }

    fn controls(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
        let url = tab_of(app, ins)
            .map(|t| t.1)
            .unwrap_or_else(|| ins.get("url"));
        if url.is_empty() {
            return vec![];
        }
        vec![
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(url)
                .into_any_element(),
        ]
    }

    fn body(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let muted = cx.theme().muted_foreground;
        if let Some(e) = &app.sites_error {
            return Some(
                div()
                    .text_base()
                    .text_color(cx.theme().danger)
                    .child(e.clone())
                    .into_any_element(),
            );
        }
        if !app.demo && app.sites.browser.is_none() {
            app.ensure_browser(cx);
        }
        if let Some((tab, _, _)) = tab_of(app, ins) {
            return Some(app.render_tab(tab, cx));
        }
        let ready = app
            .sites
            .browser
            .as_ref()
            .is_some_and(|b| b.state.lock().unwrap().ready);
        if !ready && !app.demo {
            return Some(
                ShimmerText::new("Connecting to the browser…")
                    .text_base()
                    .text_color(muted)
                    .into_any_element(),
            );
        }
        let url = ins.get("url");
        // The gateway refused (all tabs busy, a page it does not open…): say so, and offer again.
        let refused = app
            .sites
            .browser
            .as_ref()
            .and_then(|b| b.state.lock().unwrap().error.clone());
        if let Some(e) = refused
            .filter(|_| app.sites.reopening.as_deref() == Some(ins.id.as_str()) || url.is_empty())
        {
            app.sites.reopening = None;
            return Some(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_base()
                            .text_color(cx.theme().danger)
                            .child(e),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{}-retry", ins.id)))
                            .small()
                            .label("Try again")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.click_claimed = true;
                                this.reopen_thread_tab(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        if app.sites.reopening.as_deref() == Some(ins.id.as_str()) {
            return Some(
                ShimmerText::new("Opening the page again…")
                    .text_base()
                    .text_color(muted)
                    .into_any_element(),
            );
        }
        if url.is_empty() {
            // Put in: its tab opens at once.
            if !app.demo {
                app.reopen_thread_tab(cx);
            }
            return Some(
                ShimmerText::new("Opening this thread's tab…")
                    .text_base()
                    .text_color(muted)
                    .into_any_element(),
            );
        }
        // Its tab went away (the browser restarted, the tab was closed).
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_base()
                        .text_color(muted)
                        .child(format!("The tab of {url} is closed.")),
                )
                .child(
                    Button::new(SharedString::from(format!("{}-reopen", ins.id)))
                        .small()
                        .primary()
                        .label("Open again")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.click_claimed = true;
                            this.reopen_thread_tab(cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn context(&self, app: &BenchApp, ins: &Insert, _cx: &App) -> String {
        match tab_of(app, ins) {
            Some((_, url, title)) => format!(
                "This thread's tab in the shared browser is at {url} (title: {title}). Work only in it: select it with mcp__browser__browser_tabs (list, then select the one at this address)."
            ),
            None if !ins.get("url").is_empty() => format!(
                "This thread's tab (it was at {}) is closed. Open a new tab with mcp__browser__browser_tabs {{action: \"new\"}} and go on there: it becomes this thread's tab.",
                ins.get("url")
            ),
            None => "This thread has no tab in the shared browser yet: open one with mcp__browser__browser_tabs {action: \"new\"}; it becomes this thread's tab. Never use the other tabs: they are other threads'.".into(),
        }
    }

    fn opened(
        &self,
        app: &mut BenchApp,
        _ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.ensure_browser(cx);
        app.touch_browser();
    }
}
