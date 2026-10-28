//! The server browser's tabs, live in threads, through its `/live` gateway (`live.mjs` of
//! browser-server-mcp, a project of its own, or `live.py` beside Camoufox on dev): the one of the
//! browser the thread's agent drives — its `browser` MCP server's `live` address — a WebSocket
//! with the person's own key for it (`~/.bench/ui/browser.json`: `keys` by address; its `url` and
//! `token` when the MCP server names no live view), a narrow set of operations —
//! the tabs, watch one, wake a sleeping one, open, close, mouse, wheel, text, keys — and no raw
//! DevTools protocol.
//! The gateway sends at most two frames a second, only while someone watches, and keeps the
//! server's memory in budget (idle tabs fall asleep: a picture and their address; `wake`
//! reopens them). Only the site shows (no browser UI): every tab the agent opens during a turn is
//! that turn's step; the latest streams live, the others keep their last frame.

use std::collections::{HashMap, HashSet};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use gpui_kit::RenderImage;
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub id: String,
    pub url: String,
    pub title: String,
    /// Closed by the server to save memory; its last picture stays, `wake` reopens it.
    pub asleep: bool,
}

/// One tab as the view keeps it: its last frame (kept after the tab closes) and the page's
/// CSS size that frame shows (to map clicks).
#[derive(Clone, Default)]
pub struct Shot {
    pub frame: Option<Arc<RenderImage>>,
    #[allow(dead_code)]
    pub jpeg: Vec<u8>,
    pub w: f32,
    pub h: f32,
    pub url: String,
}

/// What the view reads.
#[derive(Default)]
pub struct State {
    pub pages: Vec<Page>,
    /// The tab being streamed (one at a time: the latest step's).
    pub active: Option<String>,
    pub shots: HashMap<String, Shot>,
    /// Tabs that appeared since the app last took them (to hand to the running turn).
    pub fresh: Vec<String>,
    /// Tabs whose page changed since the app last took them (an old tab the agent navigated).
    pub moved: Vec<String>,
    /// A sleeping tab reopened: (old id, new id), for the steps to follow.
    pub woke: Vec<(String, String)>,
    pub error: Option<String>,
    pub ready: bool,
}

/// Where the gateway is and this person's key: `live` (the thread's browser MCP server's live
/// view) with its key from browser.json's `keys` by address; without one, browser.json's own
/// `url` and `token`.
fn access(live: Option<&str>) -> Result<(String, String)> {
    let file = crate::client::connection::state_dir().join("browser.json");
    let v: Value = serde_json::from_slice(&std::fs::read(&file).map_err(|_| {
        anyhow!(
            "No browser access yet: ask for your key (live.mjs add <name>) and put it in {}",
            file.display()
        )
    })?)?;
    if let Some(live) = live {
        let token = v["keys"][live]
            .as_str()
            .or_else(|| (v["url"].as_str() == Some(live)).then(|| v["token"].as_str()).flatten())
            .ok_or_else(|| {
                anyhow!(
                    "No key for the browser at {live}: put it in {} under \"keys\" by this address",
                    file.display()
                )
            })?;
        return Ok((live.to_string(), token.to_string()));
    }
    let url = v["url"].as_str().unwrap_or_default().to_string();
    if url.is_empty() {
        return Err(anyhow!(
            "{} has no url: the gateway's address, wss://<host>/live",
            file.display()
        ));
    }
    Ok((url, v["token"].as_str().unwrap_or_default().to_string()))
}

pub struct Browser {
    /// The gateway it talks to.
    pub url: String,
    pub state: Arc<Mutex<State>>,
    out: mpsc::Sender<Value>,
    paused: std::sync::atomic::AtomicBool,
}

