//! Canonical candidate resolution for overloadable operator punctuation.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn select_unary_operator(
        &self,
        operator: ResolvedUnaryOperator,
        operand: &ResolvedExpression,
    ) -> Option<ResolvedOperatorResolution> {
        let protocol = operator.protocol()?;
        let left = self.resolved_expression_type(operand)?;
        matches!(
            left,
            ResolvedTypeKind::Class(_) | ResolvedTypeKind::Interface(_)
        )
        .then(|| self.resolve_operator(protocol, left, None))
    }

    pub(super) fn select_binary_operator(
        &self,
        operator: ResolvedBinaryOperator,
        left: &ResolvedExpression,
        right: &ResolvedExpression,
    ) -> Option<ResolvedOperatorResolution> {
        let protocol = operator.protocol();
        let left_type = self.resolved_expression_type(left)?;
        if !matches!(
            left_type,
            ResolvedTypeKind::Class(_) | ResolvedTypeKind::Interface(_)
        ) {
            return None;
        }
        Some(self.resolve_operator(protocol, left_type, self.resolved_expression_type(right)))
    }

    fn resolve_operator(
        &self,
        protocol: CanonicalOperatorProtocol,
        left: ResolvedTypeKind,
        right: Option<ResolvedTypeKind>,
    ) -> ResolvedOperatorResolution {
        let Some(environment) = self.environment.language_items.operators else {
            return ResolvedOperatorResolution {
                protocol,
                candidates: Vec::new(),
            };
        };
        let canonical = environment.language_item.get(protocol);
        let mut candidates = self.operator_candidates(left, canonical, environment);
        if matches!(
            canonical.kind.shape(),
            CanonicalOperatorProtocolShape::Predicate | CanonicalOperatorProtocolShape::Binary
        ) {
            candidates.retain(|candidate| {
                candidate.rhs.zip(right).is_some_and(|(expected, actual)| {
                    self.readonly_alias_type_compatible(actual, expected)
                })
            });
        }
        candidates.sort_by_key(|candidate| candidate.interface);
        candidates.dedup_by_key(|candidate| candidate.interface);
        ResolvedOperatorResolution {
            protocol,
            candidates,
        }
    }

    fn operator_candidates(
        &self,
        left: ResolvedTypeKind,
        canonical: &ResolvedOperatorProtocol,
        environment: OperatorResolutionEnvironment<'_>,
    ) -> Vec<ResolvedOperatorSelection> {
        let applications = match left {
            ResolvedTypeKind::Class(class) => std::iter::once(class)
                .chain(
                    self.environment
                        .hierarchy
                        .base_chain(class)
                        .into_iter()
                        .flatten(),
                )
                .filter_map(|class| self.environment.classes.get(class))
                .flat_map(|class| {
                    class
                        .implemented_interfaces
                        .iter()
                        .filter_map(|claim| Some((claim.interface.ordinary()?, claim.span)))
                })
                .collect::<Vec<_>>(),
            ResolvedTypeKind::Interface(interface) => self
                .environment
                .interfaces
                .get(interface)
                .map(|declaration| vec![(interface, declaration.span)])
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        applications
            .into_iter()
            .filter_map(|(interface, origin_span)| {
                let application = environment.applications.for_interface(interface)?;
                if application.key.template != canonical.template {
                    return None;
                }
                let requirement = application
                    .requirement_mappings
                    .iter()
                    .find(|mapping| mapping.template == canonical.requirement)?
                    .closed;
                let (rhs, output) = match canonical.kind.shape() {
                    CanonicalOperatorProtocolShape::Unary => {
                        let [output] = application.key.arguments.as_slice() else {
                            return None;
                        };
                        (None, *output)
                    }
                    CanonicalOperatorProtocolShape::Binary => {
                        let [rhs, output] = application.key.arguments.as_slice() else {
                            return None;
                        };
                        (Some(*rhs), *output)
                    }
                    CanonicalOperatorProtocolShape::Predicate => {
                        let [rhs] = application.key.arguments.as_slice() else {
                            return None;
                        };
                        (Some(*rhs), ResolvedTypeKind::Bool)
                    }
                };
                Some(ResolvedOperatorSelection {
                    protocol: canonical.kind,
                    interface,
                    requirement,
                    rhs,
                    output,
                    origin_span,
                })
            })
            .collect()
    }

    fn readonly_alias_type_compatible(
        &self,
        actual: ResolvedTypeKind,
        expected: ResolvedTypeKind,
    ) -> bool {
        if actual == expected {
            return true;
        }
        match (actual, expected) {
            (ResolvedTypeKind::Class(_), ResolvedTypeKind::Obj)
            | (ResolvedTypeKind::Interface(_), ResolvedTypeKind::Obj) => true,
            (ResolvedTypeKind::Class(actual), ResolvedTypeKind::Class(expected)) => self
                .environment
                .hierarchy
                .is_subtype(actual, expected)
                .unwrap_or(false),
            (ResolvedTypeKind::Class(actual), ResolvedTypeKind::Interface(expected)) => {
                self.class_conforms_to_interface(actual, expected)
            }
            _ => false,
        }
    }

    fn class_conforms_to_interface(&self, class: ClassId, interface: InterfaceId) -> bool {
        std::iter::once(class)
            .chain(
                self.environment
                    .hierarchy
                    .base_chain(class)
                    .into_iter()
                    .flatten(),
            )
            .filter_map(|class| self.environment.classes.get(class))
            .any(|class| {
                class
                    .implemented_interfaces
                    .iter()
                    .any(|claim| claim.interface.ordinary() == Some(interface))
            })
    }
}
