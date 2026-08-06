//! Exact-byte loading, Unix argument decoding, and observation comparison.

mod compare;
mod error;
mod load;
mod model;

pub use compare::{compare_exit, compare_stream};
pub use error::ExpectationError;
pub use load::{decode_arguments, load_bytes};
pub use model::{StreamMatch, StreamMismatch};
