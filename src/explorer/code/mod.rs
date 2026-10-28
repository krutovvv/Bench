//! What the editor knows of the code: the language servers' problems, hover and definitions
//! (`lsp.rs`), the Problems tab (`problems.rs`), lines changed since HEAD and a file's diff
//! (`git.rs`), a file's language (`lang.rs`).

pub mod git;
pub mod lang;
pub mod lsp;
pub mod problems;
