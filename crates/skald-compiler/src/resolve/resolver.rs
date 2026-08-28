//! Two-pass top-level/member collection and callable-body name resolution.

use std::collections::HashMap;
use std::path::Path;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{
        ClassId, ClassTemplateId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId,
        FunctionId, InitializerId, InterfaceId, InterfaceTemplateId, MethodId, StaticFieldId,
        StaticInitializerId,
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
pub const DUPLICATE_TYPE_PARAMETER: &str = "RES037";
pub const INVALID_GENERIC_APPLICATION: &str = "RES038";
pub const RAW_GENERIC_TYPE: &str = "RES039";
pub const GENERIC_ARITY_MISMATCH: &str = "RES040";
pub const INVALID_GENERIC_BOUND: &str = "RES041";
pub const DUPLICATE_GENERIC_BOUND: &str = "RES042";
pub const INVALID_GENERIC_BASE: &str = "RES043";
pub const UNSUPPORTED_PARAMETER_CONSTRUCTION: &str = "RES044";
pub const UNCONSTRAINED_TYPE_PARAMETER_MEMBER: &str = "RES045";
pub const AMBIGUOUS_GENERIC_BOUND_MEMBER: &str = "RES046";
pub const NON_TERMINATING_GENERIC_SPECIALIZATION: &str = "RES047";
pub const UNSATISFIED_GENERIC_REQUIREMENT: &str = "RES048";
pub const INVALID_INDEX_PROTOCOL: &str = "RES049";
pub const INVALID_FUNCTION_REFERENCE: &str = "RES050";
pub const INVALID_GENERIC_INTERFACE_REQUIREMENT: &str = "RES052";
pub const INVALID_ITERABLE_LANGUAGE_ITEM: &str = "RES053";
pub const MISSING_ITERABLE_APPLICATION: &str = "RES054";
pub const AMBIGUOUS_ITERABLE_APPLICATION: &str = "RES055";
pub const ITERATION_ITEM_TYPE_MISMATCH: &str = "RES056";
pub const INVALID_OPERATOR_LANGUAGE_ITEM: &str = "RES057";
pub const UNSUPPORTED_GENERIC_OPERATOR_APPLICATION: &str = "RES058";
pub const AMBIGUOUS_GENERIC_OPERATOR_APPLICATION: &str = "RES059";
pub const INCOMPATIBLE_GENERIC_OPERATOR_RHS: &str = "RES060";
pub const INVALID_RANGE_LANGUAGE_ITEM: &str = "RES061";

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
    resolve_type_inner(type_syntax, lookup, type_interner, diagnostics)
}

