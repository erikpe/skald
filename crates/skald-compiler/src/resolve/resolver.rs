//! Two-pass top-level/member collection and callable-body name resolution.

use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{
        ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId, FunctionId,
        InitializerId, InterfaceId, MethodId,
    },
    module::ModuleGraph,
    source::Span,
    syntax,
};

use super::ir::*;

mod array_types;
mod body;
mod external_links;
mod imports;
mod name_lookup;
mod program;

use array_types::ArrayTypeInterner;
use name_lookup::{ModuleLookup, TopLevelLookup};

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
pub const MODULE_CONTEXT_REQUIRED: &str = "RES023";
pub const DUPLICATE_MODULE_BINDING: &str = "RES024";
pub const UNKNOWN_MODULE_BINDING: &str = "RES025";
pub const PRIVATE_DECLARATION: &str = "RES026";
pub const UNKNOWN_QUALIFIED_DECLARATION: &str = "RES027";
pub const DUPLICATE_ORDINARY_BINDING: &str = "RES028";
pub const UNKNOWN_IMPORTED_DECLARATION: &str = "RES029";
pub const INCOMPATIBLE_EXTERNAL_ABI: &str = "RES030";

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
    program::resolve_singleton(ast)
}

/// Resolves every reachable module in a loaded graph into one flat program.
///
/// Direct module imports create exact qualified bindings. Selective imports
/// create explicit ordinary bindings to public declarations owned by their
/// canonical source modules.
pub fn resolve_module_graph(graph: &ModuleGraph) -> ResolveOutput {
    program::resolve_graph(graph)
}

fn resolve_type(
    type_syntax: &syntax::TypeSyntax,
    lookup: ModuleLookup<'_>,
    array_types: &mut ArrayTypeInterner,
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
        } => ResolvedTypeKind::Shared(resolve_shared_target(
            target,
            lookup,
            array_types,
            diagnostics,
            false,
        )?),
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
                syntax::OptionalPayloadKind::Named(name) => {
                    match lookup.select(name, diagnostics) {
                        TopLevelLookup::Found(TopLevelSymbol {
                            kind: TopLevelSymbolKind::Class(class),
                            ..
                        }) => ResolvedOptionalPayload::Class(class),
                        TopLevelLookup::Found(TopLevelSymbol {
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
                        TopLevelLookup::Found(symbol) => {
                            diagnostics.push(
                                Diagnostic::error(
                                    UNKNOWN_TYPE,
                                    format!(
                                        "`{}` does not name an optional payload type",
                                        name.text
                                    ),
                                )
                                .with_primary_label(name.span, "expected a concrete class")
                                .with_secondary_label(symbol.name_span, "function declared here"),
                            );
                            return None;
                        }
                        TopLevelLookup::Missing => {
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
                        TopLevelLookup::Diagnosed => return None,
                    }
                }
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
        } => ResolvedTypeKind::OptionalShared {
            target: resolve_shared_target(target, lookup, array_types, diagnostics, true)?,
            shared_span: *shared_span,
            question_span: *question_span,
            target_span: target.span,
        },
        syntax::TypeKind::Grouped { inner, .. } => {
            return resolve_type(inner, lookup, array_types, diagnostics).map(|mut resolved| {
                resolved.span = type_syntax.span;
                resolved
            });
        }
        syntax::TypeKind::Array { element, .. } => {
            let element = resolve_type(element, lookup, array_types, diagnostics)?;
            ResolvedTypeKind::Array(array_types.intern(element))
        }
        syntax::TypeKind::Named(name) if !name.is_qualified() && name.text == "Obj" => {
            ResolvedTypeKind::Obj
        }
        syntax::TypeKind::Named(name) => match lookup.select(name, diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => ResolvedTypeKind::Class(class),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                ..
            }) => ResolvedTypeKind::Interface(interface),
            TopLevelLookup::Found(symbol) => {
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
            TopLevelLookup::Missing => {
                diagnostics.push(
                    Diagnostic::error(UNKNOWN_TYPE, format!("unknown type `{}`", name.text))
                        .with_primary_label(name.span, "no class with this name is declared"),
                );
                return None;
            }
            TopLevelLookup::Diagnosed => return None,
        },
    };
    Some(ResolvedType {
        kind,
        span: type_syntax.span,
    })
}

fn resolve_shared_target(
    target: &syntax::TypeSyntax,
    lookup: ModuleLookup<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
    optional: bool,
) -> Option<ResolvedSharedTarget> {
    if let syntax::TypeKind::Grouped { inner, .. } = &target.kind {
        return resolve_shared_target(inner, lookup, array_types, diagnostics, optional);
    }
    if matches!(target.kind, syntax::TypeKind::Array { .. }) {
        let resolved = resolve_type(target, lookup, array_types, diagnostics)?;
        let ResolvedTypeKind::Array(array) = resolved.kind else {
            unreachable!("an array target must resolve to an array identity")
        };
        return Some(ResolvedSharedTarget::Array(array));
    }
    let syntax::TypeKind::Named(target) = &target.kind else {
        diagnostics.push(
            Diagnostic::error(UNKNOWN_TYPE, "shared ownership requires an object target")
                .with_primary_label(
                    target.span,
                    "expected a class, interface, `Obj`, or array type",
                ),
        );
        return None;
    };
    if !target.is_qualified() && target.text == "Obj" {
        return Some(ResolvedSharedTarget::Obj);
    }
    match lookup.select(target, diagnostics) {
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(class),
            ..
        }) => Some(ResolvedSharedTarget::Class(class)),
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::Interface(interface),
            ..
        }) => Some(ResolvedSharedTarget::Interface(interface)),
        TopLevelLookup::Found(symbol) => {
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
        TopLevelLookup::Missing => {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_TYPE,
                    if optional {
                        format!("unknown optional shared target `{}`", target.text)
                    } else {
                        format!("unknown shared target `{}`", target.text)
                    },
                )
                .with_primary_label(
                    target.span,
                    "no class or interface with this name is declared",
                ),
            );
            None
        }
        TopLevelLookup::Diagnosed => None,
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
