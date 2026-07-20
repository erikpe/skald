//! Two-pass top-level/member collection and callable-body name resolution.

use std::collections::HashMap;

use crate::{
    diagnostics::Diagnostics,
    identity::{ClassId, FieldId, FunctionId, InitializerId, MethodId},
    source::Span,
    syntax,
};

use super::ir::*;

mod body;
mod program;

pub const DUPLICATE_TOP_LEVEL: &str = "RES001";
pub const DUPLICATE_BINDING: &str = "RES002";
pub const UNKNOWN_NAME: &str = "RES003";
pub const INVALID_CALL_TARGET: &str = "RES004";
pub const TOP_LEVEL_USED_AS_VALUE: &str = "RES005";
pub const DUPLICATE_MEMBER: &str = "RES006";
pub const UNKNOWN_TYPE: &str = "RES007";
pub const UNKNOWN_MEMBER: &str = "RES008";
pub const INVALID_MEMBER_SELECTION: &str = "RES009";
pub const SELF_OUTSIDE_MEMBER: &str = "RES010";
pub const INVALID_CONSTRUCTION_TARGET: &str = "RES011";

#[derive(Debug)]
pub struct ResolveOutput {
    pub program: ResolvedProgram,
    pub diagnostics: Diagnostics,
}

impl ResolveOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

/// Resolves a parsed single-file compilation unit.
///
/// Declaration collection precedes every body, allowing forward references
/// while ensuring that all successful uses below this boundary carry stable
/// identities rather than source names.
pub fn resolve(ast: &syntax::CompilationUnit) -> ResolveOutput {
    program::ProgramResolver::new(ast).resolve()
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TopLevelSymbol {
    pub(super) kind: TopLevelSymbolKind,
    pub(super) name_span: Span,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TopLevelSymbolKind {
    Function(FunctionId),
    Class(ClassId),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OrdinaryMemberSymbol {
    pub(super) kind: OrdinaryMemberSymbolKind,
    pub(super) name_span: Span,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum OrdinaryMemberSymbolKind {
    Field(FieldId),
    Method(MethodId),
}

#[derive(Clone, Debug, Default)]
pub(super) struct ClassSymbols {
    pub(super) ordinary: HashMap<String, OrdinaryMemberSymbol>,
    pub(super) initializer: Option<InitializerId>,
    pub(super) initializer_span: Option<Span>,
}
