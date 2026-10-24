//! Browser sessions (`browser:<id>`): threads that work in the server's browser
//! (`crate::remote_browser`). Every tab the agent opens during a turn is that turn's step and
//! shows in its answer — live while it is the latest, its last frame after. Steps survive restarts: which tab belonged
//! to which turn and each step's last frame are kept in Bench's state folder.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::*;
use serde::{Deserialize, Serialize};

use super::HermesApp;
use crate::connection::state_dir;
use crate::remote_browser::{Browser, image_of};

/// No input and no agent at work this long: the stream pauses.
const IDLE: std::time::Duration = std::time::Duration::from_secs(60);

/// One step: the tab, the turn it belongs to (`usize::MAX`: the thread's top, before any turn).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub tab: String,
    pub turn: usize,
    pub url: String,
}

#[derive(Default)]
pub struct Sites {
    pub browser: Option<Arc<Browser>>,
    /// Session id (or "draft") -> its steps.
    pub steps: HashMap<String, Vec<Step>>,
    /// Saved frames of tabs that closed, read from disk once.
    pub saved: HashMap<String, Arc<RenderImage>>,
    pub focus: Option<FocusHandle>,
    /// Where each step's frame was laid out (to map the pointer to the page).
    pub bounds: std::rc::Rc<std::cell::RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// The last time someone looked: the person's input in a step, a step's tab arriving.
    pub touched: Option<web_time::Instant>,
    loaded: bool,
    /// When each step's frame last went to disk (a frame every few seconds is enough).
    saved_at: HashMap<String, web_time::Instant>,
}

fn steps_file() -> std::path::PathBuf {
    state_dir().join("site-steps.json")
}

/// A step's last frame, in its session's own folder: deleting the session deletes its pictures.
fn frame_file(session: &str, tab: &str) -> std::path::PathBuf {
    state_dir().join("sessions").join(session).join("frames").join(format!("{tab}.jpg"))
}

/// A deleted session's files (its steps' frames).
pub fn forget_session(id: &str) {
    let _ = std::fs::remove_dir_all(state_dir().join("sessions").join(id));
}

impl HermesApp {
    fn load_sites(&mut self) {
        if self.sites.loaded || self.demo {
            return;
        }
        self.sites.loaded = true;
        self.sites.steps = std::fs::read(steps_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
    }

    fn save_steps(&self) {
        let _ = std::fs::create_dir_all(state_dir());
        if let Ok(b) = serde_json::to_vec(&self.sites.steps) {
            let _ = std::fs::write(steps_file(), b);
        }
    }

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

    /// Frames flow only while someone watches: a browser thread is open, and the agent is working
    /// in it or the person touched a step in the last IDLE. Otherwise the stream pauses (the
    /// last frame stays; a click resumes it).
    pub fn update_browser_stream(&mut self) {
        let Some(b) = self.sites.browser.clone() else {
            return;
        };
        let open = self.panel == super::Panel::Chat && Self::subject_browser(&self.current_subject()).is_some();
        let recent = self.sites.touched.is_some_and(|t| t.elapsed() < IDLE);
        b.set_paused(!(open && (self.live.is_some() || recent)));
    }

    /// The person reached for a step: watch it live.
    pub fn touch_browser(&mut self) {
        self.sites.touched = Some(web_time::Instant::now());
        self.update_browser_stream();
    }

    /// The browser changed: new tabs go to the running turn of the open site thread; the
    /// latest step streams; closed steps keep their last frame on disk.
    fn on_browser(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.sites.browser.clone() else {
            return;
        };
        self.claim_reopened();
        let fresh: Vec<String> = std::mem::take(&mut b.state.lock().unwrap().fresh);
        let moved: Vec<String> = std::mem::take(&mut b.state.lock().unwrap().moved);
        let key = self.thread_key();
        let in_site = Self::subject_browser(&self.current_subject()).is_some();
        if in_site && !fresh.is_empty() {
            self.sites.touched = Some(web_time::Instant::now());
        }
        if in_site && self.live.is_some() && !fresh.is_empty() {
            let turn = self.turns.len().saturating_sub(1);
            let steps = self.sites.steps.entry(key.clone()).or_default();
            for tab in fresh {
                steps.push(Step { tab, turn, url: String::new() });
            }
            // Live tabs cost the server 200–450 MB each: the two latest steps stay open, the
            // older ones close (their last frame stays, a click reopens the page).
            let keep: Vec<String> = steps.iter().rev().take(2).map(|s| s.tab.clone()).collect();
            let old: Vec<String> = steps.iter().map(|s| s.tab.clone()).filter(|t| !keep.contains(t)).collect();
            self.save_steps();
            for t in old {
                if b.is_open(&t) {
                    b.close(&t);
                }
            }
        }
        // The agent worked in a tab it already had (no new one): with no step of its own yet,
        // the turn takes the tab it navigated.
        if in_site && self.live.is_some() {
            let turn = self.turns.len().saturating_sub(1);
            let steps = self.sites.steps.entry(key.clone()).or_default();
            if let Some(tab) = moved.last() {
                if !steps.iter().any(|s| s.turn == turn) {
                    steps.retain(|s| s.tab != *tab);
                    steps.push(Step { tab: tab.clone(), turn, url: String::new() });
                    self.sites.touched = Some(web_time::Instant::now());
                    self.save_steps();
                }
            }
        }
        // A sleeping step's tab reopened under a new id: the step follows it.
        let woke: Vec<(String, String)> = std::mem::take(&mut b.state.lock().unwrap().woke);
        let mut dirty = false;
        for (old, new) in woke {
            for steps in self.sites.steps.values_mut() {
                for s in steps.iter_mut().filter(|s| s.tab == old) {
                    s.tab = new.clone();
                    dirty = true;
                }
            }
        }
        // Keep each step's url, and its latest frame on disk (every few seconds): after a
        // restart of Bench or of the server's browser, the steps still show their pages.
        let state = b.state.lock().unwrap();
        for (session, steps) in self.sites.steps.iter_mut() {
            for s in steps.iter_mut() {
                if let Some(shot) = state.shots.get(&s.tab) {
                    if s.url != shot.url && !shot.url.is_empty() {
                        s.url = shot.url.clone();
                        dirty = true;
                    }
                    let due = self.sites.saved_at.get(&s.tab).is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(5));
                    if due && !shot.jpeg.is_empty() {
                        let file = frame_file(session, &s.tab);
                        let _ = std::fs::create_dir_all(file.parent().unwrap());
                        let _ = std::fs::write(file, &shot.jpeg);
                        self.sites.saved_at.insert(s.tab.clone(), web_time::Instant::now());
                        self.sites.saved.remove(&s.tab);
                    }
                }
            }
        }
        drop(state);
        if dirty {
            self.save_steps();
        }
        // Stream the open thread's latest step that still has a tab.
        if let Some(last) = self.sites.steps.get(&key).and_then(|v| v.iter().rev().find(|s| b.is_open(&s.tab))) {
            b.show(last.tab.clone());
        }
        self.update_browser_stream();
        cx.notify();
    }

