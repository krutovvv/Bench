//! What the editor knows of the code: the language servers' problems, hover and definitions
//! (`lsp.rs`), the Problems tab (`problems.rs`), lines changed since HEAD and a file's diff
//! (`git.rs`), what a thread's turn changed in a file (`turn_diff.rs`), a file's language (`lang.rs`).

pub mod git;
pub mod lang;
pub mod lsp;
pub mod problems;
pub mod turn_diff;
