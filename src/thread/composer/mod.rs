//! The thread's next message: the field (`input.rs`) with its "/" list (`slash.rs`) and saved
//! prompts (`prompts.rs`), what goes with it (`attachments.rs`: files and code references), the
//! unsent text kept per thread (`drafts.rs`), and sending it (`send.rs`, the reasoning level).

pub(crate) mod attachments;
pub(crate) mod drafts;
mod input;
mod prompts;
pub(crate) mod send;
mod slash;