    /// The sidebar's "+" under Browser: a new browser session (a thread whose turns are steps
    /// in the server's browser).
    pub fn new_browser_thread(&mut self, cx: &mut Context<Self>) {
        self.load_sites();
        self.new_session(cx);
        self.new_subject = format!("browser:{}", uuid::Uuid::new_v4().simple().to_string()[..8].to_string());
        self.ensure_browser(cx);
        cx.notify();
    }

    /// A browser session from the sidebar: its thread, the steps live or as their last frames.
    pub fn open_browser_thread(&mut self, id: String, cx: &mut Context<Self>) {
        self.load_sites();
        self.open_session(id, cx);
        self.ensure_browser(cx);
    }

    /// A step whose tab closed: a new tab at its page (signed in: the profile keeps sessions),
    /// in the step's place.
    pub fn reopen_step(&mut self, key: String, turn: usize, url: String, cx: &mut Context<Self>) {
        let Some(b) = self.ensure_browser(cx) else {
            return;
        };
        self.touch_browser();
        // Asleep on the server (memory): wake it; its new id comes back and the step follows.
        if let Some(st) = self.sites.steps.get(&key).and_then(|v| v.iter().find(|s| s.turn == turn)) {
            if b.is_asleep(&st.tab) {
                b.wake(&st.tab);
                return;
            }
        }
        if url.is_empty() {
            return;
        }
        let before: Vec<String> = b.state.lock().unwrap().pages.iter().map(|p| p.id.clone()).collect();
        b.open(url.clone());
        // The new tab arrives as "fresh"; claim it for this step when it does.
        self.sites_pending = Some((key, turn, url, before));
        cx.notify();
    }

    /// Steps of the open thread, by turn (`usize::MAX`: the top).
    pub fn steps_of(&self, turn: usize) -> Vec<Step> {
        self.sites
            .steps
            .get(&self.thread_key())
            .map(|v| v.iter().filter(|s| s.turn == turn).cloned().collect())
            .unwrap_or_default()
    }

    /// A step's picture: the live frame, else its saved one.
    pub fn step_frame(&mut self, tab: &str) -> Option<(Arc<RenderImage>, f32, f32, bool)> {
        let b = self.sites.browser.clone();
        if let Some(b) = &b {
            let s = b.state.lock().unwrap();
            if let Some(shot) = s.shots.get(tab) {
                let live = s.pages.iter().any(|p| p.id == tab) && s.active.as_deref() == Some(tab);
                if let Some(f) = &shot.frame {
                    return Some((f.clone(), shot.w, shot.h, live));
                }
            }
        }
        if let Some(img) = self.sites.saved.get(tab) {
            return Some((img.clone(), 0., 0., false));
        }
        let img = image_of(&std::fs::read(frame_file(&self.thread_key(), tab)).ok()?)?;
        self.sites.saved.insert(tab.to_string(), img.clone());
        Some((img, 0., 0., false))
    }

    /// The tab reopened for a step arrived: it takes the step's place.
    pub fn claim_reopened(&mut self) {
        let Some(b) = self.sites.browser.clone() else {
            return;
        };
        let Some((key, turn, url, before)) = self.sites_pending.clone() else {
            return;
        };
        let new = b.state.lock().unwrap().pages.iter().map(|p| p.id.clone()).find(|id| !before.contains(id));
        if let Some(tab) = new {
            self.sites_pending = None;
            let steps = self.sites.steps.entry(key).or_default();
            steps.retain(|s| s.turn != turn);
            steps.push(Step { tab, turn, url });
            self.save_steps();
        }
    }

    /// A browser session's subject (`browser:<id>`).
    pub fn subject_browser(subject: &str) -> Option<String> {
        subject.strip_prefix("browser:").map(str::to_string)
    }
}
