//! Exact-byte loading, Unix argument decoding, and observation comparison.

mod compare;
mod error;
mod load;
mod model;

pub use compare::{compare_exit, compare_matchers, compare_stream};
pub use error::ExpectationError;
pub use load::{decode_arguments, load_bytes};
pub use model::{
    EmptyStreamMatcherSet, MatcherLoadFailure, MatcherMatch, MatcherMismatch, MatcherOutcome,
    StreamComparison, StreamMatch, StreamMatcher, StreamMatcherSet, StreamMismatch,
};

#[cfg(test)]
mod tests;
