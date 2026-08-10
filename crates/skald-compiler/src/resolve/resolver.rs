//! Two-pass top-level/member collection and callable-body name resolution.

use std::collections::HashMap;
use std::path::Path;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{
        ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId, FunctionId,
        InitializerId, InterfaceId, MethodId, StaticFieldId, StaticInitializerId,
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
pub const PRIVATE_MEMBER_ACCESS: &str = "RES031";
pub const MISSING_STRING_LANGUAGE_ITEM: &str = "RES032";
pub const INVALID_STRING_LANGUAGE_ITEM: &str = "RES033";
pub const INVALID_INTRINSIC_DECLARATION: &str = "RES034";
pub const LOOP_EXIT_OUTSIDE_LOOP: &str = "RES035";

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
    resolve_with_source_path(ast, Path::new("main.ska"))
}

pub(crate) fn resolve_with_source_path(
    ast: &syntax::CompilationUnit,
    source_path: &Path,
) -> ResolveOutput {
    program::resolve_singleton(ast, source_path)
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
            question_span,
            spelling,
        } => resolve_optional_type(
            payload,
            *question_span,
            *spelling,
            lookup,
            array_types,
            diagnostics,
        )?,
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

fn resolve_optional_type(
    payload_syntax: &syntax::TypeSyntax,
    question_span: Span,
    spelling: syntax::OptionalTypeSpelling,
    lookup: ModuleLookup<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedTypeKind> {
    let boxed_question_span = if spelling == syntax::OptionalTypeSpelling::SharedShorthand {
        shared_target_syntax(payload_syntax).and_then(optional_question_span)
    } else {
        None
    };
    if let Some(boxed_question_span) = boxed_question_span {
        diagnostics.push(
            Diagnostic::error(
                INVALID_OPTIONAL_TYPE,
                "optional shared boxes are not supported",
            )
            .with_primary_label(
                boxed_question_span,
                "`shared? T?` requires a shared box containing an optional payload",
            ),
        );
        return None;
    }

    let payload = resolve_type(payload_syntax, lookup, array_types, diagnostics)?;
    let payload_span = payload_syntax.span;
    if let Some(payload) = flat_optional_payload(payload.kind) {
        return Some(ResolvedTypeKind::Optional {
            payload,
            payload_span,
            question_span,
        });
    }
    match payload.kind {
        ResolvedTypeKind::Shared(target) => {
            let (shared_span, target_span) = shared_syntax_parts(payload_syntax)
                .expect("resolved shared payload must retain shared source syntax");
            Some(ResolvedTypeKind::OptionalShared {
                target,
                shared_span,
                question_span,
                target_span,
            })
        }
        ResolvedTypeKind::Interface(_) => {
            let name = match &ungroup_type(payload_syntax).kind {
                syntax::TypeKind::Named(name) => name.text.as_str(),
                _ => "interface",
            };
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPTIONAL_TYPE,
                    format!("interface `{name}` cannot be an inline optional payload"),
                )
                .with_primary_label(
                    payload_span,
                    "use `(shared Interface)?` for an optional owning view",
                ),
            );
            None
        }
        ResolvedTypeKind::Obj => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPTIONAL_TYPE,
                    "`Obj?` is not a valid inline optional type",
                )
                .with_primary_label(
                    question_span,
                    "use `(shared Obj)?` for an optional owning object view",
                ),
            );
            None
        }
        ResolvedTypeKind::Unit => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPTIONAL_TYPE,
                    "`unit?` is not a valid optional type",
                )
                .with_primary_label(
                    question_span,
                    "`unit` has no value payload to make optional",
                ),
            );
            None
        }
        ResolvedTypeKind::Array(_) => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPTIONAL_TYPE,
                    "inline optional array payloads are not supported yet",
                )
                .with_primary_label(
                    question_span,
                    "this syntax is reserved for the optional-array implementation",
                ),
            );
            None
        }
        ResolvedTypeKind::Optional { .. } | ResolvedTypeKind::OptionalShared { .. } => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OPTIONAL_TYPE,
                    "nested optional types are not supported yet",
                )
                .with_primary_label(
                    question_span,
                    "this outer optional layer is reserved for recursive optional identities",
                ),
            );
            None
        }
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
        | ResolvedTypeKind::Class(_) => {
            unreachable!("flat optional payloads returned before deferred validation")
        }
    }
}

const fn flat_optional_payload(kind: ResolvedTypeKind) -> Option<ResolvedOptionalPayload> {
    match kind {
        ResolvedTypeKind::I64 => Some(ResolvedOptionalPayload::I64),
        ResolvedTypeKind::U64 => Some(ResolvedOptionalPayload::U64),
        ResolvedTypeKind::U8 => Some(ResolvedOptionalPayload::U8),
        ResolvedTypeKind::F64 => Some(ResolvedOptionalPayload::F64),
        ResolvedTypeKind::Bool => Some(ResolvedOptionalPayload::Bool),
        ResolvedTypeKind::Class(class) => Some(ResolvedOptionalPayload::Class(class)),
        _ => None,
    }
}

fn ungroup_type(mut type_syntax: &syntax::TypeSyntax) -> &syntax::TypeSyntax {
    while let syntax::TypeKind::Grouped { inner, .. } = &type_syntax.kind {
        type_syntax = inner;
    }
    type_syntax
}

fn shared_target_syntax(type_syntax: &syntax::TypeSyntax) -> Option<&syntax::TypeSyntax> {
    match &ungroup_type(type_syntax).kind {
        syntax::TypeKind::Shared { target, .. } => Some(target),
        _ => None,
    }
}

fn optional_question_span(type_syntax: &syntax::TypeSyntax) -> Option<Span> {
    match &ungroup_type(type_syntax).kind {
        syntax::TypeKind::Optional { question_span, .. } => Some(*question_span),
        _ => None,
    }
}

fn shared_syntax_parts(type_syntax: &syntax::TypeSyntax) -> Option<(Span, Span)> {
    match &ungroup_type(type_syntax).kind {
        syntax::TypeKind::Shared {
            shared_span,
            target,
        } => Some((*shared_span, target.span)),
        _ => None,
    }
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
    if let syntax::TypeKind::Optional { question_span, .. } = &target.kind {
        diagnostics.push(
            Diagnostic::error(
                INVALID_OPTIONAL_TYPE,
                "shared boxes containing optional payloads are not supported",
            )
            .with_primary_label(
                *question_span,
                "`shared T?` is reserved for a future shared-box design",
            ),
        );
        return None;
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
    StaticField(StaticFieldId),
    Method(MethodId),
}

#[derive(Clone, Debug, Default)]
pub(super) struct ClassSymbols {
    pub(super) ordinary: HashMap<String, OrdinaryMemberSymbol>,
    pub(super) copy_constructor_span: Option<Span>,
    pub(super) copy_assignment_span: Option<Span>,
    pub(super) destructor_span: Option<Span>,
}
