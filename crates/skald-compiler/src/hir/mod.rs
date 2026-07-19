//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

pub use dump::dump_hir;
pub use ir::{
    HirBinaryOperation, HirBlock, HirExpression, HirExpressionKind, HirFunction, HirFunctionTable,
    HirLocal, HirLocalDecl, HirParameter, HirProgram, HirReturn, HirStatement, HirUnaryOperation,
    Type,
};
