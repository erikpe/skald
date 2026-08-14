//! Structural bracket names, hierarchy selection, and declaration validation.

use super::*;

const INDEX_GET: &str = "index_get";
const INDEX_SET: &str = "index_set";
#[allow(dead_code)] // Centralized now; consumed by structural slicing next.
const SLICE_GET: &str = "slice_get";
#[allow(dead_code)] // Centralized now; consumed by structural slicing next.
const SLICE_SET: &str = "slice_set";

// Keep the complete structural bracket protocol vocabulary together even
// while indexing and slicing land in separate implementation stages.
const _: [&str; 4] = [INDEX_GET, INDEX_SET, SLICE_GET, SLICE_SET];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StructuralBracketProtocol {
    IndexGet,
    IndexSet,
}

impl StructuralBracketProtocol {
    const fn name(self) -> &'static str {
        match self {
            Self::IndexGet => INDEX_GET,
            Self::IndexSet => INDEX_SET,
        }
    }
}

impl CallableResolver<'_, '_> {
    pub(super) fn select_index_protocol_method(
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
            "required indexing protocol method is missing",
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
                self.report_invalid_index_protocol(
                    protocol,
                    bracket_span,
                    declaration.name_span,
                    "a field cannot implement an indexing protocol",
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
                self.report_invalid_index_protocol(
                    protocol,
                    bracket_span,
                    declaration.name_span,
                    "a static field cannot implement an indexing protocol",
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
        };
        if let Some(reason) = invalid_shape {
            self.report_invalid_index_protocol(
                protocol,
                bracket_span,
                declaration.name_span,
                reason,
            );
            return None;
        }
        Some(method)
    }

    fn report_invalid_index_protocol(
        &mut self,
        protocol: StructuralBracketProtocol,
        bracket_span: Span,
        declaration_span: Span,
        reason: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_INDEX_PROTOCOL,
                format!("invalid `{}` indexing protocol", protocol.name()),
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
        assert_eq!(SLICE_GET, "slice_get");
        assert_eq!(SLICE_SET, "slice_set");
    }
}
