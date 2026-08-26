//! Definition-independent validation of structural interface signatures.

use super::*;
use crate::type_capabilities::{self, TypeCategory};

pub(super) fn validate_interface_signature_type(
    term: &ResolvedTemplateType,
    capability: GenericCapability,
    diagnostics: &mut Diagnostics,
) {
    validate_closed_construction(term, diagnostics);
    if !term.depends_on_parameter() && !supports_closed_signature_capability(term, capability) {
        diagnostics.push(
            Diagnostic::error(
                super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
                "generic interface requirement contains an invalid closed type",
            )
            .with_primary_label(term.span, capability_label(capability)),
        );
    }
}

fn validate_closed_construction(term: &ResolvedTemplateType, diagnostics: &mut Diagnostics) {
    match &term.kind {
        ResolvedTemplateTypeKind::Shared(target) => {
            if !target.depends_on_parameter() && !is_closed_shared_target(target) {
                report_invalid_construction(
                    target.span,
                    "this closed type cannot be the target of `shared`",
                    diagnostics,
                );
            }
            validate_closed_construction(target, diagnostics);
        }
        ResolvedTemplateTypeKind::Optional(payload) => {
            if !payload.depends_on_parameter() && !is_closed_optional_payload(payload) {
                report_invalid_construction(
                    payload.span,
                    "this closed type cannot be an inline optional payload",
                    diagnostics,
                );
            }
            validate_closed_construction(payload, diagnostics);
        }
        ResolvedTemplateTypeKind::Array(element) => {
            if !element.depends_on_parameter() && !is_closed_stored_value(element) {
                report_invalid_construction(
                    element.span,
                    "array elements must be owning, storable values",
                    diagnostics,
                );
            }
            validate_closed_construction(element, diagnostics);
        }
        ResolvedTemplateTypeKind::ClassTemplate { arguments, .. }
        | ResolvedTemplateTypeKind::InterfaceTemplate { arguments, .. } => {
            for argument in arguments {
                validate_closed_construction(argument, diagnostics);
            }
        }
        ResolvedTemplateTypeKind::Function { parameters, result } => {
            for parameter in parameters {
                validate_closed_construction(&parameter.type_syntax, diagnostics);
            }
            validate_closed_construction(result, diagnostics);
        }
        ResolvedTemplateTypeKind::I64
        | ResolvedTemplateTypeKind::U64
        | ResolvedTemplateTypeKind::U8
        | ResolvedTemplateTypeKind::F64
        | ResolvedTemplateTypeKind::Bool
        | ResolvedTemplateTypeKind::Unit
        | ResolvedTemplateTypeKind::Obj
        | ResolvedTemplateTypeKind::Parameter(_)
        | ResolvedTemplateTypeKind::Class(_)
        | ResolvedTemplateTypeKind::Interface(_) => {}
    }
}

fn is_closed_stored_value(term: &ResolvedTemplateType) -> bool {
    type_capabilities::supports_stored_value(type_category(term))
}

fn is_closed_optional_payload(term: &ResolvedTemplateType) -> bool {
    type_capabilities::supports_optional_payload(type_category(term))
}

fn is_closed_shared_target(term: &ResolvedTemplateType) -> bool {
    type_capabilities::supports_shared_target(type_category(term))
}

fn report_invalid_construction(span: Span, label: &'static str, diagnostics: &mut Diagnostics) {
    diagnostics.push(
        Diagnostic::error(
            super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
            "generic interface requirement contains an invalid closed compound type",
        )
        .with_primary_label(span, label),
    );
}

fn supports_closed_signature_capability(
    term: &ResolvedTemplateType,
    capability: GenericCapability,
) -> bool {
    match capability {
        GenericCapability::ValueParameter => {
            type_capabilities::supports_stored_value(type_category(term))
        }
        GenericCapability::ValueResult => {
            type_capabilities::supports_value_result(type_category(term))
        }
        GenericCapability::AliasTarget(_) => supports_closed_alias_target(term),
        _ => true,
    }
}

fn supports_closed_alias_target(term: &ResolvedTemplateType) -> bool {
    match &term.kind {
        ResolvedTemplateTypeKind::Optional(payload) => type_capabilities::supports_alias_target(
            TypeCategory::Optional,
            supports_closed_alias_target(payload),
        ),
        _ => type_capabilities::supports_alias_target(type_category(term), false),
    }
}

fn type_category(term: &ResolvedTemplateType) -> TypeCategory {
    match term.kind {
        ResolvedTemplateTypeKind::I64
        | ResolvedTemplateTypeKind::U64
        | ResolvedTemplateTypeKind::U8
        | ResolvedTemplateTypeKind::F64
        | ResolvedTemplateTypeKind::Bool => TypeCategory::Primitive,
        ResolvedTemplateTypeKind::Unit => TypeCategory::Unit,
        ResolvedTemplateTypeKind::Obj => TypeCategory::Obj,
        ResolvedTemplateTypeKind::Class(_) | ResolvedTemplateTypeKind::ClassTemplate { .. } => {
            TypeCategory::Class
        }
        ResolvedTemplateTypeKind::Interface(_)
        | ResolvedTemplateTypeKind::InterfaceTemplate { .. } => TypeCategory::Interface,
        ResolvedTemplateTypeKind::Function { .. } => TypeCategory::Function,
        ResolvedTemplateTypeKind::Shared(_) => TypeCategory::Shared,
        ResolvedTemplateTypeKind::Optional(_) => TypeCategory::Optional,
        ResolvedTemplateTypeKind::Array(_) => TypeCategory::Array,
        ResolvedTemplateTypeKind::Parameter(_) => {
            unreachable!("closed capability checks exclude template parameters")
        }
    }
}

fn capability_label(capability: GenericCapability) -> &'static str {
    match capability {
        GenericCapability::ValueParameter => "value parameters require an owning stored value",
        GenericCapability::ValueResult => "non-owning views cannot escape a call",
        GenericCapability::AliasTarget(_) => "this type cannot be used as an alias target",
        _ => "invalid interface signature type",
    }
}
