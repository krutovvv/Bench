//! One terminal session: a PTY (portable-pty) running `/usr/bin/ssh <host>` on the shared
//! connection (`ssh.rs`), its output parsed into a cell grid by `alacritty_terminal` on a reader
//! thread. The view (`ui/terminal.rs`) locks the grid to paint it and sends key bytes back
//! through `write`.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, Rgb};
use anyhow::{Context as _, Result};
use gpui_kit::Keystroke;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Grid size handed to alacritty.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// The 16 ANSI colors + foreground, background, cursor, in the app's current theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub ansi: [Rgb; 16],
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
}

impl Palette {
    /// Color `i` of the xterm 256-color table (OSC 4 index space), 256.. = fg/bg/cursor.
    pub fn indexed(&self, i: usize) -> Rgb {
        match i {
            0..16 => self.ansi[i],
            16..232 => {
                let i = i - 16;
                let level = |v: usize| if v == 0 { 0 } else { (55 + v * 40) as u8 };
                Rgb {
                    r: level(i / 36),
                    g: level(i / 6 % 6),
                    b: level(i % 6),
                }
            }
            232..256 => {
                let v = (8 + (i - 232) * 10) as u8;
                Rgb { r: v, g: v, b: v }
            }
            256 => self.fg,
            258 => self.cursor,
            _ => self.bg,
        }
    }
}

/// Cell metrics and where the grid was painted, for mouse → cell mapping and size reports.
#[derive(Clone, Copy, Default)]
pub struct Layout {
    pub origin_x: f32,
    pub origin_y: f32,
    pub cell_w: f32,
    pub line_h: f32,
}

/// Receives alacritty's requests (answers to queries, title) on the reader thread.
#[derive(Clone)]
pub struct Listener {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    title: Arc<Mutex<Option<String>>>,
    palette: Arc<Mutex<Palette>>,
    layout: Arc<Mutex<(Layout, Size)>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let reply = match event {
            Event::PtyWrite(text) => Some(text),
            Event::Title(t) => {
                *self.title.lock().unwrap() = Some(t);
                None
            }
            Event::ResetTitle => {
                *self.title.lock().unwrap() = None;
                None
            }
            // OSC 4/10/11 queries: vim asks for the background to pick light or dark colors.
            Event::ColorRequest(i, format) => Some(format(self.palette.lock().unwrap().indexed(i))),
            Event::TextAreaSizeRequest(format) => {
                let (l, s) = *self.layout.lock().unwrap();
                Some(format(WindowSize {
                    num_lines: s.rows as u16,
                    num_cols: s.cols as u16,
                    cell_width: l.cell_w as u16,
                    cell_height: l.line_h as u16,
                }))
            }
            _ => None,
        };
        if let Some(text) = reply {
            let mut w = self.writer.lock().unwrap();
            let _ = w.write_all(text.as_bytes());
            let _ = w.flush();
        }
    }
}

pub struct Session {
    pub term: Mutex<Term<Listener>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    listener: Listener,
    size: Mutex<Size>,
    pub exited: AtomicBool,
    /// Why the session ended (exit status), shown in the header.
    pub exit_note: Mutex<Option<String>>,
}

/// The host name of the terminal on this machine: the user's own login shell, no ssh.
pub const LOCAL: &str = "";

