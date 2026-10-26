//! The terminal view: the session's cell grid painted cell-exactly in Menlo (one shaped run per
//! style span, each placed at its column), cursor, selection; keys, mouse and wheel go back to
//! the session.

use std::sync::Arc;

use alacritty_terminal::grid::Dimensions as _;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, CursorShape, Rgb};
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::terminal::session::{Layout, Palette, Session, Size};

/// One Light / One Dark ANSI colors (0–15).
const LIGHT_ANSI: [u32; 16] = [
    0x383A42, 0xE45649, 0x50A14F, 0xC18401, 0x4078F2, 0xA626A4, 0x0184BC, 0xA0A1A7, 0x696C77,
    0xCA1243, 0x3F8A50, 0xB7700F, 0x2F5FD0, 0x8A2B9C, 0x0A7A9E, 0x383A42,
];
const DARK_ANSI: [u32; 16] = [
    0x3F4451, 0xE06C75, 0x98C379, 0xE5C07B, 0x61AFEF, 0xC678DD, 0x56B6C2, 0xABB2BF, 0x5C6370,
    0xEA858B, 0xAAD581, 0xF0CB86, 0x7DBEF3, 0xD49AE4, 0x6FC7D1, 0xDCE0E5,
];

fn to_rgb(c: Hsla) -> Rgb {
    let c: Rgba = c.into();
    let f = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Rgb {
        r: f(c.r),
        g: f(c.g),
        b: f(c.b),
    }
}

fn hex(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

fn hsla(c: Rgb) -> Hsla {
    rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32).into()
}

pub fn palette(theme: &Theme) -> Palette {
    let table = if theme.mode.is_dark() {
        DARK_ANSI
    } else {
        LIGHT_ANSI
    };
    Palette {
        ansi: table.map(hex),
        fg: to_rgb(theme.foreground),
        // A terminal is a block of the thread, on its tinted ground.
        bg: to_rgb(theme.secondary),
        cursor: to_rgb(theme.primary),
    }
}

fn resolve(c: Color, colors: &Colors, pal: &Palette) -> Rgb {
    match c {
        Color::Spec(rgb) => rgb,
        Color::Indexed(i) => colors[i as usize].unwrap_or_else(|| pal.indexed(i as usize)),
        Color::Named(n) => colors[n as usize].unwrap_or_else(|| match n as usize {
            i @ 0..16 => pal.ansi[i],
            256 | 267 => pal.fg,
            258 => pal.cursor,
            i @ 259..=266 => dim(pal.ansi[i - 259]),
            268 => dim(pal.fg),
            _ => pal.bg,
        }),
    }
}

fn dim(c: Rgb) -> Rgb {
    let f = |v: u8| (v as f32 * 0.66) as u8;
    Rgb {
        r: f(c.r),
        g: f(c.g),
        b: f(c.b),
    }
}

/// A span of cells with one style, starting at `col`.
struct Run {
    col: usize,
    text: String,
    fg: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    /// A double-width glyph: alone in its run (each glyph is forced to one cell's advance).
    wide: bool,
}

/// What one frame paints, taken under the grid lock and painted without it.
struct Frame {
    rows: Vec<(Vec<(usize, usize, Hsla)>, Vec<Run>)>,
    selection: Vec<(usize, usize, usize)>,
    cursor: Option<(usize, usize, CursorShape, Hsla)>,
}

