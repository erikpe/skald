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
pub const INVALID_DEREFERENCE: &str = "RES019";
pub const INVALID_POINTEE_ASSIGNMENT: &str = "RES020";
pub const IMPLICIT_SHARED_DEREFERENCE: &str = "RES021";
pub const INVALID_OPTIONAL_TYPE: &str = "RES022";
pub const UNSUPPORTED_ARRAY_SYNTAX: &str = "RES023";

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
    if type_contains_array(type_syntax) {
        diagnostics.push(unsupported_array_diagnostic(type_syntax.span));
        return None;
    }
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
            let target = plain_shared_target(target, diagnostics)?;
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
        syntax::TypeKind::Optional {
            payload,
            payload_span,
            question_span,
        } => {
            let payload = match payload {
                syntax::OptionalPayloadKind::I64 => ResolvedOptionalPayload::I64,
                syntax::OptionalPayloadKind::U64 => ResolvedOptionalPayload::U64,
                syntax::OptionalPayloadKind::U8 => ResolvedOptionalPayload::U8,
                syntax::OptionalPayloadKind::F64 => ResolvedOptionalPayload::F64,
                syntax::OptionalPayloadKind::Bool => ResolvedOptionalPayload::Bool,
                syntax::OptionalPayloadKind::Named(name) => match top_levels.get(&name.text) {
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Class(class),
                        ..
                    }) => ResolvedOptionalPayload::Class(*class),
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Interface(_),
                        ..
                    }) => {
                        diagnostics.push(
                            Diagnostic::error(
                                INVALID_OPTIONAL_TYPE,
                                format!(
                                    "interface `{}` cannot be an inline optional payload",
                                    name.text
                                ),
                            )
                            .with_primary_label(
                                type_syntax.span,
                                "use `shared? Interface` for an optional owning view",
                            ),
                        );
                        return None;
                    }
                    Some(symbol) => {
                        diagnostics.push(
                            Diagnostic::error(
                                UNKNOWN_TYPE,
                                format!("`{}` does not name an optional payload type", name.text),
                            )
                            .with_primary_label(name.span, "expected a concrete class")
                            .with_secondary_label(symbol.name_span, "function declared here"),
                        );
                        return None;
                    }
                    None => {
                        diagnostics.push(
                            Diagnostic::error(
                                UNKNOWN_TYPE,
                                format!("unknown optional payload type `{}`", name.text),
                            )
                            .with_primary_label(
                                name.span,
                                "no concrete class with this name is declared",
                            ),
                        );
                        return None;
                    }
                },
            };
            ResolvedTypeKind::Optional {
                payload,
                payload_span: *payload_span,
                question_span: *question_span,
            }
        }
        syntax::TypeKind::OptionalShared {
            shared_span,
            question_span,
            target,
        } => {
            let target = plain_shared_target(target, diagnostics)?;
            ResolvedTypeKind::OptionalShared {
                target: resolve_optional_shared_target(target, top_levels, diagnostics)?,
                shared_span: *shared_span,
                question_span: *question_span,
                target_span: target.span,
            }
        }
        syntax::TypeKind::Grouped { inner, .. } => {
            return resolve_type(inner, top_levels, diagnostics).map(|mut resolved| {
                resolved.span = type_syntax.span;
                resolved
            });
        }
        syntax::TypeKind::Array { .. } => {
            unreachable!("array types are rejected at the resolution gate")
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

fn unsupported_array_diagnostic(span: Span) -> Diagnostic {
    Diagnostic::error(
        UNSUPPORTED_ARRAY_SYNTAX,
        "array semantics are not implemented yet",
    )
    .with_primary_label(
        span,
        "array syntax is accepted, but resolution support is pending",
    )
}

fn type_contains_array(type_syntax: &syntax::TypeSyntax) -> bool {
    match &type_syntax.kind {
        syntax::TypeKind::Array { .. } => true,
        syntax::TypeKind::Shared { target, .. }
        | syntax::TypeKind::OptionalShared { target, .. } => type_contains_array(target),
        syntax::TypeKind::Grouped { inner, .. } => type_contains_array(inner),
        _ => false,
    }
}

fn plain_shared_target<'a>(
    target: &'a syntax::TypeSyntax,
    diagnostics: &mut Diagnostics,
) -> Option<&'a syntax::Name> {
    if let syntax::TypeKind::Named(name) = &target.kind {
        return Some(name);
    }
    diagnostics.push(
        Diagnostic::error(UNKNOWN_TYPE, "shared ownership requires an object target")
            .with_primary_label(target.span, "expected a class, interface, or `Obj`"),
    );
    None
}

fn resolve_optional_shared_target(
    target: &syntax::Name,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedSharedTarget> {
    if target.text == "Obj" {
        return Some(ResolvedSharedTarget::Obj);
    }
    match top_levels.get(&target.text) {
        Some(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(class),
            ..
        }) => Some(ResolvedSharedTarget::Class(*class)),
        Some(TopLevelSymbol {
            kind: TopLevelSymbolKind::Interface(interface),
            ..
        }) => Some(ResolvedSharedTarget::Interface(*interface)),
        Some(symbol) => {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_TYPE,
                    format!("`{}` does not name a shared object type", target.text),
                )
                .with_primary_label(target.span, "expected a class, interface, or `Obj`")
                .with_secondary_label(symbol.name_span, "function declared here"),
            );
            None
        }
        None => {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_TYPE,
                    format!("unknown optional shared target `{}`", target.text),
                )
                .with_primary_label(
                    target.span,
                    "no class or interface with this name is declared",
                ),
            );
            None
        }
    }
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
