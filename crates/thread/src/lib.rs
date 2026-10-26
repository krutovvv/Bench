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

mod rows;
mod state;
#[cfg(test)]
mod tests;

pub use rows::{Row, Step};
pub use state::Thread;
