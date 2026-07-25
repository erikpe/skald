//! Shared-owner verification.
//!
//! Structural checks validate each instruction and declaration in isolation.
//! Ownership analysis then verifies allocation, owner, checked-view, and anchor
//! state across every control-flow path.

mod ownership;
mod structural;
