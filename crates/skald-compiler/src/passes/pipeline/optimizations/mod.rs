//! Target-independent final-MIR optimization implementations.

pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
pub(in crate::passes::pipeline) mod primitive_constant_folding;
mod primitive_evaluation;
mod primitive_facts;
pub(in crate::passes::pipeline) mod whole_world_reachability;
