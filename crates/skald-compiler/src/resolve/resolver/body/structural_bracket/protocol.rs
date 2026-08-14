//! Structural bracket names, hierarchy selection, and declaration validation.

use super::*;

const INDEX_GET: &str = "index_get";
const INDEX_SET: &str = "index_set";
const SLICE_GET: &str = "slice_get";
const SLICE_SET: &str = "slice_set";

// Keep the complete structural bracket protocol vocabulary together even
// while indexing and slicing land in separate implementation stages.
const _: [&str; 4] = [INDEX_GET, INDEX_SET, SLICE_GET, SLICE_SET];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StructuralBracketProtocol {
    IndexGet,
    IndexSet,
    SliceGet,
    SliceSet,
}

impl StructuralBracketProtocol {
    const fn name(self) -> &'static str {
        match self {
            Self::IndexGet => INDEX_GET,
            Self::IndexSet => INDEX_SET,
            Self::SliceGet => SLICE_GET,
            Self::SliceSet => SLICE_SET,
        }
    }
}

impl CallableResolver<'_, '_> {
    pub(super) fn select_structural_bracket_method(
        &mut self,
        class: ClassId,
        protocol: StructuralBracketProtocol,
        bracket_span: Span,
    ) -> Option<MethodId> {
        let name = protocol.name();
        let selected = self.select_member_named(
            class,
            name,
            bracket_span,
            INVALID_INDEX_PROTOCOL,
            "required structural bracket protocol method is missing",
        )?;
        let method = match selected {
            SelectedClassMember::Method(method) => method,
            SelectedClassMember::Field(field) => {
                let declaration = self
                    .environment
                    .classes
                    .get(field.class())
                    .and_then(|class| class.field(field))
                    .expect("selected protocol field must retain declaration metadata");
                self.report_invalid_structural_bracket_protocol(
                    protocol,
                    bracket_span,
                    declaration.name_span,
                    "a field cannot implement a structural bracket protocol",
                );
                return None;
            }
            SelectedClassMember::StaticField(field) => {
                let declaration = self
                    .environment
                    .classes
                    .get(field.class())
                    .and_then(|class| class.static_field(field))
                    .expect("selected protocol static field must retain declaration metadata");
                self.report_invalid_structural_bracket_protocol(
                    protocol,
                    bracket_span,
                    declaration.name_span,
                    "a static field cannot implement a structural bracket protocol",
                );
                return None;
            }
        };
        let declaration = self
            .environment
            .classes
            .get(method.class())
            .and_then(|class| class.method(method))
            .expect("selected protocol method must retain declaration metadata")
            .clone();

        let invalid_shape = match protocol {
            StructuralBracketProtocol::IndexGet => match declaration.kind {
                ResolvedMethodKind::Static => Some("`index_get` must be an instance method"),
                ResolvedMethodKind::Instance {
                    receiver_access: ResolvedReceiverAccess::Mutable,
                    ..
                } => Some("`index_get` must have a read-only receiver"),
                ResolvedMethodKind::Instance { .. } if declaration.parameters.len() != 1 => {
                    Some("`index_get` must take exactly one key parameter")
                }
                ResolvedMethodKind::Instance { .. }
                    if matches!(
                        declaration.parameters[0].binding_mode,
                        ResolvedParameterBindingMode::MutableAlias { .. }
                    ) =>
                {
                    Some("`index_get` key parameter cannot be a mutable alias")
                }
                ResolvedMethodKind::Instance { .. } => None,
            },
            StructuralBracketProtocol::IndexSet => match declaration.kind {
                ResolvedMethodKind::Static => Some("`index_set` must be an instance method"),
                ResolvedMethodKind::Instance {
                    receiver_access: ResolvedReceiverAccess::ReadOnly,
                    ..
                } => Some("`index_set` must have a mutable receiver"),
                ResolvedMethodKind::Instance { .. } if declaration.parameters.len() != 2 => {
                    Some("`index_set` must take exactly a key and replacement parameter")
                }
                ResolvedMethodKind::Instance { .. }
                    if declaration.parameters.iter().any(|parameter| {
                        matches!(
                            parameter.binding_mode,
                            ResolvedParameterBindingMode::MutableAlias { .. }
                        )
                    }) =>
                {
                    Some("`index_set` parameters cannot be mutable aliases")
                }
                ResolvedMethodKind::Instance { .. }
                    if declaration.return_type.kind != ResolvedTypeKind::Unit =>
                {
                    Some("`index_set` must return exactly `unit`")
                }
                ResolvedMethodKind::Instance { .. } => None,
            },
            StructuralBracketProtocol::SliceGet => match declaration.kind {
                ResolvedMethodKind::Static => Some("`slice_get` must be an instance method"),
                ResolvedMethodKind::Instance {
                    receiver_access: ResolvedReceiverAccess::Mutable,
                    ..
                } => Some("`slice_get` must have a read-only receiver"),
                ResolvedMethodKind::Instance { .. } if declaration.parameters.len() != 2 => {
                    Some("`slice_get` must take exactly start and end parameters")
                }
                ResolvedMethodKind::Instance { .. }
                    if !self.has_exact_slice_bound_parameters(&declaration.parameters[..2]) =>
                {
                    Some("`slice_get` bounds must be exact value parameters of type `i64?`")
                }
                ResolvedMethodKind::Instance { .. } => None,
            },
            StructuralBracketProtocol::SliceSet => match declaration.kind {
                ResolvedMethodKind::Static => Some("`slice_set` must be an instance method"),
                ResolvedMethodKind::Instance {
                    receiver_access: ResolvedReceiverAccess::ReadOnly,
                    ..
                } => Some("`slice_set` must have a mutable receiver"),
                ResolvedMethodKind::Instance { .. } if declaration.parameters.len() != 3 => {
                    Some("`slice_set` must take start, end, and replacement parameters")
                }
                ResolvedMethodKind::Instance { .. }
                    if !self.has_exact_slice_bound_parameters(&declaration.parameters[..2]) =>
                {
                    Some("`slice_set` bounds must be exact value parameters of type `i64?`")
                }
                ResolvedMethodKind::Instance { .. }
                    if matches!(
                        declaration.parameters[2].binding_mode,
                        ResolvedParameterBindingMode::MutableAlias { .. }
                    ) =>
                {
                    Some("`slice_set` replacement cannot be a mutable alias")
                }
                ResolvedMethodKind::Instance { .. }
                    if declaration.return_type.kind != ResolvedTypeKind::Unit =>
                {
                    Some("`slice_set` must return exactly `unit`")
                }
                ResolvedMethodKind::Instance { .. } => None,
            },
        };
        if let Some(reason) = invalid_shape {
            self.report_invalid_structural_bracket_protocol(
                protocol,
                bracket_span,
                declaration.name_span,
                reason,
            );
            return None;
        }
        Some(method)
    }

