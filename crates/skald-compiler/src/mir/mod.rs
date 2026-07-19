//! Target-independent mid-level IR.
//!
//! MIR will make evaluation order, control flow, temporaries, calls, and
//! cleanup explicit. The initial form need not be SSA, but must leave room for
//! an SSA representation or conversion pass later.
