//! What the thread's list shows, one file per kind of row: the person's and the agent's messages
//! (`message.rs`), the agent's work and its steps (`work.rs`, what one call says in a line:
//! `tool_labels.rs`), a waiting message (`queued.rs`), the agent's questions and plan
//! (`asks.rs`, answering: `answer.rs`), its background processes (`processes.rs`), the files it
//! changed (`changed_files.rs`, a file's diff: `diff.rs`), the pages it read (`extras.rs`).

mod answer;
mod asks;
mod changed_files;
pub(crate) mod diff;
mod extras;
pub(crate) mod message;
mod processes;
mod queued;
mod steps;
pub(crate) mod tool_labels;
mod welcome;
pub(crate) mod work;
