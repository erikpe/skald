//! Target-independent final-MIR optimization implementations.

// Checked-integer folding remains private until its complete pass is registered.
#[allow(dead_code)]
mod checked_integer_evaluation;
#[allow(dead_code)]
mod checked_integer_protocol;
#[allow(dead_code)]
mod checked_integer_rewrite;
pub(in crate::passes::pipeline) mod conservative_cfg_cleanup;
pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
mod primitive_algebra;
pub(in crate::passes::pipeline) mod primitive_algebraic_simplification;
pub(in crate::passes::pipeline) mod primitive_constant_folding;
mod primitive_evaluation;
mod primitive_facts;
pub(in crate::passes::pipeline) mod whole_world_reachability;
