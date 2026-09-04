//! Target-independent final-MIR optimization implementations.

// The checked protocol analysis consumes this evaluator in the next staged
// roadmap task. Keep it private and independently tested until that owner lands.
#[allow(dead_code)]
mod checked_integer_evaluation;
pub(in crate::passes::pipeline) mod conservative_cfg_cleanup;
pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
mod primitive_algebra;
pub(in crate::passes::pipeline) mod primitive_algebraic_simplification;
pub(in crate::passes::pipeline) mod primitive_constant_folding;
mod primitive_evaluation;
mod primitive_facts;
pub(in crate::passes::pipeline) mod whole_world_reachability;
