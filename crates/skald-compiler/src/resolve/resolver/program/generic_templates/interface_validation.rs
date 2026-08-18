//! Definition-independent validation of structural interface signatures.

use super::*;

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
    if direct_interface_array(term) {
        diagnostics.push(
            Diagnostic::error(
                super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
                "array types are not supported in interface requirements",
            )
            .with_primary_label(
                term.span,
                "arrays do not participate in interface dispatch contracts",
            ),
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
    !matches!(
        term.kind,
        ResolvedTemplateTypeKind::Unit
            | ResolvedTemplateTypeKind::Obj
            | ResolvedTemplateTypeKind::Interface(_)
    )
}

fn is_closed_optional_payload(term: &ResolvedTemplateType) -> bool {
    matches!(
        term.kind,
        ResolvedTemplateTypeKind::I64
            | ResolvedTemplateTypeKind::U64
            | ResolvedTemplateTypeKind::U8
            | ResolvedTemplateTypeKind::F64
            | ResolvedTemplateTypeKind::Bool
            | ResolvedTemplateTypeKind::Class(_)
            | ResolvedTemplateTypeKind::ClassTemplate { .. }
            | ResolvedTemplateTypeKind::Shared(_)
            | ResolvedTemplateTypeKind::Optional(_)
            | ResolvedTemplateTypeKind::Array(_)
    )
}

fn is_closed_shared_target(term: &ResolvedTemplateType) -> bool {
    matches!(
        term.kind,
        ResolvedTemplateTypeKind::Obj
            | ResolvedTemplateTypeKind::Class(_)
            | ResolvedTemplateTypeKind::Interface(_)
            | ResolvedTemplateTypeKind::ClassTemplate { .. }
            | ResolvedTemplateTypeKind::InterfaceTemplate { .. }
            | ResolvedTemplateTypeKind::Array(_)
            | ResolvedTemplateTypeKind::Optional(_)
    )
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
        GenericCapability::ValueParameter => !matches!(
            term.kind,
            ResolvedTemplateTypeKind::Unit
                | ResolvedTemplateTypeKind::Obj
                | ResolvedTemplateTypeKind::Interface(_)
        ),
        GenericCapability::ValueResult => !matches!(
            term.kind,
            ResolvedTemplateTypeKind::Obj | ResolvedTemplateTypeKind::Interface(_)
        ),
        GenericCapability::AliasTarget(_) => match &term.kind {
            ResolvedTemplateTypeKind::I64
            | ResolvedTemplateTypeKind::U64
            | ResolvedTemplateTypeKind::U8
            | ResolvedTemplateTypeKind::F64
            | ResolvedTemplateTypeKind::Bool
            | ResolvedTemplateTypeKind::Obj
            | ResolvedTemplateTypeKind::Class(_)
            | ResolvedTemplateTypeKind::Interface(_)
            | ResolvedTemplateTypeKind::Array(_) => true,
            ResolvedTemplateTypeKind::Optional(payload) => optional_alias_payload(payload),
            _ => false,
        },
        _ => true,
    }
}

fn optional_alias_payload(term: &ResolvedTemplateType) -> bool {
    match &term.kind {
        ResolvedTemplateTypeKind::I64
        | ResolvedTemplateTypeKind::U64
        | ResolvedTemplateTypeKind::U8
        | ResolvedTemplateTypeKind::F64
        | ResolvedTemplateTypeKind::Bool
        | ResolvedTemplateTypeKind::Class(_)
        | ResolvedTemplateTypeKind::Array(_) => true,
        ResolvedTemplateTypeKind::Optional(payload) => optional_alias_payload(payload),
        _ => false,
    }
}

fn direct_interface_array(term: &ResolvedTemplateType) -> bool {
    matches!(term.kind, ResolvedTemplateTypeKind::Array(_))
        || matches!(
            &term.kind,
            ResolvedTemplateTypeKind::Shared(target)
                if matches!(target.kind, ResolvedTemplateTypeKind::Array(_))
        )
}

fn capability_label(capability: GenericCapability) -> &'static str {
    match capability {
        GenericCapability::ValueParameter => "value parameters require an owning stored value",
        GenericCapability::ValueResult => "non-owning views cannot escape a call",
        GenericCapability::AliasTarget(_) => "this type cannot be used as an alias target",
        _ => "invalid interface signature type",
    }
}