fn snapshot(session: &Session, pal: &Palette, focused: bool) -> Frame {
    let term = session.term.lock().unwrap();
    let content = term.renderable_content();
    let size = Size {
        cols: term.columns(),
        rows: term.screen_lines(),
    };
    let offset = content.display_offset as i32;
    let default_bg = pal.bg;
    let mut rows: Vec<(Vec<(usize, usize, Hsla)>, Vec<Run>)> =
        (0..size.rows).map(|_| (vec![], vec![])).collect();
    let mut sel_rows: Vec<(usize, usize, usize)> = vec![];
    let cursor_vis = content.mode.contains(TermMode::SHOW_CURSOR) && offset == 0;
    let cursor = content.cursor;
    let mut cursor_out = None;
    let mut cursor_cell: Option<(usize, usize)> = None;
    if cursor_vis && cursor.shape != CursorShape::Hidden {
        let row = cursor.point.line.0 + offset;
        if row >= 0 && (row as usize) < size.rows {
            cursor_cell = Some((row as usize, cursor.point.column.0));
            cursor_out = Some((
                row as usize,
                cursor.point.column.0,
                if focused {
                    cursor.shape
                } else {
                    CursorShape::HollowBlock
                },
                hsla(pal.cursor),
            ));
        }
    }
    let block_cursor = focused && matches!(cursor.shape, CursorShape::Block);
    // ⌘ held over a path or URL: it is underlined, like a link.
    let hover = *session.hover.lock().unwrap();

    for indexed in content.display_iter {
        let row = indexed.point.line.0 + offset;
        if row < 0 || row as usize >= size.rows {
            continue;
        }
        let row = row as usize;
        let col = indexed.point.column.0;
        let cell = indexed.cell;
        if let Some(sel) = &content.selection {
            if sel.contains(indexed.point) {
                match sel_rows.last_mut() {
                    Some((r, c0, n)) if *r == row && *c0 + *n == col => *n += 1,
                    _ => sel_rows.push((row, col, 1)),
                }
            }
        }
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let mut fg = resolve(cell.fg, content.colors, pal);
        let mut bg = resolve(cell.bg, content.colors, pal);
        if cell.flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if cell.flags.contains(Flags::DIM) {
            fg = dim(fg);
        }
        if cell.flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        if block_cursor && cursor_cell == Some((row, col)) {
            fg = pal.bg;
            bg = pal.cursor;
        }
        let width = if cell.flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };
        let (bgs, runs) = &mut rows[row];
        if bg != default_bg {
            let c = hsla(bg);
            match bgs.last_mut() {
                Some((c0, n, color)) if *color == c && *c0 + *n == col => *n += width,
                _ => bgs.push((col, width, c)),
            }
        }
        let ch = if cell.c == '\t' { ' ' } else { cell.c };
        if ch == ' ' && cell.extra.is_none() {
            continue;
        }
        let linked = hover.is_some_and(|(r, a, b)| r == row && (a..=b).contains(&col));
        let style = (
            hsla(fg),
            cell.flags.contains(Flags::BOLD),
            cell.flags.contains(Flags::ITALIC),
            linked || cell.flags.intersects(Flags::ALL_UNDERLINES),
            cell.flags.contains(Flags::STRIKEOUT),
        );
        let mut text = String::new();
        text.push(ch);
        for z in cell.zerowidth().into_iter().flatten() {
            text.push(*z);
        }
        match runs.last_mut() {
            Some(r)
                if width == 1
                    && !r.wide
                    && r.col + r.text.chars().count() == col
                    && (r.fg, r.bold, r.italic, r.underline, r.strike) == style =>
            {
                r.text.push_str(&text)
            }
            _ => runs.push(Run {
                col,
                text,
                fg: style.0,
                bold: style.1,
                italic: style.2,
                underline: style.3,
                strike: style.4,
                wide: width == 2,
            }),
        }
    }
    Frame {
        rows,
        selection: sel_rows,
        cursor: cursor_out,
    }
}