fn resolve_type_inner(
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
        syntax::TypeKind::Function(function) => {
            let mut parameters = Vec::with_capacity(function.parameters.len());
            for parameter in &function.parameters {
                let type_syntax =
                    resolve_type_inner(&parameter.type_syntax, lookup, type_interner, diagnostics)?;
                let mode = match parameter.mode {
                    syntax::FunctionTypeParameterMode::Value => {
                        ResolvedFunctionTypeParameterMode::Value
                    }
                    syntax::FunctionTypeParameterMode::ReadOnlyAlias { .. } => {
                        ResolvedFunctionTypeParameterMode::ReadOnlyAlias
                    }
                    syntax::FunctionTypeParameterMode::MutableAlias { .. } => {
                        ResolvedFunctionTypeParameterMode::MutableAlias
                    }
                };
                parameters.push(ResolvedFunctionTypeParameter {
                    mode,
                    type_syntax,
                    span: parameter.span,
                });
            }
            let result = resolve_type_inner(&function.result, lookup, type_interner, diagnostics)?;
            let id = type_interner.intern_function(parameters, result, function.span);
            ResolvedTypeKind::Function(id)
        }
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
            return resolve_type_inner(inner, lookup, type_interner, diagnostics).map(
                |mut resolved| {
                    resolved.span = type_syntax.span;
                    resolved
                },
            );
        }
        syntax::TypeKind::Array { element, .. } => {
            let element = resolve_type_inner(element, lookup, type_interner, diagnostics)?;
            ResolvedTypeKind::Array(type_interner.intern_array(element))
        }
        syntax::TypeKind::Named(named) if named.arguments.is_some() => {
            if let Some(class) = lookup.specialized_class(named.span) {
                report_generic_application(named, lookup, diagnostics);
                ResolvedTypeKind::Class(class)
            } else if let Some(interface) = lookup.specialized_interface(named.span) {
                report_generic_application(named, lookup, diagnostics);
                ResolvedTypeKind::Interface(interface)
            } else {
                report_generic_application(named, lookup, diagnostics);
                return None;
            }
        }
        syntax::TypeKind::Named(named)
            if !named.name.is_qualified() && named.name.text == "Obj" =>
        {
            ResolvedTypeKind::Obj
        }
        syntax::TypeKind::Named(named) => match lookup.select(&named.name, diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => ResolvedTypeKind::Class(class),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                ..
            }) => ResolvedTypeKind::Interface(interface),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::ClassTemplate(_),
                name_span,
            }) => {
                diagnostics.push(
                    Diagnostic::error(
                        RAW_GENERIC_TYPE,
                        format!(
                            "generic class `{}` requires type arguments",
                            named.name.text
                        ),
                    )
                    .with_primary_label(named.name.span, "type arguments cannot be omitted")
                    .with_secondary_label(name_span, "template declared here"),
                );
                return None;
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::InterfaceTemplate(_),
                name_span,
            }) => {
                diagnostics.push(
                    Diagnostic::error(
                        RAW_GENERIC_TYPE,
                        format!(
                            "generic interface `{}` requires type arguments",
                            named.name.text
                        ),
                    )
                    .with_primary_label(named.name.span, "type arguments cannot be omitted")
                    .with_secondary_label(name_span, "template declared here"),
                );
                return None;
            }
            TopLevelLookup::Found(symbol) => {
                diagnostics.push(
                    Diagnostic::error(
                        UNKNOWN_TYPE,
                        format!("`{}` does not name a type", named.name.text),
                    )
                    .with_primary_label(named.name.span, "expected a class or interface type")
                    .with_secondary_label(symbol.name_span, "function declared here"),
                );
                return None;
            }
            TopLevelLookup::Missing => {
                diagnostics.push(
                    Diagnostic::error(UNKNOWN_TYPE, format!("unknown type `{}`", named.name.text))
                        .with_primary_label(named.name.span, "no class with this name is declared"),
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

fn report_generic_application(
    named: &syntax::NamedTypeSyntax,
    lookup: ModuleLookup<'_>,
    diagnostics: &mut Diagnostics,
) {
    let arguments = named
        .arguments
        .as_ref()
        .expect("generic application reporting requires arguments");
    if !named.name.is_qualified() && named.name.text == "Obj" {
        diagnostics.push(
            Diagnostic::error(INVALID_GENERIC_APPLICATION, "`Obj` is not a generic class")
                .with_primary_label(arguments.span, "type arguments are not allowed here"),
        );
        return;
    }
    match lookup.select(&named.name, diagnostics) {
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::ClassTemplate(template),
            name_span,
        }) => {
            let expected = lookup.template_arity(template);
            let actual = arguments.arguments.len();
            if expected != actual {
                diagnostics.push(
                    Diagnostic::error(
                        GENERIC_ARITY_MISMATCH,
                        format!(
                            "generic class `{}` expects {expected} type argument{}, but {actual} {} supplied",
                            named.name.text,
                            if expected == 1 { "" } else { "s" },
                            if actual == 1 { "was" } else { "were" },
                        ),
                    )
                    .with_primary_label(arguments.span, "wrong number of type arguments")
                    .with_secondary_label(name_span, "template declared here"),
                );
            }
        }
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::InterfaceTemplate(_),
            ..
        }) => {
            let syntax = syntax::TypeSyntax {
                kind: syntax::TypeKind::Named(named.clone()),
                span: named.span,
            };
            let _ = program::TemplateTypeResolver::for_application_site(lookup, diagnostics)
                .resolve(&syntax);
        }
        TopLevelLookup::Found(symbol) => diagnostics.push(
            Diagnostic::error(
                INVALID_GENERIC_APPLICATION,
                format!("`{}` is not a generic class", named.name.text),
            )
            .with_primary_label(arguments.span, "type arguments are not allowed here")
            .with_secondary_label(symbol.name_span, "declaration is non-generic"),
        ),
        TopLevelLookup::Missing => diagnostics.push(
            Diagnostic::error(UNKNOWN_TYPE, format!("unknown type `{}`", named.name.text))
                .with_primary_label(
                    named.name.span,
                    "no generic class with this name is declared",
                ),
        ),
        TopLevelLookup::Diagnosed => {}
    }
}

fn resolve_optional_type(
    payload_syntax: &syntax::TypeSyntax,
    lookup: ModuleLookup<'_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedTypeKind> {
    let payload = resolve_type_inner(payload_syntax, lookup, type_interner, diagnostics)?;
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
        let leaf = resolve_type_inner(leaf_syntax, lookup, type_interner, diagnostics)?;
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
        let resolved = resolve_type_inner(target, lookup, type_interner, diagnostics)?;
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
        let resolved = resolve_type_inner(target, lookup, type_interner, diagnostics)?;
        let ResolvedTypeKind::Array(array) = resolved.kind else {
            unreachable!("an array target must resolve to an array identity")
        };
        return ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Array(array));
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
    if target.arguments.is_some() {
        if let Some(class) = lookup.specialized_class(target.span) {
            report_generic_application(target, lookup, diagnostics);
            return ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Class(class));
        }
        if let Some(interface) = lookup.specialized_interface(target.span) {
            report_generic_application(target, lookup, diagnostics);
            return ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Interface(interface));
        }
        report_generic_application(target, lookup, diagnostics);
        return None;
    }
    if !target.name.is_qualified() && target.name.text == "Obj" {
        return ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Obj);
    }
    match lookup.select(&target.name, diagnostics) {
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(class),
            ..
        }) => ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Class(class)),
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::Interface(interface),
            ..
        }) => ResolvedSharedTarget::from_direct_type(ResolvedTypeKind::Interface(interface)),
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::ClassTemplate(_),
            name_span,
        }) => {
            diagnostics.push(
                Diagnostic::error(
                    RAW_GENERIC_TYPE,
                    format!(
                        "generic class `{}` requires type arguments",
                        target.name.text
                    ),
                )
                .with_primary_label(target.name.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
            );
            None
        }
        TopLevelLookup::Found(TopLevelSymbol {
            kind: TopLevelSymbolKind::InterfaceTemplate(_),
            name_span,
        }) => {
            diagnostics.push(
                Diagnostic::error(
                    RAW_GENERIC_TYPE,
                    format!(
                        "generic interface `{}` requires type arguments",
                        target.name.text
                    ),
                )
                .with_primary_label(target.name.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
            );
            None
        }
        TopLevelLookup::Found(symbol) => {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_TYPE,
                    format!("`{}` does not name a shared object type", target.name.text),
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
                    format!("unknown shared target `{}`", target.name.text),
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
    ClassTemplate(ClassTemplateId),
    Interface(InterfaceId),
    InterfaceTemplate(InterfaceTemplateId),
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
