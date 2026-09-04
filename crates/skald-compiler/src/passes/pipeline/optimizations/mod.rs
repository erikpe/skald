//! Target-independent final-MIR optimization implementations.

mod checked_integer_evaluation;
pub(in crate::passes::pipeline) mod checked_integer_folding;
mod checked_integer_protocol;
mod checked_integer_rewrite;
pub(in crate::passes::pipeline) mod conservative_cfg_cleanup;
pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
pub(in crate::passes::pipeline) mod post_proof_unreachable_block_elimination;
mod primitive_algebra;
pub(in crate::passes::pipeline) mod primitive_algebraic_simplification;
pub(in crate::passes::pipeline) mod primitive_constant_folding;
mod primitive_evaluation;
mod primitive_facts;
pub(in crate::passes::pipeline) mod whole_world_reachability;

pub(in crate::passes) use checked_integer_evaluation::{
    evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation,
};
pub(in crate::passes) use primitive_evaluation::{
    evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation,
};