impl Browser {
    /// Connects to the gateway; `wake` gets a message for every change the view should show.
    pub fn start(live: Option<&str>, wake: async_channel::Sender<()>) -> Result<Arc<Self>> {
        if cfg!(target_family = "wasm") {
            return Err(anyhow!("The server's browser is live in the desktop app"));
        }
        let (url, token) = access(live)?;
        let (out, rx) = mpsc::channel::<Value>();
        let b = Arc::new(Self {
            url: url.clone(),
            state: Arc::new(Mutex::new(State::default())),
            out,
            paused: std::sync::atomic::AtomicBool::new(false),
        });
        let me = b.clone();
        std::thread::spawn(move || {
            let full = format!("{url}?token={token}");
            match tungstenite::connect(full.as_str()) {
                Ok((ws, _)) => me.run(ws, rx, wake),
                Err(e) => {
                    me.state.lock().unwrap().error =
                        Some(format!("The browser's gateway refused: {e}"));
                    let _ = wake.try_send(());
                }
            }
        });
        Ok(b)
    }

    fn run(
        self: Arc<Self>,
        mut ws: WebSocket<MaybeTlsStream<TcpStream>>,
        rx: mpsc::Receiver<Value>,
        wake: async_channel::Sender<()>,
    ) {
        let timeout = Some(Duration::from_millis(40));
        match ws.get_mut() {
            MaybeTlsStream::Plain(s) => {
                let _ = s.set_read_timeout(timeout);
            }
            #[cfg(not(target_family = "wasm"))]
            MaybeTlsStream::Rustls(s) => {
                let _ = s.get_mut().set_read_timeout(timeout);
            }
            _ => {}
        }
        self.state.lock().unwrap().ready = true;
        let _ = wake.try_send(());
        let mut known: HashSet<String> = HashSet::new();
        let mut first = true;
        loop {
            while let Ok(msg) = rx.try_recv() {
                let _ = ws.send(Message::text(msg.to_string()));
            }
            let text = match ws.read() {
                Ok(Message::Text(t)) => t.to_string(),
                Ok(Message::Close(_)) => break,
                Ok(_) => continue,
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(_) => break,
            };
            let Ok(m) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            let mut s = self.state.lock().unwrap();
            if let Some(list) = m["tabs"].as_array() {
                let before: HashMap<String, String> = s
                    .pages
                    .iter()
                    .map(|p| (p.id.clone(), p.url.clone()))
                    .collect();
                s.pages = list
                    .iter()
                    .map(|t| Page {
                        id: t["id"].as_str().unwrap_or_default().into(),
                        url: t["url"].as_str().unwrap_or_default().into(),
                        title: t["title"].as_str().unwrap_or_default().into(),
                        asleep: t["asleep"].as_bool().unwrap_or(false),
                    })
                    .collect();
                // The first list is what was already open; later new tabs are the running turn's.
                let ids: Vec<(String, bool)> =
                    s.pages.iter().map(|p| (p.id.clone(), p.asleep)).collect();
                for (id, asleep) in ids {
                    if known.insert(id.clone()) && !first && !asleep {
                        s.fresh.push(id);
                    }
                }
                if !first {
                    let moved: Vec<String> = s
                        .pages
                        .iter()
                        .filter(|p| !p.asleep && before.get(&p.id).is_some_and(|u| *u != p.url))
                        .map(|p| p.id.clone())
                        .collect();
                    for id in moved {
                        s.moved.retain(|m| *m != id);
                        s.moved.push(id);
                    }
                }
                first = false;
                let urls: Vec<(String, String)> = s
                    .pages
                    .iter()
                    .map(|p| (p.id.clone(), p.url.clone()))
                    .collect();
                for (id, url) in urls {
                    if let Some(shot) = s.shots.get_mut(&id) {
                        if !url.is_empty() && url != "about:blank" {
                            shot.url = url;
                        }
                    }
                }
            } else if let Some(data) = m["frame"].as_str() {
                let tab = m["tab"].as_str().unwrap_or_default().to_string();
                if let Some((frame, jpeg)) = decode(data) {
                    let url = s
                        .pages
                        .iter()
                        .find(|p| p.id == tab)
                        .map(|p| p.url.clone())
                        .unwrap_or_default();
                    let old = s.shots.get(&tab).cloned().unwrap_or_default();
                    let (w, h) = (
                        m["w"].as_f64().unwrap_or(0.) as f32,
                        m["h"].as_f64().unwrap_or(0.) as f32,
                    );
                    s.shots.insert(
                        tab,
                        Shot {
                            frame: Some(frame),
                            jpeg,
                            w: if w > 0. { w } else { old.w },
                            h: if h > 0. { h } else { old.h },
                            url: if url.is_empty() { old.url } else { url },
                        },
                    );
                }
            } else if let (Some(old), Some(new)) = (m["woke"].as_str(), m["as"].as_str()) {
                known.insert(new.to_string());
                s.woke.push((old.to_string(), new.to_string()));
                s.active = None;
            } else if let Some(e) = m["error"].as_str() {
                s.error = Some(e.to_string());
            }
            drop(s);
            let _ = wake.try_send(());
        }
        self.state
            .lock()
            .unwrap()
            .error
            .get_or_insert_with(|| "The browser's connection closed".into());
        let _ = wake.try_send(());
    }

