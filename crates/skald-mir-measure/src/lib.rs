//! Opt-in repository measurement of local final-MIR redundancy.
//!
//! The tool validates a versioned corpus, compiles each unique whole-world
//! root through the ordinary driver, and projects borrowed verified pipeline
//! checkpoints into deterministic structural reports. It is not a compiler
//! pass and adds no cost to ordinary compilation.

mod aggregate;
mod cli;
mod corpus;
mod digest;
mod measure;
mod model;
mod projection;
mod render;
mod revision;

pub use cli::run_cli;
pub use corpus::{load_corpus, Corpus, CorpusError, Workload};
pub use measure::{measure_corpus, MeasurementError, MeasurementOptions};
pub use model::{MeasurementReport, ReportFormat};
pub use render::render_report;
