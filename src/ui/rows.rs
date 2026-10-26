//! The thread's rows as numbers: how tall each one is, where it begins, which ones the
//! viewport needs. The renderer realizes only those and holds the rest behind two spacers, so a
//! frame costs what is on screen, not what the thread has ever said — the length of a thread
//! stops being a cost. Heights are learned from the rows as they are laid out and remembered,
//! so the sums are exact for everything already seen and estimated for the rest.

use std::collections::HashMap;

/// A row of the thread, identified by what it shows rather than by its place: rows are added in
/// the middle (an insert, a new turn) and a height measured yesterday belongs to the content,
/// not to the index it happened to have then.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RowKey {
    /// The turn's own row (its message, its steps, its answer).
    Turn(usize),
    /// An insert, at its place among the turns.
    Insert(String),
    /// The agent's plan row.
    Plan,
    /// The row of the agent's background processes.
    Procs,
    /// What the person typed and has not yet sent.
    Queue,
    /// The composer, the thread's last row.
    Composer,
    /// Any other block that hangs under a turn (an approval, a form, an open file).
    Block(String),
}

/// A row that has never been laid out is counted at this height until it is seen.
const UNKNOWN: f32 = 120.;

/// How far past the viewport rows are still realized, so scrolling finds them already measured
/// instead of showing a hole. Measured in viewports.
pub const OVERSCAN: f32 = 2.;

/// The rows and their heights.
#[derive(Default)]
pub struct Rows {
    /// What is on the thread now, in order.
    keys: Vec<RowKey>,
    /// Heights learned by laying rows out, kept across frames (and across threads: the same
    /// turn keeps its height when the thread is reopened).
    measured: HashMap<RowKey, f32>,
    /// Where each row begins: `starts[i]` is the distance from the top of the content to row i.
    starts: Vec<f32>,
    /// The whole content's height.
    content: f32,
}

impl Rows {
    /// Adopt this shape of rows, keeping the heights of the rows that are still there. Returns
    /// whether anything changed, which is what tells the caller the frame must be re-measured.
    pub fn sync(&mut self, keys: &[RowKey]) -> bool {
        let changed = keys != self.keys;
        if changed {
            self.keys = keys.to_vec();
            let keep: std::collections::HashSet<&RowKey> = self.keys.iter().collect();
            self.measured.retain(|k, _| keep.contains(k));
        }
        self.rebuild();
        changed
    }

    /// The height row `i` is counted at: the one measured from it, or the average of the measured
    /// ones when it has not been laid out yet.
    fn height(&self, i: usize) -> f32 {
        match self.keys.get(i).and_then(|k| self.measured.get(k)) {
            Some(h) => *h,
            None => self.estimate(),
        }
    }

    /// The stand-in for a row nobody has measured: the mean of the measured heights, so a thread
    /// whose first screenful is known already sizes the rest about right.
    fn estimate(&self) -> f32 {
        let (sum, n) = self
            .measured
            .values()
            .fold((0f32, 0usize), |(s, n), h| (s + h, n + 1));
        if n == 0 {
            UNKNOWN
        } else {
            (sum / n as f32).max(1.)
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

    /// Remember the height row `i` was measured at. Returns by how much the content grew (or
    /// shrank) — the caller shifts the view by that much when the row is above the viewport, so
    /// what is being read does not slide out from under the eyes.
    pub fn set_measured(&mut self, i: usize, h: f32) -> Option<f32> {
        let key = self.keys.get(i)?.clone();
        let h = h.max(1.);
        let before = self.measured.get(&key).copied();
        if before.is_some_and(|b| (b - h).abs() < 0.5) {
            return None;
        }
        let delta = before.map(|b| h - b).unwrap_or(0.);
        self.measured.insert(key, h);
        self.rebuild();
        Some(if before.is_some() { delta } else { 0. })
    }

    /// Whether the row was measured at all: an unmeasured row is counted at a guess, so the sums
    /// around it are not yet to be trusted.
    pub fn is_measured(&self, i: usize) -> bool {
        self.keys.get(i).is_some_and(|k| self.measured.contains_key(k))
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
        ends_before(from)..ends_before(to).max(ends_before(from) + 1).min(self.keys.len())
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
        r.sync(&turns(10));
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
    fn an_unmeasured_row_is_counted_at_the_average_of_the_known_ones() {
        let mut r = Rows::default();
        r.sync(&turns(4));
        // Nothing measured yet: a standing guess, so the content is not zero.
        assert_eq!(r.content(), 4f32 * UNKNOWN);
        r.set_measured(0, 50.);
        r.set_measured(1, 150.);
        // The mean of the two known ones stands in for the two unknown: the third row begins
        // where the second ends, and the content carries two guesses on top.
        assert_eq!(r.start(2), 50. + 150.);
        assert_eq!(r.content(), 50. + 150. + 2f32 * 100.);
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
        // A row seen for the first time does not move anything that was already counted.
        let mut r2 = Rows::default();
        r2.sync(&turns(2));
        assert_eq!(r2.set_measured(0, 90.), Some(0.));
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
        big.sync(&turns(100));
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
        // An insert arrives after the third turn: the turns shift by one place and keep heights.
        let mut keys = turns(10);
        keys.insert(3, RowKey::Insert("a".into()));
        r.sync(&keys);
        assert_eq!(r.lens(), 11);
        assert_eq!(r.start(1), 100.);
        // The new row itself is a guess until it is laid out.
        assert!(!r.is_measured(3));
        assert!(r.is_measured(4));
        // Rows that went away are forgotten, so a reopened thread measures afresh.
        r.sync(&turns(2));
        assert_eq!(r.lens(), 2);
    }
}