    fn op(&self, v: Value) {
        let _ = self.out.send(v);
    }

    /// Stream `id` (one at a time). A sleeping tab sends its saved picture.
    pub fn show(&self, id: String) {
        let mut s = self.state.lock().unwrap();
        if s.active.as_deref() == Some(&id) || self.is_paused() {
            return;
        }
        s.active = Some(id.clone());
        drop(s);
        self.op(json!({"op": "watch", "tab": id}));
    }

    /// Reopen a sleeping tab (its new id comes back in `State::woke`).
    pub fn wake(&self, id: &str) {
        self.op(json!({"op": "wake", "tab": id}));
    }

    /// A new tab at `url`.
    pub fn open(&self, url: String) {
        self.op(json!({"op": "open", "url": url}));
    }

    #[allow(dead_code)]
    pub fn close(&self, id: &str) {
        self.op(json!({"op": "close", "tab": id}));
    }

    /// Stop (or resume) the stream; the last frame stays on screen.
    pub fn set_paused(&self, paused: bool) {
        let was = self
            .paused
            .swap(paused, std::sync::atomic::Ordering::Relaxed);
        if paused && !was {
            self.state.lock().unwrap().active = None;
            self.op(json!({"op": "unwatch"}));
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_open(&self, id: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .pages
            .iter()
            .any(|p| p.id == id && !p.asleep)
    }

    pub fn is_asleep(&self, id: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .pages
            .iter()
            .any(|p| p.id == id && p.asleep)
    }

    /// A mouse event at a point of the page (CSS pixels).
    pub fn mouse(&self, kind: &str, x: f32, y: f32, button: &str, clicks: u32) {
        self.op(json!({"op": "mouse", "kind": kind, "x": x, "y": y, "button": button, "clicks": clicks}));
    }

    pub fn wheel(&self, x: f32, y: f32, dx: f32, dy: f32) {
        self.op(json!({"op": "wheel", "x": x, "y": y, "dx": dx, "dy": dy}));
    }

    /// Typed text (as a paste of characters: layouts and IMEs are the Mac's business).
    pub fn text(&self, text: &str) {
        self.op(json!({"op": "text", "text": text}));
    }

    /// A key that is not text: Enter, Backspace, Tab, arrows, Escape (with ⌘/⇧/⌥/⌃ as given).
    pub fn key(&self, key: &str, code: &str, vk: u32, modifiers: u32) {
        self.op(json!({"op": "key", "key": key, "code": code, "vk": vk, "modifiers": modifiers}));
    }
}

/// A JPEG frame as the GPU image GPUI draws (BGRA), and its bytes (kept for the disk).
fn decode(b64: &str) -> Option<(Arc<RenderImage>, Vec<u8>)> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Some((image_of(&bytes)?, bytes))
}

/// A JPEG (a saved step) as the GPU image GPUI draws.
pub fn image_of(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
