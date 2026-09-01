//! Nominal `Iterable<Item, State>` selection and loop-local scope resolution.

use super::*;
use crate::identity::{LocalId, LoopId};

#[derive(Clone, Copy)]
struct IterableCandidate {
    selection: ResolvedIterableSelection,
}

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_for_in(
        &mut self,
        statement: &syntax::ForInStatement,
    ) -> Option<ResolvedForIn> {
        if self.environment.range_requests.is_some() {
            if let syntax::ForInSource::Range(range) = &statement.source {
                self.probe_range_for_in(statement, range);
                return None;
            }
        }

        let loop_id = LoopId::new(self.callable, self.next_loop_index);
        self.next_loop_index += 1;

        // Header components resolve in the enclosing scope. In particular,
        // the iterable cannot observe the item binding it will produce.
        let annotation = statement
            .annotation
            .as_ref()
            .map(|annotation| self.resolve_type(&annotation.type_syntax));
        let annotation_valid = annotation.as_ref().is_none_or(Option::is_some);
        let annotation = annotation.flatten();
        let (source, source_type) = match &statement.source {
            syntax::ForInSource::Iterable(iterable) => {
                let iterable = self.resolve_expression(iterable)?;
                let source_type = self.resolved_expression_type(&iterable);
                (ResolvedForInSource::Iterable(iterable), source_type)
            }
            syntax::ForInSource::Range(range) => {
                let source = self.resolve_range_source(range)?;
                let source_type = match &source {
                    ResolvedForInSource::Iterable(iterable) => {
                        self.resolved_expression_type(iterable)
                    }
                    ResolvedForInSource::Range(range) => {
                        Some(ResolvedTypeKind::Class(range.range_class))
                    }
                };
                (source, source_type)
            }
        };
        let selection = self.select_iterable(
            source_type,
            annotation.as_ref().map(|annotation| annotation.kind),
            statement,
        )?;
        if !annotation_valid {
            return None;
        }

        let (local, body) = self.resolve_iteration_body(statement, loop_id, selection.item);

        Some(ResolvedForIn {
            loop_id,
            binding: local,
            source,
            selection,
            body,
            for_span: statement.for_span,
            binding_span: statement.binding.span,
            annotation_span: statement
                .annotation
                .as_ref()
                .map(|annotation| annotation.span),
            in_span: statement.in_span,
            span: statement.span,
        })
    }

    fn probe_range_for_in(
        &mut self,
        statement: &syntax::ForInStatement,
        range: &syntax::ForRangeSource,
    ) {
        let loop_id = LoopId::new(self.callable, self.next_loop_index);
        self.next_loop_index += 1;

        let annotation = statement
            .annotation
            .as_ref()
            .map(|annotation| self.resolve_type(&annotation.type_syntax));
        let annotation_valid = annotation.as_ref().is_none_or(Option::is_some);
        let annotation = annotation.flatten();
        let Some(item) = self.probe_range_source(range) else {
            return;
        };
        if !annotation_valid || annotation.is_some_and(|annotation| annotation.kind != item) {
            return;
        }

        let _ = self.resolve_iteration_body(statement, loop_id, item);
    }

    fn resolve_iteration_body(
        &mut self,
        statement: &syntax::ForInStatement,
        loop_id: LoopId,
        item: ResolvedTypeKind,
    ) -> (LocalId, ResolvedBlock) {
        let local = LocalId::new(self.callable, self.locals.len());
        self.scopes.push(HashMap::new());
        let declared = self.declare_binding(
            &statement.binding.text,
            BindingSymbol {
                id: BindingId::Local(local),
                ty: item,
                name_span: statement.binding.span,
            },
            "iteration binding",
        );
        debug_assert!(declared, "a fresh loop body scope has no bindings");
        self.locals.push(ResolvedLocal {
            id: local,
            name: statement.binding.text.to_string(),
            name_span: statement.binding.span,
            type_syntax: ResolvedType {
                kind: item,
                span: statement
                    .annotation
                    .as_ref()
                    .map_or(statement.binding.span, |annotation| {
                        annotation.type_syntax.span
                    }),
            },
            span: statement.binding.span,
        });

        self.active_loops.push(loop_id);
        let body = self.resolve_block_in_current_scope(&statement.body, false);
        let active = self
            .active_loops
            .pop()
            .expect("resolving an iteration body requires an active loop");
        debug_assert_eq!(active, loop_id);
        self.scopes
            .pop()
            .expect("an iteration body owns one lexical scope");
        (local, body)
    }

    fn select_iterable(
        &mut self,
        static_type: Option<ResolvedTypeKind>,
        annotation: Option<ResolvedTypeKind>,
        statement: &syntax::ForInStatement,
    ) -> Option<ResolvedIterableSelection> {
        let Some(environment) = self.environment.language_items.iteration else {
            // The canonical declaration diagnostic is owned by program
            // resolution. Avoid a misleading secondary protocol error.
            return None;
        };
        if let Some(selection) = self
            .environment
            .specialization
            .and_then(|specialization| specialization.iteration_selection(statement.for_span))
        {
            return annotation
                .is_none_or(|annotation| annotation == selection.item)
                .then_some(selection)
                .or_else(|| {
                    self.diagnostics.push(
                        Diagnostic::error(
                            super::super::ITERATION_ITEM_TYPE_MISMATCH,
                            "the iteration item annotation does not match the selected generic bound",
                        )
                        .with_primary_label(
                            statement
                                .annotation
                                .as_ref()
                                .expect("a failed annotation comparison has source syntax")
                                .type_syntax
                                .span,
                            "exact item type required here",
                        )
                        .with_secondary_label(
                            selection.origin_span,
                            "the definition-site bound selected this item type",
                        ),
                    );
                    None
                });
        }
        let mut candidates = static_type
            .into_iter()
            .flat_map(|ty| self.iterable_candidates(ty, environment))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.selection.interface);
        candidates.dedup_by_key(|candidate| candidate.selection.interface);

        let unfiltered = candidates.clone();
        if let Some(annotation) = annotation {
            candidates.retain(|candidate| candidate.selection.item == annotation);
        }

        match candidates.as_slice() {
            [candidate] => Some(candidate.selection),
            [] if annotation.is_some() && !unfiltered.is_empty() => {
                let annotation_span = statement
                    .annotation
                    .as_ref()
                    .expect("an annotation type supplied the filter")
                    .type_syntax
                    .span;
                let mut diagnostic = Diagnostic::error(
                    super::super::ITERATION_ITEM_TYPE_MISMATCH,
                    "the iteration item annotation matches no eligible `Iterable` application",
                )
                .with_primary_label(annotation_span, "exact item type required here");
                for candidate in unfiltered {
                    diagnostic = diagnostic.with_secondary_label(
                        candidate.selection.origin_span,
                        "candidate application has a different item type",
                    );
                }
                self.diagnostics.push(diagnostic);
                None
            }
            [] => {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::MISSING_ITERABLE_APPLICATION,
                        "the iterable type provides no eligible `std::iter::Iterable` application",
                    )
                    .with_primary_label(
                        statement.source.span(),
                        "no nominal iteration claim is reachable from this static type",
                    ),
                );
                None
            }
            candidates => {
                let mut diagnostic = Diagnostic::error(
                    super::super::AMBIGUOUS_ITERABLE_APPLICATION,
                    "multiple eligible `std::iter::Iterable` applications remain",
                )
                .with_primary_label(
                    statement
                        .annotation
                        .as_ref()
                        .map_or(statement.source.span(), |annotation| {
                            annotation.type_syntax.span
                        }),
                    "iteration protocol selection is ambiguous",
                );
                for candidate in candidates {
                    diagnostic = diagnostic.with_secondary_label(
                        candidate.selection.origin_span,
                        "candidate application declared here",
                    );
                }
                self.diagnostics.push(diagnostic);
                None
            }
        }
    }

    fn iterable_candidates(
        &self,
        ty: ResolvedTypeKind,
        environment: IterationResolutionEnvironment<'_>,
    ) -> Vec<IterableCandidate> {
        match ty {
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
                    class.implemented_interfaces.iter().filter_map(|claim| {
                        self.iterable_candidate(
                            claim.interface.ordinary()?,
                            claim.span,
                            environment,
                        )
                    })
                })
                .collect(),
            ResolvedTypeKind::Interface(interface) => self
                .environment
                .interfaces
                .get(interface)
                .and_then(|declaration| {
                    self.iterable_candidate(interface, declaration.span, environment)
                })
                .into_iter()
                .collect(),
            ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class)) => {
                self.iterable_candidates(ResolvedTypeKind::Class(class), environment)
            }
            ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(interface)) => {
                self.iterable_candidates(ResolvedTypeKind::Interface(interface), environment)
            }
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Function(_)
            | ResolvedTypeKind::Array(_)
            | ResolvedTypeKind::Shared(_)
            | ResolvedTypeKind::Optional(_) => Vec::new(),
        }
    }

    fn iterable_candidate(
        &self,
        interface: InterfaceId,
        origin_span: Span,
        environment: IterationResolutionEnvironment<'_>,
    ) -> Option<IterableCandidate> {
        let application = environment.applications.for_interface(interface)?;
        if application.key.template != environment.language_item.template {
            return None;
        }
        let [item, state] = application.key.arguments.as_slice() else {
            return None;
        };
        let requirement = |template| {
            application
                .requirement_mappings
                .iter()
                .find(|mapping| mapping.template == template)
                .map(|mapping| mapping.closed)
        };
        Some(IterableCandidate {
            selection: ResolvedIterableSelection {
                interface,
                iter_state: requirement(environment.language_item.iter_state_requirement)?,
                iter_next: requirement(environment.language_item.iter_next_requirement)?,
                item: *item,
                state: *state,
                origin_span,
            },
        })
    }
}