    fn has_exact_slice_bound_parameters(&self, parameters: &[ResolvedParameter]) -> bool {
        parameters.iter().all(|parameter| {
            if !matches!(parameter.binding_mode, ResolvedParameterBindingMode::Value) {
                return false;
            }
            let ResolvedTypeKind::Optional(optional) = parameter.type_syntax.kind else {
                return false;
            };
            self.type_interner
                .optional(optional)
                .is_some_and(|optional| optional.payload.kind == ResolvedTypeKind::I64)
        })
    }

    fn report_invalid_structural_bracket_protocol(
        &mut self,
        protocol: StructuralBracketProtocol,
        bracket_span: Span,
        declaration_span: Span,
        reason: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_INDEX_PROTOCOL,
                format!("invalid `{}` structural bracket protocol", protocol.name()),
            )
            .with_primary_label(bracket_span, reason)
            .with_secondary_label(declaration_span, "protocol member declared here"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centralizes_all_structural_bracket_protocol_names() {
        assert_eq!(StructuralBracketProtocol::IndexGet.name(), INDEX_GET);
        assert_eq!(StructuralBracketProtocol::IndexSet.name(), INDEX_SET);
        assert_eq!(StructuralBracketProtocol::SliceGet.name(), SLICE_GET);
        assert_eq!(StructuralBracketProtocol::SliceSet.name(), SLICE_SET);
    }
}
