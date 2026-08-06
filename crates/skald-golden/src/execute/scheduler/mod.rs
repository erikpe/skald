//! Bounded dependency-graph scheduling and canonical result assembly.

mod coordinator;
mod graph;
mod outcome;

pub use coordinator::execute_parallel;

#[cfg(test)]
mod tests;
