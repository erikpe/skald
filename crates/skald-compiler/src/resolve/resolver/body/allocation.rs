//! Shared-allocation target validation and construction-mode resolution.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_allocation(
        &mut self,
        allocation: &syntax::AllocationExpr,
    ) -> Option<ResolvedExpression> {
        if reject_qualified_name(&allocation.target, self.diagnostics) {
            return None;
        }
        let class = if allocation.target.text == "Obj" {
            self.diagnostics.push(
                Diagnostic::error(INVALID_CONSTRUCTION_TARGET, "`Obj` cannot be allocated")
                    .with_primary_label(
                        allocation.target.span,
                        "`Obj` is a view target and cannot be allocated directly",
                    ),
            );
            None
        } else {
            self.resolve_allocation_class(allocation)
        };

        let mode = self.resolve_allocation_mode(&allocation.arguments);
        let (Some(class), Some(mode)) = (class, mode) else {
            return None;
        };
        Some(ResolvedExpression::Allocation(ResolvedAllocationExpr {
            class,
            new_span: allocation.new_span,
            target_span: allocation.target.span,
            mode,
            span: allocation.span,
        }))
    }

    fn resolve_allocation_class(&mut self, allocation: &syntax::AllocationExpr) -> Option<ClassId> {
        match self
            .environment
            .top_levels
            .get(allocation.target.text.as_str())
            .copied()
        {
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => self.validate_constructible_allocation_class(class, allocation),
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION_TARGET,
                        format!("interface `{}` cannot be allocated", allocation.target.text),
                    )
                    .with_primary_label(allocation.target.span, "`new` requires a concrete class"),
                );
                None
            }
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Function(_),
                ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION_TARGET,
                        format!(
                            "function `{}` is not an allocation class",
                            allocation.target.text
                        ),
                    )
                    .with_primary_label(allocation.target.span, "`new` requires a concrete class"),
                );
                None
            }
            None => {
                self.report_unknown(
                    &allocation.target.text,
                    allocation.target.span,
                    "unknown allocation class",
                );
                None
            }
        }
    }

    fn validate_constructible_allocation_class(
        &mut self,
        class: ClassId,
        allocation: &syntax::AllocationExpr,
    ) -> Option<ClassId> {
        let declaration = self
            .environment
            .classes
            .get(class)
            .expect("class symbols must reference declaration metadata");
        let (constructible, message, label) = match &allocation.arguments {
            syntax::CallArguments::Ordinary(_) => (
                !declaration.initializers.is_empty(),
                format!("class `{}` has no ordinary initializer", declaration.name),
                "ordinary allocation requires an explicit `init` declaration",
            ),
            syntax::CallArguments::Copy { .. } => (
                declaration.copy_constructor != ResolvedCopyOperation::Unavailable,
                format!("class `{}` is not copy-constructible", declaration.name),
                "copy allocation requires an available copy constructor",
            ),
        };
        if constructible {
            return Some(class);
        }
        self.diagnostics.push(
            Diagnostic::error(INVALID_CONSTRUCTION_TARGET, message)
                .with_primary_label(allocation.target.span, label),
        );
        None
    }

    fn resolve_allocation_mode(
        &mut self,
        arguments: &syntax::CallArguments,
    ) -> Option<ResolvedConstructionMode> {
        match arguments {
            syntax::CallArguments::Ordinary(arguments) => {
                let mut resolved = Vec::with_capacity(arguments.len());
                let mut valid = true;
                for argument in arguments {
                    match self.resolve_expression(argument) {
                        Some(argument) => resolved.push(argument),
                        None => valid = false,
                    }
                }
                valid.then_some(ResolvedConstructionMode::Initialize {
                    arguments: resolved,
                })
            }
            syntax::CallArguments::Copy { copy_span, source } => self
                .resolve_expression(source)
                .map(|source| ResolvedConstructionMode::Copy {
                    copy_span: *copy_span,
                    source: Box::new(source),
                }),
        }
    }
}
