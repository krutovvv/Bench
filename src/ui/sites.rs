//! A step of a site thread: the page itself (no browser UI), as wide as the thread's text and
//! as tall as the page's frame. The latest step is live — clicks, scrolls and typing go to the
//! server's tab; an earlier one shows its last frame and reopens its page on a click.

use std::sync::Arc;

use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;

/// A step's frame at most this tall in the thread.
const MAX_H: f32 = 620.;

impl HermesApp {
    pub(super) fn render_site_step(&mut self, tab: String, turn: usize, url: String, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some((frame, w, h, live)) = self.step_frame(&tab) else {
            // Still coming (the tab is there, its first frame is not), or gone with no picture
            // left (the server's browser restarted): then say so, and offer the page again.
            let open = self.sites.browser.as_ref().is_some_and(|b| b.is_open(&tab));
            if open || url.is_empty() {
                return div()
                    .py_2()
                    .child(ShimmerText::new("Opening the page…").text_base().text_color(muted))
                    .into_any_element();
            }
            let thread = self.thread_key();
            let label = url_label(&url);
            return div()
                .id(SharedString::from(format!("site-gone-{tab}")))
                .px_3()
                .py_2()
                .border_1()
                .border_color(theme.border)
                .text_base()
                .text_color(muted)
                .cursor_pointer()
                .hover(|s| s.text_color(theme.foreground))
                .child(format!("{label}  ·  the page is closed — click to open it again"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.click_claimed = true;
                    this.reopen_step(thread.clone(), turn, url.clone(), cx)
                }))
                .into_any_element();
        };
        let size = frame.size(0);
        let (fw, fh) = (size.width.0 as f32, size.height.0 as f32);
        let bounds = self.sites.bounds.clone();
        let key = tab.clone();
        // Width from the last layout (the thread's text column); height by the frame's aspect.
        let width = bounds.borrow().get(&tab).map(|b| f32::from(b.size.width)).unwrap_or(720.);
        let height = (width * fh / fw.max(1.)).min(MAX_H);
        let (pw, ph) = if w > 0. { (w, h) } else { (fw, fh) };
        let focus = self.sites.focus.get_or_insert_with(|| cx.focus_handle()).clone();
        let b_down = self.sites.browser.clone();
        let (b_up, b_move, b_wheel, b_key) = (b_down.clone(), b_down.clone(), b_down.clone(), b_down.clone());
        let to_page = {
            let bounds = bounds.clone();
            let key = key.clone();
            move |p: Point<Pixels>| -> Option<(f32, f32)> {
                let b = *bounds.borrow().get(&key)?;
                let x = (f32::from(p.x - b.origin.x) / f32::from(b.size.width)).clamp(0., 1.) * pw;
                let y = (f32::from(p.y - b.origin.y) / f32::from(b.size.height)).clamp(0., 1.) * ph;
                Some((x, y))
            }
        };
        let (tp_down, tp_up, tp_move, tp_wheel) = (to_page.clone(), to_page.clone(), to_page.clone(), to_page);
        let thread = self.thread_key();
        let paused = self.sites.browser.as_ref().is_some_and(|b| b.is_paused());
        v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .id(SharedString::from(format!("site-{tab}")))
                    .relative()
                    .w_full()
                    .h(px(height))
                    // The picture never leaves its frame (a frame of another size than the
                    // last one must not spill over the thread).
                    .overflow_hidden()
                    .border_1()
                    .border_color(if live { theme.primary.opacity(0.5) } else { theme.border })
                    .when(live, |d| d.track_focus(&focus).cursor(CursorStyle::Arrow))
                    .when(!live, |d| d.cursor_pointer())
                    .child(img(ImageSource::Render(frame.clone())).absolute().inset_0().size_full().object_fit(ObjectFit::Contain))
                    // Where the frame is on screen, for mapping the pointer to the page.
                    .child(
                        canvas(
                            {
                                let bounds = bounds.clone();
                                let key = key.clone();
                                move |b, _, _| {
                                    bounds.borrow_mut().insert(key.clone(), b);
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                            this.click_claimed = true;
                            let was_paused = this.sites.browser.as_ref().is_some_and(|b| b.is_paused());
                            this.touch_browser();
                            // The first click on a paused view only wakes it.
                            if live && was_paused {
                                focus.focus(window, cx);
                                return;
                            }
                            if live {
                                focus.focus(window, cx);
                                if let (Some(b), Some((x, y))) = (&b_down, tp_down(ev.position)) {
                                    b.mouse("mousePressed", x, y, "left", ev.click_count as u32);
                                }
                            } else {
                                this.reopen_step(thread.clone(), turn, url.clone(), cx);
                            }
                        }),
                    )
                    .when(live, |d| {
                        d.on_mouse_up(MouseButton::Left, move |ev: &MouseUpEvent, _, _| {
                            if let (Some(b), Some((x, y))) = (&b_up, tp_up(ev.position)) {
                                b.mouse("mouseReleased", x, y, "left", ev.click_count as u32);
                            }
                        })
                        .on_mouse_move(move |ev: &MouseMoveEvent, _, _| {
                            if let (Some(b), Some((x, y))) = (&b_move, tp_move(ev.position)) {
                                let button = if ev.pressed_button == Some(MouseButton::Left) { "left" } else { "none" };
                                b.mouse("mouseMoved", x, y, button, 0);
                            }
                        })
                        .on_scroll_wheel(move |ev: &ScrollWheelEvent, window, cx| {
                            if let (Some(b), Some((x, y))) = (&b_wheel, tp_wheel(ev.position)) {
                                let d = ev.delta.pixel_delta(window.line_height());
                                b.wheel(x, y, -f32::from(d.x), -f32::from(d.y));
                                cx.stop_propagation();
                            }
                        })
                        .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                            this.sites.touched = Some(web_time::Instant::now());
                            if let Some(b) = &b_key {
                                if send_key(b, &ev.keystroke, cx) {
                                    cx.stop_propagation();
                                }
                            }
                            let _ = this;
                        }))
                    }),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .text_base()
                    .text_color(muted)
                    .truncate()
                    .child(if self.demo {
                        url_label(&self.step_url(&tab))
                    } else if !live {
                        format!("{}  ·  click to open it again", url_label(&self.step_url(&tab)))
                    } else if paused {
                        format!("{}  ·  paused — click to watch live", url_label(&self.step_url(&tab)))
                    } else {
                        url_label(&self.step_url(&tab))
                    }),
            )
            .into_any_element()
    }

    fn step_url(&self, tab: &str) -> String {
        self.sites
            .steps
            .values()
            .flatten()
            .find(|s| s.tab == tab)
            .map(|s| s.url.clone())
            .unwrap_or_default()
    }
}