impl Session {
    /// Start `/usr/bin/ssh <host>` (or, for [`LOCAL`], the login shell) in a new PTY. `wake`
    /// gets a message whenever new output was parsed (at most one queued: the view repaints once
    /// per batch).
    pub fn spawn(
        host: &str,
        palette: Palette,
        wake: async_channel::Sender<()>,
    ) -> Result<Arc<Self>> {
        let size = Size {
            cols: 100,
            rows: 30,
        };
        let pair = native_pty_system()
            .openpty(pty_size(size))
            .context("open a pseudo-terminal")?;
        let local = host == LOCAL;
        let program = if local {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
        } else {
            "/usr/bin/ssh".into()
        };
        let mut cmd = CommandBuilder::new(&program);
        if local {
            // A login shell, as Terminal.app starts it (the profile sets PATH and the prompt).
            cmd.arg("-l");
        } else {
            // The shared connection: this terminal reuses it, or opens it for the IDE and the agent.
            cmd.args(crate::ssh::share_args());
            cmd.arg(host);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // A Finder-launched app has no locale; ssh forwards LANG/LC_* (SendEnv) to the server.
        if std::env::var("LANG").map_or(true, |l| l.is_empty()) {
            cmd.env("LANG", "en_US.UTF-8");
        }
        if let Ok(home) = std::env::var("HOME") {
            cmd.cwd(home);
        }
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("start {program}"))?;
        drop(pair.slave);
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let listener = Listener {
            writer: Arc::new(Mutex::new(writer)),
            title: Arc::new(Mutex::new(None)),
            palette: Arc::new(Mutex::new(palette)),
            layout: Arc::new(Mutex::new((Layout::default(), size))),
        };
        let config = Config {
            scrolling_history: 10_000,
            ..Default::default()
        };
        let session = Arc::new(Self {
            term: Mutex::new(Term::new(config, &size, listener.clone())),
            master: Mutex::new(pair.master),
            killer: Mutex::new(killer),
            listener,
            size: Mutex::new(size),
            exited: AtomicBool::new(false),
            exit_note: Mutex::new(None),
        });

        let this = session.clone();
        std::thread::spawn(move || {
            let mut parser: Processor = Processor::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        parser.advance(&mut *this.term.lock().unwrap(), &buf[..n]);
                        let _ = wake.try_send(());
                    }
                }
            }
            let name = if local { "The shell" } else { "ssh" };
            let note = match child.wait() {
                Ok(status) if status.success() && local => "The shell has exited".to_string(),
                Ok(status) if status.success() => "Connection closed".to_string(),
                Ok(status) => format!("{name} exited with code {}", status.exit_code()),
                Err(e) => format!("{name}: {e}"),
            };
            *this.exit_note.lock().unwrap() = Some(note);
            this.exited.store(true, Ordering::SeqCst);
            let _ = wake.try_send(());
        });
        Ok(session)
    }

    pub fn write(&self, bytes: &[u8]) {
        let mut w = self.listener.writer.lock().unwrap();
        let _ = w.write_all(bytes);
        let _ = w.flush();
    }

    /// Typed text goes to the program; the view jumps back to the bottom like any terminal.
    pub fn input(&self, bytes: &[u8]) {
        {
            let mut term = self.term.lock().unwrap();
            term.scroll_display(Scroll::Bottom);
            term.selection = None;
        }
        self.write(bytes);
    }

    pub fn title(&self) -> Option<String> {
        self.listener.title.lock().unwrap().clone()
    }

    pub fn set_palette(&self, palette: Palette) {
        *self.listener.palette.lock().unwrap() = palette;
    }

    pub fn layout(&self) -> Layout {
        self.listener.layout.lock().unwrap().0
    }

    /// Called while painting: the grid follows the view's size.
    pub fn resize(&self, size: Size, layout: Layout) {
        *self.listener.layout.lock().unwrap() = (layout, size);
        let mut cur = self.size.lock().unwrap();
        if *cur == size {
            return;
        }
        *cur = size;
        self.term.lock().unwrap().resize(size);
        let _ = self.master.lock().unwrap().resize(pty_size(size));
    }

    pub fn size(&self) -> Size {
        *self.size.lock().unwrap()
    }

    pub fn scroll(&self, lines: i32) {
        self.term
            .lock()
            .unwrap()
            .scroll_display(Scroll::Delta(lines));
    }

    /// Viewport cell under a window position (clamped into the grid).
    pub fn point_at(&self, x: f32, y: f32) -> (Point, Side) {
        let l = self.layout();
        let size = self.size();
        let term = self.term.lock().unwrap();
        let offset = term.grid().display_offset() as i32;
        let fx = ((x - l.origin_x) / l.cell_w.max(1.)).max(0.);
        let col = (fx as usize).min(size.cols.saturating_sub(1));
        let row = (((y - l.origin_y) / l.line_h.max(1.)).max(0.) as usize)
            .min(size.rows.saturating_sub(1));
        let side = if fx.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        (Point::new(Line(row as i32 - offset), Column(col)), side)
    }

    /// The viewport cell (column, row) under a window position, clamped into the grid.
    pub fn cell_at(&self, x: f32, y: f32) -> (usize, usize) {
        let l = self.layout();
        let size = self.size();
        let col = (((x - l.origin_x) / l.cell_w.max(1.)).max(0.) as usize)
            .min(size.cols.saturating_sub(1));
        let row = (((y - l.origin_y) / l.line_h.max(1.)).max(0.) as usize)
            .min(size.rows.saturating_sub(1));
        (col, row)
    }

    /// The program asked for mouse events (htop, mc, vim with `mouse=a`…).
    pub fn wants_mouse(&self) -> bool {
        self.mode().intersects(TermMode::MOUSE_MODE)
    }

    /// Send a mouse event the xterm way: SGR (`ESC [ < b ; x ; y M/m`) when the program asked
    /// for it, else the legacy `ESC [ M` bytes. `button`: 0 left, 1 middle, 2 right, 64/65
    /// wheel up/down; `drag` adds 32; `release` ends a press.
    pub fn mouse_report(&self, button: u8, drag: bool, release: bool, col: usize, row: usize) {
        let mode = self.mode();
        if drag && !mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
            return;
        }
        let b = button + if drag { 32 } else { 0 };
        let (x, y) = (col + 1, row + 1);
        let bytes = if mode.contains(TermMode::SGR_MOUSE) {
            format!("\x1b[<{b};{x};{y}{}", if release { 'm' } else { 'M' }).into_bytes()
        } else {
            // Legacy: one byte per value, 32-based; a release is button 3.
            let b = if release { 3 } else { b };
            let enc = |v: usize| (32 + v.min(223)) as u8;
            vec![0x1b, b'[', b'M', 32 + b, enc(x), enc(y)]
        };
        self.write(&bytes);
    }

    /// The word at a cell, for ⌘-click: a URL, else a path-like token (has a `/`).
    pub fn link_at(&self, col: usize, row: usize) -> Option<String> {
        let term = self.term.lock().unwrap();
        let offset = term.grid().display_offset() as i32;
        let line = &term.grid()[Line(row as i32 - offset)];
        let chars: Vec<char> = (0..term.columns()).map(|c| line[Column(c)].c).collect();
        drop(term);
        link_in(&chars, col)
    }

    pub fn select_start(&self, x: f32, y: f32, clicks: usize) {
        let (point, side) = self.point_at(x, y);
        let ty = match clicks {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.term.lock().unwrap().selection = Some(Selection::new(ty, point, side));
    }

    pub fn select_update(&self, x: f32, y: f32) {
        let (point, side) = self.point_at(x, y);
        if let Some(sel) = self.term.lock().unwrap().selection.as_mut() {
            sel.update(point, side);
        }
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term
            .lock()
            .unwrap()
            .selection_to_string()
            .filter(|s| !s.is_empty())
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().unwrap().mode()
    }

    /// Hang up the ssh client (SIGHUP), as closing a terminal window does.
    pub fn kill(&self) {
        let _ = self.killer.lock().unwrap().kill();
    }
}

