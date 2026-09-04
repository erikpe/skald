//! Target-independent final-MIR optimization implementations.

mod checked_integer_evaluation;
pub(in crate::passes::pipeline) mod checked_integer_folding;
mod checked_integer_protocol;
mod checked_integer_rewrite;
pub(in crate::passes::pipeline) mod conservative_cfg_cleanup;
pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
mod primitive_algebra;
pub(in crate::passes::pipeline) mod primitive_algebraic_simplification;
pub(in crate::passes::pipeline) mod primitive_constant_folding;
mod primitive_evaluation;
mod primitive_facts;
pub(in crate::passes::pipeline) mod whole_world_reachability;
