//! The thread's rows as numbers: how tall each one is, where it begins, which ones the
//! viewport needs. The renderer realizes only those and holds the rest behind two spacers, so a
//! frame costs what is on screen, not what the thread has ever said — the length of a thread
//! stops being a cost. Heights are learned from the rows as they are laid out and remembered,
//! so the sums are exact for everything already seen. What has not been seen is counted at a
//! guess that does NOT move: a guess that moves restacks the rows above the eyes, and the thread
//! jerks under the wheel — which is the one thing a scroll must never do.

use std::collections::HashMap;

/// A row of the thread, identified by what it shows rather than by its place: rows are added in
/// the middle (an insert, a new turn) and a height measured yesterday belongs to the content,
/// not to the index it happened to have then.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RowKey {
    /// A turn's row: its message, its steps, its answer, and whatever belongs under it (its
    /// inserts, the plan and the processes when their moment is there).
    Turn(usize),
    /// A row that is not a turn: the head of the thread (the environment's open rows, the
    /// placeholder) or its tail (the forms, the queue, the composer).
    Block(&'static str),
}

/// A row that has never been laid out is counted at this height until it is seen.
const UNKNOWN: f32 = 120.;

/// How far past the viewport rows are still realized, so scrolling finds them already measured
/// instead of showing a hole. Measured in viewports.
pub const OVERSCAN: f32 = 2.;

/// Bank a row's measured `delta` so the frame can pay it back. What moves the words being read
/// is the content ABOVE the top of the viewport (`off`): a row that begins above it, however far
/// down it ends, pushes everything under it down when it grows, and the frame owes the person the
/// same amount of scroll. A row that begins at or below the top changes only the room under the
/// eyes, which the person is moving through anyway. The test of this is the row's START, not its
/// end: a row counted too short before it was measured ends far below the top even though it
/// sits above it, and banking on its end is exactly what let a thread jump on the first scroll.
/// `row_start` is where the row began in the layout the person is looking at — taken before this
/// frame's measurements, so a frame that measures a band of rows banks all of them, not the ones
/// that happened to report first.
pub fn bank_shift(shift: f32, delta: f32, row_start: f32, off: f32) -> f32 {
    if delta != 0. && row_start < off {
        shift + delta
    } else {
        shift
    }
}

/// The rows and their heights.
#[derive(Default)]
pub struct Rows {
    /// What is on the thread now, in order.
    keys: Vec<RowKey>,
    /// Which thread these heights belong to. A turn numbered 3 in one thread is not the turn
    /// numbered 3 in the next; carrying the heights over sizes a freshly opened thread by the
    /// wrong numbers, which the first scroll then corrects row by row — under the eyes.
    owner: Option<String>,
    /// Heights learned by laying rows out, kept across frames of one thread.
    measured: HashMap<RowKey, f32>,
    /// What a row that has not been laid out is counted at, kept per row: taken once, when the
    /// row appeared, and never revised while it stays. A live mean would be closer to the truth
    /// and would move the rows above the viewport every time any row anywhere is measured.
    guess: HashMap<RowKey, f32>,
    /// Where each row begins: `starts[i]` is the distance from the top of the content to row i.
    starts: Vec<f32>,
    /// The whole content's height.
    content: f32,
}

impl Rows {
    /// Adopt this thread's shape of rows, keeping the heights of the rows that are still there.
    /// A different `owner` — another thread — forgets every height: they belong to the thread
    /// that was laid out, not to the numbers of these rows. Returns whether the shape changed.
    pub fn sync(&mut self, owner: Option<&str>, keys: &[RowKey]) -> bool {
        let switched = self.owner.as_deref().unwrap_or("") != owner.unwrap_or("");
        if switched {
            self.measured.clear();
            self.guess.clear();
            self.owner = owner.map(str::to_string);
        }
        let changed = keys != self.keys || switched;
        if changed {
            self.keys = keys.to_vec();
            let keep: std::collections::HashSet<&RowKey> = self.keys.iter().collect();
            self.measured.retain(|k, _| keep.contains(k));
            self.guess.retain(|k, _| keep.contains(k));
            // A row nobody has seen yet stands at the mean of what this thread has measured —
            // taken once, here, for THIS row. Rows already guessed keep their number.
            let mean = match self
                .measured
                .values()
                .fold((0f32, 0usize), |(s, n), h| (s + h, n + 1))
            {
                (_, 0) => UNKNOWN,
                (s, n) => (s / n as f32).max(1.),
            };
            for k in &self.keys {
                self.guess.entry(k.clone()).or_insert(mean);
            }
        }
        self.rebuild();
        changed
    }

