//! Two-pass top-level/member collection and callable-body name resolution.

use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{
        ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId, FunctionId,
        InitializerId, InterfaceId, MethodId,
    },
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
pub const INVALID_LIFECYCLE_SIGNATURE: &str = "RES012";
pub const INVALID_BASE_CLASS: &str = "RES013";
pub const INHERITANCE_CYCLE: &str = "RES014";
pub const INHERITED_MEMBER_COLLISION: &str = "RES015";
pub const INVALID_BASE_INITIALIZATION: &str = "RES016";
pub const INVALID_OVERRIDE: &str = "RES017";
pub const INVALID_INTERFACE_CLAIM: &str = "RES018";

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

fn resolve_type(
    type_syntax: &syntax::TypeSyntax,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedType> {
    let kind = match &type_syntax.kind {
        syntax::TypeKind::I64 => ResolvedTypeKind::I64,
        syntax::TypeKind::U64 => ResolvedTypeKind::U64,
        syntax::TypeKind::U8 => ResolvedTypeKind::U8,
        syntax::TypeKind::F64 => ResolvedTypeKind::F64,
        syntax::TypeKind::Bool => ResolvedTypeKind::Bool,
        syntax::TypeKind::Unit => ResolvedTypeKind::Unit,
        syntax::TypeKind::Shared {
            shared_span: _,
            target,
        } => {
            let target_kind = if target.text == "Obj" {
                ResolvedSharedTarget::Obj
            } else {
                match top_levels.get(&target.text) {
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Class(class),
                        ..
                    }) => ResolvedSharedTarget::Class(*class),
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Interface(interface),
                        ..
                    }) => ResolvedSharedTarget::Interface(*interface),
                    Some(symbol) => {
                        diagnostics.push(
                            Diagnostic::error(
                                UNKNOWN_TYPE,
                                format!("`{}` does not name a shared object type", target.text),
                            )
                            .with_primary_label(
                                target.span,
                                "expected a class, interface, or `Obj`",
                            )
                            .with_secondary_label(symbol.name_span, "function declared here"),
                        );
                        return None;
                    }
                    None => {
                        diagnostics.push(
                            Diagnostic::error(
                                UNKNOWN_TYPE,
                                format!("unknown shared target `{}`", target.text),
                            )
                            .with_primary_label(
                                target.span,
                                "no class or interface with this name is declared",
                            ),
                        );
                        return None;
                    }
                }
            };
            ResolvedTypeKind::Shared(target_kind)
        }
        syntax::TypeKind::Named(name) if name.text == "Obj" => ResolvedTypeKind::Obj,
        syntax::TypeKind::Named(name) => match top_levels.get(&name.text) {
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => ResolvedTypeKind::Class(*class),
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                ..
            }) => ResolvedTypeKind::Interface(*interface),
            Some(symbol) => {
                diagnostics.push(
                    Diagnostic::error(
                        UNKNOWN_TYPE,
                        format!("`{}` does not name a type", name.text),
                    )
                    .with_primary_label(name.span, "expected a class or interface type")
                    .with_secondary_label(symbol.name_span, "function declared here"),
                );
                return None;
            }
            None => {
                diagnostics.push(
                    Diagnostic::error(UNKNOWN_TYPE, format!("unknown type `{}`", name.text))
                        .with_primary_label(name.span, "no class with this name is declared"),
                );
                return None;
            }
        },
    };
    Some(ResolvedType {
        kind,
        span: type_syntax.span,
    })
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
    Interface(InterfaceId),
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

impl OrdinaryMemberSymbolKind {
    pub(super) const fn declaring_class(self) -> ClassId {
        match self {
            Self::Field(field) => field.class(),
            Self::Method(method) => method.class(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ClassSymbols {
    pub(super) ordinary: HashMap<String, OrdinaryMemberSymbol>,
    pub(super) copy_constructor_span: Option<Span>,
    pub(super) copy_assignment_span: Option<Span>,
    pub(super) destructor_span: Option<Span>,
}
