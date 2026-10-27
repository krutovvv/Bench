//! The thread's list state (`View`): what it shows, how it is scrolled, the room past the end, and
//! the rows synced from the journal by diff.

use ::thread::Row;
use gpui_kit::*;

/// What the list shows and how it is scrolled. Owned by `BenchApp`.
pub struct View {
    pub list: ListState,
    /// The thread's rows, as last given to the list.
    pub rows: Vec<Row>,
    /// The thread's version they were made at.
    pub(super) version: u64,
    /// Which thread they are of: another one starts at the top.
    pub(super) owner: Option<String>,
    /// While a run is live, a timer repaints once a second (its clock).
    pub(super) ticker: Option<Task<()>>,
    /// The person sent a message: once the rows have changed and are laid out, the composer is
    /// brought into view if they pushed it past the bottom edge. Nothing else moves the thread
    /// but the person (as code in an editor).
    reveal: Reveal,
    /// The room past the end, worked out before the list lays out (`room_past_end`): the list
    /// cannot be asked while it renders its items.
    pub(super) room: Pixels,
    /// The composer is below the viewport ("Jump to latest" shows), as last known.
    pub(super) below: bool,
    /// `to_end` asked for this frame to lay out without the room past the end.
    pub(super) jump: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Reveal {
    Off,
    /// Sent; waiting for the rows to change.
    Armed,
    /// The rows changed; look after their layout.
    Check,
}

impl Default for View {
    fn default() -> Self {
        Self {
            list: ListState::new(3, ListAlignment::Top, px(800.)),
            rows: vec![],
            version: u64::MAX,
            owner: None,
            ticker: None,
            reveal: Reveal::Off,
            room: px(0.),
            below: false,
            jump: false,
        }
    }
}

impl View {
    /// The list's item of row `k`; the head is item 0.
    pub fn item_of(k: usize) -> usize {
        k + 1
    }

    /// The tail's item (the composer's).
    pub fn tail(&self) -> usize {
        self.rows.len() + 1
    }

    /// The room the thread scrolls on past its end, as an editor past its last line: exactly
    /// enough that, scrolled to the bottom, the tail (the composer) stands where it stands in a
    /// new thread — just under the air at the top. A new thread is then exactly as tall as its
    /// viewport and does not scroll at all. The tail's height is the last frame's; `window_h`
    /// stands in for the viewport before the list's first layout.
    pub(super) fn room_past_end(&self, window_h: Pixels) -> Pixels {
        let viewport = match self.list.viewport_bounds().size.height {
            h if h > px(0.) => h,
            _ => window_h - px(bench_ui::STATUS_H + 36.),
        };
        let tail = self
            .list
            .bounds_for_item(self.tail())
            .map_or(px(0.), |b| b.size.height);
        (viewport - px(bench_ui::TURN_Y) - tail).max(px(0.))
    }

    /// After a send, the rows laid out: the composer's bottom past the viewport's is brought up
    /// to it. A composer still in view (the thread grows into the room past the end) stays put.
    pub(super) fn reveal_after_send(&mut self) {
        if self.reveal != Reveal::Check {
            return;
        }
        self.reveal = Reveal::Off;
        let pushed = self
            .list
            .bounds_for_item(self.tail())
            .is_none_or(|b| b.bottom() > self.list.viewport_bounds().bottom() + px(1.));
        if pushed {
            self.to_end();
        }
    }

    /// Scroll so the composer's bottom is at the viewport's bottom. The list's own reveal adds up
    /// the heights of the rows before it, and rows never on screen count as nothing: in a long
    /// thread just opened it stops a message or two down. Scrolled to its end, the list lays out
    /// from the last item up, measuring as it goes — so for that frame the room past the end is
    /// nothing, and it comes back below the viewport the frame after.
    pub(super) fn to_end(&mut self) {
        self.jump = true;
        self.room = px(0.);
        let end = self.tail() + 1;
        self.list.remeasure_items(end..end + 1);
        self.list.scroll_to_end();
    }

    /// Give the list `rows` and answer the first row that changed. Unchanged rows keep their
    /// measured heights; a thread of another owner starts at the top, not following.
    pub(super) fn sync(&mut self, owner: Option<&str>, rows: Vec<Row>, follow: bool) -> usize {
        if follow {
            self.reveal = Reveal::Armed;
        }
        if owner != self.owner.as_deref() {
            self.owner = owner.map(str::to_string);
            self.rows = rows;
            self.list.reset(self.rows.len() + 3);
            self.list.scroll_to(ListOffset::default());
            self.reveal = Reveal::Off;
            return 0;
        }
        // Rows at their place are measured again (the reader keeps their place, even inside a row
        // being written); only the number of rows changes by splicing, at the end (`thread::diff`).
        let change = ::thread::diff(&self.rows, &rows);
        let first = change
            .remeasure
            .as_ref()
            .map(|r| r.start)
            .or(change.splice.as_ref().map(|(r, _)| r.start))
            .unwrap_or(rows.len());
        self.rows = rows;
        if let Some(r) = change.remeasure {
            self.list
                .remeasure_items(Self::item_of(r.start)..Self::item_of(r.end));
        }
        if let Some((r, count)) = change.splice {
            self.list
                .splice(Self::item_of(r.start)..Self::item_of(r.end), count);
        }
        if self.reveal == Reveal::Armed && first < self.rows.len() {
            self.reveal = Reveal::Check;
        }
        first
    }
}
