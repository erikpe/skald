//! Structural bracket names, hierarchy selection, and declaration validation.

use super::*;

const INDEX_GET: &str = "index_get";
const INDEX_SET: &str = "index_set";
const SLICE_GET: &str = "slice_get";
const SLICE_SET: &str = "slice_set";

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

        let mutable = match declaration.kind {
            ResolvedMethodKind::Static => {
                self.report_invalid_structural_bracket_protocol(
                    protocol,
                    bracket_span,
                    declaration.name_span,
                    "structural bracket protocols require an instance method",
                );
                return None;
            }
            ResolvedMethodKind::Instance {
                receiver_access, ..
            } => receiver_access == ResolvedReceiverAccess::Mutable,
        };
        if let Some(reason) = self.invalid_structural_bracket_signature(
            protocol,
            mutable,
            &declaration.parameters,
            declaration.return_type.kind,
        ) {
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

    pub(super) fn select_structural_bracket_requirement(
        &mut self,
        interface: InterfaceId,
        protocol: StructuralBracketProtocol,
        bracket_span: Span,
    ) -> Option<InterfaceRequirementId> {
        let declaration = self
            .environment
            .interfaces
            .get(interface)
            .expect("interface receiver type must reference a declaration");
        let Some(requirement) = declaration
            .requirements
            .iter()
            .find(|requirement| requirement.name == protocol.name())
            .cloned()
        else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INDEX_PROTOCOL,
                    format!(
                        "interface `{}` has no structural bracket requirement `{}`",
                        declaration.name,
                        protocol.name()
                    ),
                )
                .with_primary_label(
                    bracket_span,
                    "required structural bracket protocol requirement is missing",
                ),
            );
            return None;
        };
        if let Some(reason) = self.invalid_structural_bracket_signature(
            protocol,
            requirement.mutable,
            &requirement.parameters,
            requirement.return_type.kind,
        ) {
            self.report_invalid_structural_bracket_protocol(
                protocol,
                bracket_span,
                requirement.name_span,
                reason,
            );
            return None;
        }
        Some(requirement.id)
    }

    fn invalid_structural_bracket_signature<P: StructuralBracketParameter>(
        &self,
        protocol: StructuralBracketProtocol,
        mutable: bool,
        parameters: &[P],
        return_type: ResolvedTypeKind,
    ) -> Option<&'static str> {
        match protocol {
            StructuralBracketProtocol::IndexGet if mutable => {
                Some("`index_get` must have a read-only receiver")
            }
            StructuralBracketProtocol::IndexGet if parameters.len() != 1 => {
                Some("`index_get` must take exactly one key parameter")
            }
            StructuralBracketProtocol::IndexGet
                if parameters[0].binding_mode().matches_mutable_alias() =>
            {
                Some("`index_get` key parameter cannot be a mutable alias")
            }
            StructuralBracketProtocol::IndexSet if !mutable => {
                Some("`index_set` must have a mutable receiver")
            }
            StructuralBracketProtocol::IndexSet if parameters.len() != 2 => {
                Some("`index_set` must take exactly a key and replacement parameter")
            }
            StructuralBracketProtocol::IndexSet
                if parameters
                    .iter()
                    .any(|parameter| parameter.binding_mode().matches_mutable_alias()) =>
            {
                Some("`index_set` parameters cannot be mutable aliases")
            }
            StructuralBracketProtocol::IndexSet if return_type != ResolvedTypeKind::Unit => {
                Some("`index_set` must return exactly `unit`")
            }
            StructuralBracketProtocol::SliceGet if mutable => {
                Some("`slice_get` must have a read-only receiver")
            }
            StructuralBracketProtocol::SliceGet if parameters.len() != 2 => {
                Some("`slice_get` must take exactly start and end parameters")
            }
            StructuralBracketProtocol::SliceGet
                if !self.has_exact_slice_bound_parameters(parameters) =>
            {
                Some("`slice_get` bounds must be exact value parameters of type `i64?`")
            }
            StructuralBracketProtocol::SliceSet if !mutable => {
                Some("`slice_set` must have a mutable receiver")
            }
            StructuralBracketProtocol::SliceSet if parameters.len() != 3 => {
                Some("`slice_set` must take start, end, and replacement parameters")
            }
            StructuralBracketProtocol::SliceSet
                if !self.has_exact_slice_bound_parameters(&parameters[..2]) =>
            {
                Some("`slice_set` bounds must be exact value parameters of type `i64?`")
            }
            StructuralBracketProtocol::SliceSet
                if parameters[2].binding_mode().matches_mutable_alias() =>
            {
                Some("`slice_set` replacement cannot be a mutable alias")
            }
            StructuralBracketProtocol::SliceSet if return_type != ResolvedTypeKind::Unit => {
                Some("`slice_set` must return exactly `unit`")
            }
            _ => None,
        }
    }

    fn has_exact_slice_bound_parameters<P: StructuralBracketParameter>(
        &self,
        parameters: &[P],
    ) -> bool {
        parameters.iter().all(|parameter| {
            if parameter.binding_mode() != ResolvedParameterBindingMode::Value {
                return false;
            }
            let ResolvedTypeKind::Optional(optional) = parameter.type_kind() else {
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

trait StructuralBracketParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode;
    fn type_kind(&self) -> ResolvedTypeKind;
}

impl StructuralBracketParameter for ResolvedParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode {
        self.binding_mode
    }

    fn type_kind(&self) -> ResolvedTypeKind {
        self.type_syntax.kind
    }
}

impl StructuralBracketParameter for ResolvedInterfaceParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode {
        self.binding_mode
    }

    fn type_kind(&self) -> ResolvedTypeKind {
        self.type_syntax.kind
    }
}

trait ParameterBindingModeExt {
    fn matches_mutable_alias(self) -> bool;
}

impl ParameterBindingModeExt for ResolvedParameterBindingMode {
    fn matches_mutable_alias(self) -> bool {
        matches!(self, Self::MutableAlias { .. })
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
