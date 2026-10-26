//! What a list on screen must do to go from one set of rows to the next.
//!
//! A row that stays at its place but changed (text streaming in, a step added, a result come)
//! is measured again, which keeps the reader's place even inside that row. Only the number of
//! rows changes by splicing, and that happens at the end. Splicing a row the reader is in sends
//! the view to the row's top: done for every streamed chunk, that was the thread jumping.

use std::ops::Range;

use crate::Row;

#[derive(Debug, Default, PartialEq)]
pub struct Change {
    /// Rows to measure again (their place is unchanged).
    pub remeasure: Option<Range<usize>>,
    /// Rows `range` replaced by `count` new ones: only ever at the end.
    pub splice: Option<(Range<usize>, usize)>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.remeasure.is_none() && self.splice.is_none()
    }
}

/// The change from `old` rows to `new` ones.
pub fn diff(old: &[Row], new: &[Row]) -> Change {
    let common = old.len().min(new.len());
    let changed: Vec<usize> = (0..common).filter(|&k| old[k] != new[k]).collect();
    let remeasure = match (changed.first(), changed.last()) {
        (Some(&a), Some(&b)) => Some(a..b + 1),
        _ => None,
    };
    let splice = (old.len() != new.len()).then(|| (common..old.len(), new.len() - common));
    Change { remeasure, splice }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(key: &str, text: &str) -> Row {
        Row::User {
            key: key.into(),
            text: text.into(),
            at: None,
        }
    }
    fn writing(text: &str) -> Row {
        Row::Writing { text: text.into() }
    }
    fn answer(key: &str, text: &str) -> Row {
        Row::Answer {
            key: key.into(),
            text: text.into(),
            error: None,
        }
    }

    #[test]
    fn a_streamed_chunk_remeasures_the_row_and_splices_nothing() {
        let old = [user("1", "go"), writing("Hel")];
        let new = [user("1", "go"), writing("Hello")];
        assert_eq!(
            diff(&old, &new),
            Change {
                remeasure: Some(1..2),
                splice: None
            }
        );
    }

    #[test]
    fn the_answer_replacing_what_was_written_is_a_remeasure_too() {
        let old = [user("1", "go"), writing("Hello")];
        let new = [user("1", "go"), answer("2", "Hello.")];
        assert_eq!(
            diff(&old, &new),
            Change {
                remeasure: Some(1..2),
                splice: None
            }
        );
    }

    #[test]
    fn new_rows_are_spliced_in_at_the_end() {
        let old = [user("1", "go")];
        let new = [user("1", "go"), answer("2", "ok"), user("3", "next")];
        assert_eq!(
            diff(&old, &new),
            Change {
                remeasure: None,
                splice: Some((1..1, 2))
            }
        );
    }

    #[test]
    fn rows_gone_from_the_end_are_spliced_out() {
        let old = [user("1", "go"), writing("par"), user("q", "later")];
        let new = [user("1", "go"), user("q", "later")];
        assert_eq!(
            diff(&old, &new),
            Change {
                remeasure: Some(1..2),
                splice: Some((2..3, 0))
            }
        );
    }

    #[test]
    fn nothing_changed_is_nothing_to_do() {
        let rows = [user("1", "go")];
        assert!(diff(&rows, &rows).is_empty());
    }
}
