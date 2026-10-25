//! The server's browser (`crate::remote_browser`): one Chrome shared by every thread, a tab per
//! thread. A thread's `/browser` insert keeps its tab ({tab, url}): the tab the agent works in
//! while it answers in that thread becomes the thread's, the insert streams it live, and a tab
//! that went away is opened again at its address (or the agent opens a new one).

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::*;
use serde_json::json;

use super::BenchApp;
use crate::remote_browser::Browser;

/// Where a new thread's tab starts.
const START_PAGE: &str = "https://www.google.com/";

/// No input and no agent at work this long: the stream pauses.
const IDLE: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Default)]
pub struct Sites {
    pub browser: Option<Arc<Browser>>,
    pub focus: Option<FocusHandle>,
    /// Where each tab's frame was laid out (to map the pointer to the page).
    pub bounds: std::rc::Rc<std::cell::RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// The last time someone looked: the person's input in the tab, the agent moving it.
    pub touched: Option<web_time::Instant>,
    /// An insert whose tab is being opened again: the next new tab is its.
    pub reopening: Option<String>,
    /// The demo's tab: a picture.
    pub demo_frame: Option<Arc<RenderImage>>,
}

impl BenchApp {
    /// The server's browser, connected on first use.
    pub fn ensure_browser(&mut self, cx: &mut Context<Self>) -> Option<Arc<Browser>> {
        if let Some(b) = &self.sites.browser {
            return Some(b.clone());
        }
        // The demo's steps are pictures; nothing connects.
        if self.demo {
            return None;
        }
        let (tx, rx) = async_channel::bounded::<()>(1);
        match Browser::start(tx) {
            Ok(b) => {
                cx.spawn(async move |this, cx| {
                    while rx.recv().await.is_ok() {
                        if this.update(cx, |this, cx| this.on_browser(cx)).is_err() {
                            break;
                        }
                    }
                })
                .detach();
                self.sites.browser = Some(b.clone());
                Some(b)
            }
            Err(e) => {
                self.sites_error = Some(format!("{e:#}"));
                None
            }
        }
    }

    /// Frames flow only while someone watches: the open thread shows its browser, and the agent
    /// is working or the person touched the tab in the last IDLE. Otherwise the stream pauses
    /// (the last frame stays; a click resumes it).
    pub fn update_browser_stream(&mut self) {
        let Some(b) = self.sites.browser.clone() else {
            return;
        };
        let open = self.panel == super::Panel::Chat && self.thread_has_browser();
        let recent = self.sites.touched.is_some_and(|t| t.elapsed() < IDLE);
        b.set_paused(!(open && (self.live.is_some() || recent)));
        if let Some(tab) = self.thread_tab() {
            // Asleep on the server (memory): woken, its new id comes back in `woke`.
            if open && b.is_asleep(&tab) {
                b.wake(&tab);
            } else {
                b.show(tab);
            }
        }
    }

    /// The person reached for a step: watch it live.
    pub fn touch_browser(&mut self) {
        self.sites.touched = Some(web_time::Instant::now());
        self.update_browser_stream();
    }

    /// The open thread's browser insert: (its id, its tab, its url).
    pub fn thread_browser(&self) -> Option<(String, String, String)> {
        let all: Vec<_> = self
            .thread_inserts()
            .into_iter()
            .filter(|i| i.key == "browser")
            .collect();
        // Several in a thread: the live one (see `InsertKind::shared`).
        let live = self.insert_live.get("browser");
        let ins = all.iter().find(|i| Some(&i.id) == live).or(all.last())?;
        Some((ins.id.clone(), ins.get("tab"), ins.get("url")))
    }

    /// The open thread's tab, when it is open in the browser.
    pub fn thread_tab(&self) -> Option<String> {
        let (_, tab, _) = self.thread_browser()?;
        let b = self.sites.browser.as_ref()?;
        (!tab.is_empty() && b.is_open(&tab)).then_some(tab)
    }

    /// The thread's tab: a new one (at its last address, when it had one), which becomes the
    /// thread's.
    pub fn reopen_thread_tab(&mut self, cx: &mut Context<Self>) {
        let (Some((id, _, url)), Some(b)) = (self.thread_browser(), self.ensure_browser(cx)) else {
            return;
        };
        if self.sites.reopening.is_some() {
            return;
        }
        self.sites.reopening = Some(id);
        self.touch_browser();
        // A new thread's tab starts on a search page (the gateway opens only http(s) pages); the
        // agent, or the person, takes it from there.
        b.state.lock().unwrap().error = None;
        b.open(if url.is_empty() {
            START_PAGE.into()
        } else {
            url
        });
    }

    /// The browser changed: a tab the agent opened or moved while it answers in the open thread
    /// is that thread's tab from now on; a tab woken under a new id keeps its thread.
    fn on_browser(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.sites.browser.clone() else {
            return;
        };
        let (fresh, moved, woke) = {
            let mut st = b.state.lock().unwrap();
            (
                std::mem::take(&mut st.fresh),
                std::mem::take(&mut st.moved),
                std::mem::take(&mut st.woke),
            )
        };
        if let Some((id, tab, url)) = self.thread_browser() {
            let mut claim = None;
            if let Some(new) = fresh
                .last()
                .filter(|_| self.sites.reopening.as_deref() == Some(id.as_str()))
            {
                self.sites.reopening = None;
                claim = Some(new.clone());
            } else if self.live.is_some() {
                claim = moved.last().or(fresh.last()).cloned();
            }
            if let Some((_, new)) = woke.iter().find(|(old, _)| *old == tab) {
                claim = Some(new.clone());
            }
            if let Some(new) = claim.filter(|t| *t != tab) {
                self.sites.touched = Some(web_time::Instant::now());
                self.patch_insert(&id, json!({ "state": { "tab": new, "url": url } }), cx);
            }
            // The tab's address as it goes, for opening it again.
            if let Some(tab) = self.thread_tab() {
                let now = b
                    .state
                    .lock()
                    .unwrap()
                    .pages
                    .iter()
                    .find(|p| p.id == tab)
                    .map(|p| p.url.clone())
                    .unwrap_or_default();
                if !now.is_empty() && now != url {
                    self.patch_insert(&id, json!({ "state": { "tab": tab, "url": now } }), cx);
                }
            }
        }
        self.update_browser_stream();
        cx.notify();
    }

    /// The thread's tab's picture: the live frame (live: it streams now).
    pub fn tab_frame(&self, tab: &str) -> Option<(Arc<RenderImage>, f32, f32, bool)> {
        if self.demo {
            return self.sites.demo_frame.clone().map(|f| (f, 0., 0., false));
        }
        let b = self.sites.browser.as_ref()?;
        let s = b.state.lock().unwrap();
        let shot = s.shots.get(tab)?;
        let live = s.pages.iter().any(|p| p.id == tab) && s.active.as_deref() == Some(tab);
        shot.frame.clone().map(|f| (f, shot.w, shot.h, live))
    }
}