/// The grid element: measures cells, resizes the session to fit, paints a frame.
fn grid(session: Arc<Session>, focused: bool, theme: &Theme) -> impl IntoElement {
    let pal = palette(theme);
    let font_size = theme.mono_font_size;
    let sel_color = theme.selection;
    canvas(
        move |bounds, window, _cx| {
            let font = font("Menlo");
            let ts = window.text_system();
            let font_id = ts.resolve_font(&font);
            let cell_w = ts
                .advance(font_id, font_size, 'm')
                .map(|s| s.width)
                .unwrap_or(font_size * 0.6);
            let line_h = (font_size * 1.35).round();
            let cols = ((bounds.size.width / cell_w).floor() as usize).max(2);
            // The view may be shorter than the program's screen (a terminal in a thread): the
            // rows under it are cut off by the view.
            let rows = ((bounds.size.height / line_h).floor() as usize)
                .max(1)
                .max(session.min_rows());
            session.resize(
                Size { cols, rows },
                Layout {
                    origin_x: f32::from(bounds.origin.x),
                    origin_y: f32::from(bounds.origin.y),
                    cell_w: f32::from(cell_w),
                    line_h: f32::from(line_h),
                },
            );
            (snapshot(&session, &pal, focused), cell_w, line_h)
        },
        move |bounds, (frame, cell_w, line_h), window, cx| {
            let o = bounds.origin;
            let at = |row: usize, col: usize| {
                point(o.x + cell_w * col as f32, o.y + line_h * row as f32)
            };
            for (row, (bgs, _)) in frame.rows.iter().enumerate() {
                for (col, n, color) in bgs {
                    window.paint_quad(fill(
                        Bounds::new(at(row, *col), size(cell_w * *n as f32, line_h)),
                        *color,
                    ));
                }
            }
            for (row, col, n) in &frame.selection {
                window.paint_quad(fill(
                    Bounds::new(at(*row, *col), size(cell_w * *n as f32, line_h)),
                    sel_color,
                ));
            }
            for (row, (_, runs)) in frame.rows.iter().enumerate() {
                for r in runs {
                    let mut f = font("Menlo");
                    if r.bold {
                        f.weight = FontWeight::BOLD;
                    }
                    if r.italic {
                        f.style = FontStyle::Italic;
                    }
                    let run = TextRun {
                        len: r.text.len(),
                        font: f,
                        color: r.fg,
                        background_color: None,
                        underline: r.underline.then(|| UnderlineStyle {
                            thickness: px(1.),
                            color: Some(r.fg),
                            wavy: false,
                        }),
                        strikethrough: r.strike.then(|| StrikethroughStyle {
                            thickness: px(1.),
                            color: Some(r.fg),
                        }),
                    };
                    let shaped = window.text_system().shape_line(
                        SharedString::from(r.text.clone()),
                        font_size,
                        &[run],
                        Some(if r.wide { cell_w * 2. } else { cell_w }),
                    );
                    let _ = shaped.paint(at(row, r.col), line_h, TextAlign::Left, None, window, cx);
                }
            }
            if let Some((row, col, shape, color)) = frame.cursor {
                let origin = at(row, col);
                match shape {
                    CursorShape::Block => {} // drawn as an inverted cell above
                    CursorShape::Beam => {
                        window.paint_quad(fill(Bounds::new(origin, size(px(2.), line_h)), color))
                    }
                    CursorShape::Underline => window.paint_quad(fill(
                        Bounds::new(
                            point(origin.x, origin.y + line_h - px(2.)),
                            size(cell_w, px(2.)),
                        ),
                        color,
                    )),
                    _ => window.paint_quad(outline(
                        Bounds::new(origin, size(cell_w, line_h)),
                        color,
                        BorderStyle::Solid,
                    )),
                }
            }
        },
    )
    .size_full()
}

