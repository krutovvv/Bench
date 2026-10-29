//! What the thread's list shows, one file per kind of row: the person's and the agent's messages
//! (`message.rs`), the agent's work and its steps (`work.rs`; what one call says in a line is
//! `crates/thread`'s `tool_labels`), a waiting message (`queued.rs`), the agent's questions and
//! plan (`asks.rs`; answering a question: `clarify.rs`), its background processes
//! (`processes.rs`), the files it changed (`changed_files.rs`; a diff's line stats:
//! `diff_stats.rs`), the pages it read (`pages.rs`).

mod asks;
mod clarify;
mod changed_files;
pub(crate) mod diff_stats;
mod pages;
pub(crate) mod message;
mod processes;
mod queued;
mod steps;
mod welcome;
pub(crate) mod work;
