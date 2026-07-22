//! Repository-local Markdown link and index validation.

mod checker;
mod markdown;

pub use checker::{check_repository, Diagnostic};

#[cfg(test)]
mod tests;