    /// The height row `i` is counted at: the one measured from it, or the guess taken for it.
    fn height(&self, i: usize) -> f32 {
        let Some(key) = self.keys.get(i) else {
            return UNKNOWN;
        };
        match self.measured.get(key).or(self.guess.get(key)) {
            Some(h) => *h,
            None => UNKNOWN,
        }
    }

    fn rebuild(&mut self) {
        self.starts = Vec::with_capacity(self.keys.len());
        let mut y = 0f32;
        for i in 0..self.keys.len() {
            self.starts.push(y);
            y += self.height(i);
        }
        self.content = y;
    }

    /// How far from the top of the content row `i` begins (0 for the first row, the content's
    /// height for one past the end).
    pub fn start(&self, i: usize) -> f32 {
        match self.starts.get(i as usize) {
            Some(y) => *y,
            None => self.content,
        }
    }

    /// The whole height of the content, as the scroll container needs it.
    pub fn content(&self) -> f32 {
        self.content
    }

    pub fn lens(&self) -> usize {
        self.keys.len()
    }

    /// Remember the height row `i` was measured at. Returns by how much the content moved where
    /// that row stands — against whatever the row was counted at, its earlier measurement or the
    /// guess that stood in for it — so the caller can hold the view still when the row is above
    /// the eyes. A first measurement is NOT a free change: the row was already in the sums at its
    /// guess, and saying "0" there is what let the thread slide without any compensation.
    pub fn set_measured(&mut self, i: usize, h: f32) -> Option<f32> {
        let key = self.keys.get(i)?.clone();
        let h = h.max(1.);
        let known = self.measured.get(&key).copied();
        if known.is_some_and(|b| (b - h).abs() < 0.5) {
            return None;
        }
        let was = known.or_else(|| self.guess.get(&key).copied())?;
        let delta = h - was;
        self.measured.insert(key, h);
        self.rebuild();
        Some(if delta.abs() < 0.5 { 0. } else { delta })
    }

