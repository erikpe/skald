//! Target-independent final-MIR optimization implementations.

pub(in crate::passes::pipeline) mod dead_pure_definition_elimination;
// LSR0 deliberately establishes this semantic owner before LSR1 gives it a
// production caller.
#[allow(dead_code)]
mod primitive_evaluation;
pub(in crate::passes::pipeline) mod whole_world_reachability;
