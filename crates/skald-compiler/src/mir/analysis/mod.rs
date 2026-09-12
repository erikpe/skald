//! Short-lived, callable-local MIR analysis facts.
//!
//! This private facade owns representation-neutral queries which are useful to
//! more than one MIR responsibility. Analyses describe one immutable snapshot
//! and must be rebuilt after mutation.

mod cfg;
mod dominators;

pub(crate) use cfg::{MirCfgBlockTopology, MirCfgEdge, MirCfgTopology};
pub(crate) use dominators::MirDominators;
