//! Structured diagnostics shared by pipeline phases.
//!
//! User errors are reported as diagnostics; they are not represented by Rust
//! panics. Rendering is kept separate from diagnostic construction.
