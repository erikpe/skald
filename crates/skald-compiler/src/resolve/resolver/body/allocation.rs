//! Shared-allocation target validation and construction-mode resolution.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_optional_box_allocation(
        &mut self,
        allocation: &syntax::OptionalBoxAllocationExpr,
    ) -> Option<ResolvedExpression> {
        let resolved_target = self.resolve_type(&allocation.target);
        let initializer = match &allocation.initializer {
            syntax::OptionalBoxInitializer::Absent {
                left_paren_span,
                right_paren_span,
            } => Some(ResolvedOptionalBoxInitializer::Absent {
                left_paren_span: *left_paren_span,
                right_paren_span: *right_paren_span,
            }),
            syntax::OptionalBoxInitializer::Value {
                left_paren_span,
                value,
                right_paren_span,
            } => {
                self.resolve_expression(value)
                    .map(|value| ResolvedOptionalBoxInitializer::Value {
                        left_paren_span: *left_paren_span,
                        value: Box::new(value),
                        right_paren_span: *right_paren_span,
                    })
            }
        };
        let (Some(resolved_target), Some(initializer)) = (resolved_target, initializer) else {
            return None;
        };
        let ResolvedTypeKind::Optional(exact_optional) = resolved_target.kind else {
            unreachable!("optional-box syntax must resolve to an optional identity")
        };
        let target = self
            .type_interner
            .intern_optional_box(exact_optional, allocation.target.span);
        match self
            .type_interner
            .optional_box(target)
            .expect("newly interned optional-box target must exist")
            .object_leaf
        {
            Some(ResolvedObjectTarget::Obj) => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_CONSTRUCTION_TARGET, "`Obj?` cannot be allocated")
                        .with_primary_label(
                            allocation.target.span,
                            "`Obj` is a static box view; allocate an exact concrete class instead",
                        ),
                );
                return None;
            }
            Some(ResolvedObjectTarget::Interface(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION_TARGET,
                        "an interface optional box cannot be allocated",
                    )
                    .with_primary_label(
                        allocation.target.span,
                        "allocate an optional box with an exact concrete class leaf",
                    ),
                );
                return None;
            }
            Some(ResolvedObjectTarget::Class(_)) | None => {}
        }
        Some(ResolvedExpression::OptionalBoxAllocation(
            ResolvedOptionalBoxAllocationExpr {
                exact_optional,
                target,
                new_span: allocation.new_span,
                target_span: allocation.target.span,
                initializer,
                span: allocation.span,
            },
        ))
    }

    pub(super) fn resolve_allocation(
        &mut self,
        allocation: &syntax::AllocationExpr,
    ) -> Option<ResolvedExpression> {
        let class = if allocation.target.arguments.is_some() {
            match self.specialized_class(&allocation.target) {
                Some(class) => self.validate_constructible_allocation_class(class, allocation),
                None => {
                    self.report_unsupported_generic_application(&allocation.target);
                    None
                }
            }
        } else if !allocation.target.name.is_qualified() && allocation.target.name.text == "Obj" {
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
            .lookup
            .select(&allocation.target.name, self.diagnostics)
        {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => self.validate_constructible_allocation_class(class, allocation),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION_TARGET,
                        format!(
                            "interface `{}` cannot be allocated",
                            allocation.target.name.text
                        ),
                    )
                    .with_primary_label(allocation.target.span, "`new` requires a concrete class"),
                );
                None
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::ClassTemplate(_),
                ..
            }) => {
                self.report_raw_generic_type(&allocation.target.name.text, allocation.target.span);
                None
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::InterfaceTemplate(_),
                name_span,
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        RAW_GENERIC_TYPE,
                        format!(
                            "generic interface `{}` requires type arguments",
                            allocation.target.name.text
                        ),
                    )
                    .with_primary_label(allocation.target.span, "type arguments cannot be omitted")
                    .with_secondary_label(name_span, "template declared here"),
                );
                None
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Function(_),
                ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION_TARGET,
                        format!(
                            "function `{}` is not an allocation class",
                            allocation.target.name.text
                        ),
                    )
                    .with_primary_label(allocation.target.span, "`new` requires a concrete class"),
                );
                None
            }
            TopLevelLookup::Missing => {
                self.report_unknown(
                    &allocation.target.name.text,
                    allocation.target.span,
                    "unknown allocation class",
                );
                None
            }
            TopLevelLookup::Diagnosed => None,
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