/// A page's address as read: no scheme, no query (a search's long encoded query is noise).
fn url_label(url: &str) -> String {
    let u = url.trim_start_matches("https://").trim_start_matches("http://");
    u.split(['?', '#']).next().unwrap_or(u).trim_end_matches('/').to_string()
}

/// A key to the page: text as typed, the special keys by name, ⌘V pastes. Whether it was sent.
fn send_key(b: &Arc<crate::remote_browser::Browser>, ks: &Keystroke, cx: &mut App) -> bool {
    let m = &ks.modifiers;
    let mods = (m.alt as u32) | ((m.control as u32) << 1) | ((m.platform as u32) << 2) | ((m.shift as u32) << 3);
    if m.platform && ks.key == "v" {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            b.text(&text);
        }
        return true;
    }
    let special = match ks.key.as_str() {
        "enter" => Some(("Enter", "Enter", 13)),
        "backspace" => Some(("Backspace", "Backspace", 8)),
        "tab" => Some(("Tab", "Tab", 9)),
        "escape" => Some(("Escape", "Escape", 27)),
        "left" => Some(("ArrowLeft", "ArrowLeft", 37)),
        "up" => Some(("ArrowUp", "ArrowUp", 38)),
        "right" => Some(("ArrowRight", "ArrowRight", 39)),
        "down" => Some(("ArrowDown", "ArrowDown", 40)),
        "delete" => Some(("Delete", "Delete", 46)),
        "home" => Some(("Home", "Home", 36)),
        "end" => Some(("End", "End", 35)),
        "pageup" => Some(("PageUp", "PageUp", 33)),
        "pagedown" => Some(("PageDown", "PageDown", 34)),
        _ => None,
    };
    if let Some((key, code, vk)) = special {
        b.key(key, code, vk, mods);
        return true;
    }
    if m.platform || m.control {
        // ⌘A and the like, as the page's own shortcuts.
        let k = ks.key.to_uppercase();
        if k.len() == 1 {
            b.key(&ks.key, &format!("Key{k}"), k.chars().next().unwrap() as u32, mods);
            return true;
        }
        return false;
    }
    if let Some(text) = &ks.key_char {
        b.text(text);
        return true;
    }
    false
}
