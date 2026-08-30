//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

pub mod static_lifecycle;

mod graph;
mod pipeline;

pub use pipeline::{run_mir_pipeline, verify_final_mir, VerifiedFinalMirProgram};
pub(crate) use pipeline::{run_mir_pipeline_measured, MeasuredMirPipeline, MirPipelineStatistics};
