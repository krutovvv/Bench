//! A thread as Bench shows it, without GPUI.
//!
//! Gives: `Thread` (fed with `open` and `apply(ThreadEvent)`) and `Row`s, the thread's lines in
//! order. Takes: the `protocol` types.
//!
//! The rules of order (each is a test in `tests.rs`):
//! 1. The journal comes first, ordered by item key; then the answer being written; then the queue.
//! 2. The queue keeps the agent's order ("send now" puts a message first, so it shows first).
//! 3. A queued message the agent takes becomes a journal row in the same place on screen.
//! 4. Steps and time are counted per piece of work, between two messages — never per run.
//! 5. An insert sits where its entry is; a removed one is not shown.
//! 6. A message just sent shows at once, after the journal, until the journal has it.

pub mod diff;
mod rows;
mod state;
#[cfg(test)]
mod tests;

pub use diff::{Change, diff};
pub use protocol::{Hold, PlanStep, ThreadEvent};
pub use rows::{AskCard, Row, Step, worked_ms};
pub use state::Thread;
