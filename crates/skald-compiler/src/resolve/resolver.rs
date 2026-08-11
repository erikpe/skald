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

mod body;
mod external_links;
mod imports;
mod name_lookup;
mod program;
mod type_interner;

use name_lookup::{ModuleLookup, TopLevelLookup};
use type_interner::ResolvedTypeInterner;

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
    type_interner: &mut ResolvedTypeInterner,
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
            type_interner,
            diagnostics,
        )?),
        syntax::TypeKind::Optional { payload, .. } => {
            resolve_optional_type(payload, lookup, type_interner, diagnostics)?
        }
        syntax::TypeKind::Grouped { inner, .. } => {
            return resolve_type(inner, lookup, type_interner, diagnostics).map(|mut resolved| {
                resolved.span = type_syntax.span;
                resolved
            });
        }
        syntax::TypeKind::Array { element, .. } => {
            let element = resolve_type(element, lookup, type_interner, diagnostics)?;
            ResolvedTypeKind::Array(type_interner.intern_array(element))
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
    lookup: ModuleLookup<'_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedTypeKind> {
    let payload = resolve_type(payload_syntax, lookup, type_interner, diagnostics)?;
    let optional = type_interner.intern_optional(payload.clone());
    Some(ResolvedTypeKind::Optional(optional))
}

fn resolve_shared_target(
    target: &syntax::TypeSyntax,
    lookup: ModuleLookup<'_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedSharedTarget> {
    if syntax_type_is_optional(target) {
        let (optional_depth, leaf_syntax) = optional_syntax_leaf(target)
            .expect("an optional shared target must have an optional syntax leaf");
        let leaf = resolve_type(leaf_syntax, lookup, type_interner, diagnostics)?;
        let object_leaf = match leaf.kind {
            ResolvedTypeKind::Obj => Some(ResolvedObjectTarget::Obj),
            ResolvedTypeKind::Class(class) => Some(ResolvedObjectTarget::Class(class)),
            ResolvedTypeKind::Interface(interface) => {
                Some(ResolvedObjectTarget::Interface(interface))
            }
            _ => None,
        };
        if matches!(
            object_leaf,
            Some(ResolvedObjectTarget::Obj | ResolvedObjectTarget::Interface(_))
        ) {
            let target = type_interner.intern_optional_object_box_view(
                optional_depth,
                object_leaf.expect("matched object view"),
                target.span,
            );
            return Some(ResolvedSharedTarget::OptionalBox(target));
        }
        let resolved = resolve_type(target, lookup, type_interner, diagnostics)?;
        let ResolvedTypeKind::Optional(optional) = resolved.kind else {
            unreachable!("an optional target must resolve to an optional identity")
        };
        let target = type_interner.intern_optional_box(optional, target.span);
        return Some(ResolvedSharedTarget::OptionalBox(target));
    }
    if let syntax::TypeKind::Grouped { inner, .. } = &target.kind {
        return resolve_shared_target(inner, lookup, type_interner, diagnostics);
    }
    if matches!(target.kind, syntax::TypeKind::Array { .. }) {
        let resolved = resolve_type(target, lookup, type_interner, diagnostics)?;
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
                    format!("unknown shared target `{}`", target.text),
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

fn optional_syntax_leaf(mut target: &syntax::TypeSyntax) -> Option<(usize, &syntax::TypeSyntax)> {
    let mut depth = 0usize;
    loop {
        match &target.kind {
            syntax::TypeKind::Grouped { inner, .. } => target = inner,
            syntax::TypeKind::Optional { payload, .. } => {
                depth += 1;
                target = payload;
            }
            _ => return (depth > 0).then_some((depth, target)),
        }
    }
}

fn syntax_type_is_optional(target: &syntax::TypeSyntax) -> bool {
    match &target.kind {
        syntax::TypeKind::Optional { .. } => true,
        syntax::TypeKind::Grouped { inner, .. } => syntax_type_is_optional(inner),
        _ => false,
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