    /// The rows the viewport shows, plus the overscan on both sides, as a range of indices.
    /// Binary searches like the library's own list: row ends only grow with the index.
    pub fn visible(&self, offset: f32, viewport: f32) -> std::ops::Range<usize> {
        if self.keys.is_empty() {
            return 0..0;
        }
        let band = viewport * OVERSCAN;
        let from = (offset - band).max(0.);
        let to = offset + viewport + band;
        let ends_before = |edge: f32| {
            let (mut low, mut high) = (0usize, self.keys.len());
            while low < high {
                let mid = low + (high - low) / 2;
                if self.start(mid) + self.height(mid) <= edge {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            low
        };
        ends_before(from)
            ..ends_before(to)
                .max(ends_before(from) + 1)
                .min(self.keys.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turns(n: usize) -> Vec<RowKey> {
        (0..n).map(RowKey::Turn).collect()
    }

    /// Ten rows of 100 px each, all measured.
    fn ten() -> Rows {
        let mut r = Rows::default();
        r.sync(Some("t"), &turns(10));
        for i in 0..10 {
            r.set_measured(i, 100.);
        }
        r
    }

    #[test]
    fn rows_stack_from_the_top() {
        let r = ten();
        assert_eq!(r.start(0), 0.);
        assert_eq!(r.start(3), 300.);
        assert_eq!(r.content(), 1000.);
        // One past the end is the bottom of the content.
        assert_eq!(r.start(10), 1000.);
    }

    #[test]
    fn a_row_nobody_has_seen_keeps_the_height_it_was_counted_at() {
        let mut r = Rows::default();
        r.sync(Some("t"), &turns(4));
        // Nothing measured yet: the standing number, so the content is not zero.
        assert_eq!(r.content(), 4f32 * UNKNOWN);
        r.set_measured(0, 50.);
        r.set_measured(1, 150.);
        // The unseen rows keep the guess they were given when the thread appeared: measuring a
        // row here does not restack a row there.
        assert_eq!(r.start(2), 50. + 150.);
        assert_eq!(r.content(), 50. + 150. + 2f32 * UNKNOWN);
        // A row that APPEARS is guessed at the mean of what the thread has measured by then.
        let mut keys = turns(4);
        keys.insert(2, RowKey::Block("a"));
        r.sync(Some("t"), &keys);
        assert_eq!(r.height(2), 100.);
    }

    #[test]
    fn a_growth_is_reported_so_the_view_can_be_held_still() {
        let mut r = ten();
        // A row answering a reply taller than thought: the content grows by the difference.
        assert_eq!(r.set_measured(2, 180.), Some(80.));
        assert_eq!(r.content(), 1080.);
        assert_eq!(r.start(3), 380.);
        // The same number again is not a change.
        assert_eq!(r.set_measured(2, 180.), None);
        // A row seen for the first time reports the difference against the guess that stood in
        // for it — the content above the eyes did move by that much, and the frame owes it.
        let mut r2 = Rows::default();
        r2.sync(Some("t"), &turns(2));
        assert_eq!(r2.set_measured(0, 90.), Some(90. - UNKNOWN));
        // And at exactly the guess it stands at, it is no change at all.
        let mut r3 = Rows::default();
        r3.sync(Some("t"), &turns(2));
        assert_eq!(r3.set_measured(0, UNKNOWN), Some(0.));
    }

    #[test]
    fn the_viewport_sees_its_rows_plus_the_overscan() {
        let r = ten();
        // Viewport 300 px tall at the top: rows 0..2 are on screen, and the band of two extra
        // viewports reaches to row 8 — nine of the ten are realized with room to spare.
        assert_eq!(r.visible(0., 300.), 0..9);
        // Far down, the band does not run past the end, and the row that ends exactly at its
        // edge is already behind it: a spacer of 100 px stands in for it.
        assert_eq!(r.visible(700., 300.), 1..10);
        // A taller thread: only the band around the viewport is wanted.
        let mut big = Rows::default();
        big.sync(Some("t"), &turns(100));
        for i in 0..100 {
            big.set_measured(i, 100.);
        }
        // 600 px of overscan on each side of the 300 px viewport: the band 2400..3900, which is
        // rows 24..39 — a tenth of the thread, not a hundredth of a frame's work for all of it.
        assert_eq!(big.visible(3000., 300.), 24..39);
        // And it never comes back empty.
        assert!(big.visible(9_700., 300.).len() >= 1);
    }

    #[test]
    fn rows_keep_their_height_when_the_shape_changes() {
        let mut r = ten();
        // A row appears after the third turn: the ones after it shift by one place and keep the
        // heights they were measured at, while the new one is counted at their average.
        let mut keys = turns(10);
        keys.insert(3, RowKey::Block("a"));
        r.sync(Some("t"), &keys);
        assert_eq!(r.lens(), 11);
        assert_eq!(r.start(1), 100.);
        assert_eq!(r.content(), 11f32 * 100.);
        // Rows that went away are forgotten, so the bank holds only what a thread shows.
        r.sync(Some("t"), &turns(2));
        assert_eq!(r.lens(), 2);
    }
}

/// The thread's promise to the eyes: what is being read stays under them. The renderer realizes
/// a band around the viewport, learns the heights of those rows, banks what the rows above the
/// top of the viewport gained or lost (`bank_shift`), and the next frame pays the bank by moving
/// the scroll. These drive that arithmetic on the real `Rows`: an anchor row is put at the top of
/// the viewport, its neighbours are measured the way a scroll measures them, and the anchor must
/// still be at the top afterwards — otherwise the thread moved by itself, which is the jerk.
#[cfg(test)]
mod hold_tests {
    use super::{RowKey, Rows, UNKNOWN, bank_shift};

    fn turns(n: usize) -> Vec<RowKey> {
        (0..n).map(RowKey::Turn).collect()
    }

    /// The starts of every row as they stand in the layout the person is looking at. The
    /// renderer knows these when it builds the band, before any of this frame's measurements.
    fn seen(rows: &Rows) -> Vec<f32> {
        (0..rows.lens()).map(|i| rows.start(i)).collect()
    }

    /// Learn a row's height the way a painted row reports it: against the place it had in the
    /// layout being looked at, banking what moved the content above the top of the viewport.
    fn learn(rows: &mut Rows, i: usize, h: f32, seen: &[f32], off: f32, shift: &mut f32) {
        if let Some(delta) = rows.set_measured(i, h) {
            *shift = bank_shift(*shift, delta, seen[i], off);
        }
    }

    #[test]
    fn an_anchor_row_stays_put_while_the_rows_above_are_measured_for_the_first_time() {
        // A long thread: its first screenful was laid out and is short; the rows below it stand
        // at that average. The eyes are on row 4. Scrolling down realizes rows 2..6, and they
        // are nothing like the average — 500 px each. Rows 2 and 3 sit above the eyes and grow
        // by 380 px apiece; the frame must hand back 760 px of scroll, or the words jump up.
        let mut rows = Rows::default();
        rows.sync(Some("a"), &turns(20));
        for i in 0..2 {
            rows.set_measured(i, 60.);
        }
        let anchor = 4;
        let off = rows.start(anchor);
        let before = seen(&rows);
        let mut shift = 0f32;
        for i in 2..6 {
            learn(&mut rows, i, 500., &before, off, &mut shift);
        }
        assert_eq!(
            off + shift,
            rows.start(anchor),
            "the anchor slid by {:.0} px: scroll {:.0} + bank {:.0}, the row now begins at {:.0}",
            rows.start(anchor) - (off + shift),
            off,
            shift,
            rows.start(anchor)
        );
    }

    #[test]
    fn measuring_a_row_below_does_not_restack_the_rows_above() {
        // Learning a row far below must leave the rows above the eyes exactly where they were.
        // A guess that follows the live mean moves every unseen row each time any row is
        // measured, and only the measured row has a delta to bank — the rest slide for nothing.
        let mut rows = Rows::default();
        rows.sync(Some("a"), &turns(20));
        rows.set_measured(0, 100.);
        rows.set_measured(1, 100.);
        let above = rows.start(3); // row 2 is unseen: it stands at the guess it was given
        rows.set_measured(15, 900.);
        assert_eq!(
            rows.start(3),
            above,
            "an unseen row above moved by {:.0} px because a row far below was measured",
            rows.start(3) - above
        );
    }

    #[test]
    fn the_first_guess_starts_from_the_standing_number() {
        // Nothing measured yet: the thread is counted at `UNKNOWN` per row, so the scroll rail
        // and the room past the end have something to work with before anything is laid out.
        let mut rows = Rows::default();
        rows.sync(Some("a"), &turns(3));
        assert_eq!(rows.content(), 3f32 * UNKNOWN);
    }

    #[test]
    fn another_thread_does_not_open_at_the_heights_of_the_one_it_leaves() {
        // Turn 3 of one thread and turn 3 of the next are different rows that happen to share a
        // number. Carrying the heights over sizes a freshly opened thread by numbers from
        // somewhere else, and the first scroll spends the frame correcting them.
        let mut rows = Rows::default();
        rows.sync(Some("a"), &turns(4));
        for i in 0..4 {
            rows.set_measured(i, 900.);
        }
        assert_eq!(rows.content(), 4f32 * 900.);
        rows.sync(Some("b"), &turns(4));
        assert_eq!(rows.content(), 4f32 * UNKNOWN);
    }

    #[test]
    fn a_row_at_the_top_of_the_viewport_holding_still_is_not_banked() {
        // The row the eyes are on grows (an answer streaming under it): its own height must not
        // scroll the view — the top of that row is what the person is holding.
        let mut rows = Rows::default();
        rows.sync(Some("a"), &turns(4));
        let off = rows.start(2);
        assert_eq!(bank_shift(0., 300., off, off), 0.);
        // A row that ends one pixel above that line does move it.
        assert_eq!(bank_shift(0., 300., off - 1., off), 300.);
    }
}
