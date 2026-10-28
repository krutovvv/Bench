//! The middle: the open documents as Zed's pane — their keys and opening, closing, saving
//! (`doc.rs`), a document as Zed's `Item` (`item.rs`), a file in an editor (`file.rs`), the tabs
//! and code sent to the thread (`tabs.rs`), a comment on lines of it (`comment.rs`), the pane drawn (`view.rs`), the tabs and folds kept
//! between starts (`persistence.rs`).

pub mod comment;
pub mod doc;
pub mod file;
pub mod item;
pub mod persistence;
pub mod tabs;
pub mod view;