fn pty_size(s: Size) -> PtySize {
    PtySize {
        rows: s.rows as u16,
        cols: s.cols as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// The bytes a key press sends to the program (xterm conventions). `None` = not for the PTY
/// (⌘ shortcuts are the app's).
pub fn key_bytes(ks: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let m = &ks.modifiers;
    if m.platform {
        return None;
    }
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    // xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl.
    let modp = 1 + m.shift as u8 + 2 * m.alt as u8 + 4 * m.control as u8;
    let csi = |final_: char| -> Vec<u8> {
        if modp > 1 {
            format!("\x1b[1;{modp}{final_}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{final_}").into_bytes()
        } else {
            format!("\x1b[{final_}").into_bytes()
        }
    };
    let tilde = |n: u8| -> Vec<u8> {
        if modp > 1 {
            format!("\x1b[{n};{modp}~").into_bytes()
        } else {
            format!("\x1b[{n}~").into_bytes()
        }
    };
    let alt = |mut b: Vec<u8>| -> Vec<u8> {
        if m.alt {
            b.insert(0, 0x1b);
        }
        b
    };
    let bytes = match ks.key.as_str() {
        "enter" => alt(b"\r".to_vec()),
        "tab" if m.shift => b"\x1b[Z".to_vec(),
        "tab" => b"\t".to_vec(),
        "escape" => b"\x1b".to_vec(),
        "backspace" if m.control => b"\x08".to_vec(),
        "backspace" => alt(b"\x7f".to_vec()),
        "space" if m.control => vec![0],
        "up" => csi('A'),
        "down" => csi('B'),
        "right" => csi('C'),
        "left" => csi('D'),
        "home" => csi('H'),
        "end" => csi('F'),
        "insert" => tilde(2),
        "delete" => tilde(3),
        "pageup" => tilde(5),
        "pagedown" => tilde(6),
        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        "f5" => tilde(15),
        "f6" => tilde(17),
        "f7" => tilde(18),
        "f8" => tilde(19),
        "f9" => tilde(20),
        "f10" => tilde(21),
        "f11" => tilde(23),
        "f12" => tilde(24),
        key if m.control && key.chars().count() == 1 => {
            let c = key.chars().next()?.to_ascii_lowercase();
            let b = match c {
                'a'..='z' => c as u8 - b'a' + 1,
                '@' | '2' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '_' | '-' | '7' => 0x1f,
                '8' | '?' => 0x7f,
                _ => return None,
            };
            alt(vec![b])
        }
        // Option works as Meta: ESC + the key itself (not the macOS accented character).
        key if m.alt && key.chars().count() == 1 => {
            let s = if m.shift {
                key.to_uppercase()
            } else {
                key.to_string()
            };
            alt(s.into_bytes())
        }
        "space" => b" ".to_vec(),
        _ => ks.key_char.as_ref()?.as_bytes().to_vec(),
    };
    Some(bytes)
}

/// The link around `col` in a terminal line: a URL, else a path-like token (has a `/`), without
/// trailing punctuation or a `:line[:col]` suffix.
pub fn link_in(chars: &[char], col: usize) -> Option<String> {
    let stop = |c: char| c.is_whitespace() || "\"'<>()[]{}`|,;".contains(c);
    if col >= chars.len() || stop(chars[col]) {
        return None;
    }
    let mut a = col;
    while a > 0 && !stop(chars[a - 1]) {
        a -= 1;
    }
    let mut b = col;
    while b + 1 < chars.len() && !stop(chars[b + 1]) {
        b += 1;
    }
    let word: String = chars[a..=b].iter().collect();
    // Trailing punctuation and a `path:line` suffix are not part of it.
    let word = word.trim_end_matches(['.', ':', '!', '?']);
    let word = match word.split_once(':') {
        Some((p, rest))
            if !word.contains("://") && rest.chars().all(|c| c.is_ascii_digit() || c == ':') =>
        {
            p
        }
        _ => word,
    };
    (word.contains("://") || word.contains('/')).then(|| word.to_string())
}

/// Pasted text, bracketed when the program asked for it.
pub fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if mode.contains(TermMode::BRACKETED_PASTE) {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::link_in;

    fn at(line: &str, word: &str) -> Option<String> {
        let chars: Vec<char> = line.chars().collect();
        let col = line.find(word).unwrap() + 1;
        link_in(&chars, line[..col].chars().count())
    }

    #[test]
    fn links_in_terminal_lines() {
        assert_eq!(
            at("see https://example.com/a?b=1. ok", "https"),
            Some("https://example.com/a?b=1".into())
        );
        assert_eq!(
            at("error in /etc/nginx/conf.d/site.conf:12:5 here", "/etc"),
            Some("/etc/nginx/conf.d/site.conf".into())
        );
        assert_eq!(
            at("open ~/app/main.py, then", "~/app"),
            Some("~/app/main.py".into())
        );
        assert_eq!(at("just words here", "words"), None);
    }
}