impl BenchApp {
    pub(crate) fn render_terminal(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let tab = &self.ssh.tabs[ix];
        let session = tab.session.clone();
        let focus = tab.focus.clone();
        let focused = focus.is_focused(window);
        let s_key = session.clone();
        let s_down = session.clone();
        let s_move = session.clone();
        let s_wheel = session.clone();
        let s_mods = session.clone();
        let (s_up, s_right, s_right_up) = (session.clone(), session.clone(), session.clone());
        let alias = tab.alias.clone();
        div()
            .id(("terminal", tab.id))
            .size_full()
            // Rows past the view's height (see Session::min_rows) stay hidden.
            .overflow_hidden()
            .bg(theme.background)
            .px_4()
            .py_2()
            .track_focus(&focus)
            .cursor(if session.hover.lock().unwrap().is_some() {
                CursorStyle::PointingHand
            } else {
                CursorStyle::IBeam
            })
            .on_key_down(cx.listener(move |_, ev: &KeyDownEvent, _, cx| {
                let ks = &ev.keystroke;
                if ks.modifiers.platform {
                    match ks.key.as_str() {
                        "c" => {
                            if let Some(text) = s_key.selection_text() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }
                        }
                        "v" => {
                            if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                                s_key.input(&crate::inserts::terminal::session::paste_bytes(
                                    &text,
                                    s_key.mode(),
                                ));
                            }
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                if let Some(bytes) = crate::inserts::terminal::session::key_bytes(ks, s_key.mode())
                {
                    s_key.input(&bytes);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            // Left button: ⌘-click opens a link or a path; a program that asked for the mouse
            // gets it (Shift still selects); otherwise it selects text.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    focus.focus(window, cx);
                    let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
                    let (col, row) = s_down.cell_at(x, y);
                    if ev.modifiers.platform {
                        if let Some(link) = s_down.link_at(col, row) {
                            this.open_terminal_link(alias.clone(), link, window, cx);
                        }
                        return;
                    }
                    if s_down.wants_mouse() && !ev.modifiers.shift {
                        s_down.mouse_report(0, false, false, col, row);
                        return;
                    }
                    s_down.select_start(x, y, ev.click_count);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |_, ev: &MouseUpEvent, _, _| {
                    if s_up.wants_mouse() && !ev.modifiers.shift {
                        let (col, row) =
                            s_up.cell_at(f32::from(ev.position.x), f32::from(ev.position.y));
                        s_up.mouse_report(0, false, true, col, row);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |_, ev: &MouseDownEvent, _, _| {
                    if s_right.wants_mouse() {
                        let (col, row) =
                            s_right.cell_at(f32::from(ev.position.x), f32::from(ev.position.y));
                        s_right.mouse_report(2, false, false, col, row);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |_, ev: &MouseUpEvent, _, _| {
                    if s_right_up.wants_mouse() {
                        let (col, row) =
                            s_right_up.cell_at(f32::from(ev.position.x), f32::from(ev.position.y));
                        s_right_up.mouse_report(2, false, true, col, row);
                    }
                }),
            )
            // ⌘ let go: the link is no longer underlined.
            .on_modifiers_changed(cx.listener(move |_, ev: &ModifiersChangedEvent, _, cx| {
                if !ev.modifiers.platform && s_mods.hover_at(None) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(move |_, ev: &MouseMoveEvent, _, cx| {
                let (x, y) = (f32::from(ev.position.x), f32::from(ev.position.y));
                let cell = ev.modifiers.platform.then(|| s_move.cell_at(x, y));
                if s_move.hover_at(cell) {
                    cx.notify();
                }
                if s_move.wants_mouse() && !ev.modifiers.shift {
                    if ev.pressed_button == Some(MouseButton::Left) {
                        let (col, row) = s_move.cell_at(x, y);
                        s_move.mouse_report(0, true, false, col, row);
                    }
                    return;
                }
                if ev.pressed_button == Some(MouseButton::Left) {
                    s_move.select_update(x, y);
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(move |this, ev: &ScrollWheelEvent, _, cx| {
                let line_h = px(s_wheel.layout().line_h.max(1.));
                let dy = f32::from(ev.delta.pixel_delta(line_h).y);
                let acc = this.ssh.scroll_acc + dy / f32::from(line_h);
                let lines = acc.trunc() as i32;
                this.ssh.scroll_acc = acc - lines as f32;
                if lines == 0 {
                    return;
                }
                let mode = s_wheel.mode();
                if s_wheel.wants_mouse() {
                    // The program takes the wheel itself (buttons 64/65).
                    let (col, row) =
                        s_wheel.cell_at(f32::from(ev.position.x), f32::from(ev.position.y));
                    let b = if lines > 0 { 64 } else { 65 };
                    for _ in 0..lines.unsigned_abs() {
                        s_wheel.mouse_report(b, false, false, col, row);
                    }
                } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
                    // Full-screen programs (less, vim, htop) get arrow keys instead.
                    let key = if lines > 0 { "A" } else { "B" };
                    let seq = if mode.contains(TermMode::APP_CURSOR) {
                        format!("\x1bO{key}")
                    } else {
                        format!("\x1b[{key}")
                    };
                    s_wheel.write(seq.repeat(lines.unsigned_abs() as usize).as_bytes());
                } else {
                    s_wheel.scroll(lines);
                }
                cx.notify();
            }))
            .child(grid(session, focused, &theme))
            .into_any_element()
    }
}
