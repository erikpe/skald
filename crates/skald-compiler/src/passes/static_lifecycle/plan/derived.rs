//! Derived planned views that are intentionally absent from stored MIR.

use std::collections::BTreeMap;

use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    mir::{
        MirStaticLifecycleTransition, MirStaticLifecycleTransitionKind, PreliminaryMirStaticField,
        PreliminaryMirStaticInitializer, StaticLifecyclePlan,
    },
};

use super::model::PlannedMirProgram;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LifecyclePosition {
    pub(crate) activation: usize,
    pub(crate) shutdown: usize,
}

pub(crate) fn positions(plan: &StaticLifecyclePlan) -> BTreeMap<StaticFieldId, LifecyclePosition> {
    let field_count = plan.activation().len();
    plan.activation()
        .iter()
        .copied()
        .enumerate()
        .map(|(activation, field)| {
            (
                field,
                LifecyclePosition {
                    activation,
                    shutdown: field_count - activation - 1,
                },
            )
        })
        .collect()
}

pub(crate) struct PlannedTransitions {
    pub(crate) activation: Vec<MirStaticLifecycleTransition>,
    pub(crate) shutdown: Vec<MirStaticLifecycleTransition>,
}

pub(crate) fn transitions(program: &PlannedMirProgram) -> PlannedTransitions {
    let index = PlannedIndex::new(program);
    let activation = program
        .lifecycle()
        .activation()
        .iter()
        .flat_map(|field| {
            let declaration = index.field(*field);
            let initializer = declaration
                .initializer
                .map(|initializer| index.initializer(initializer));
            let publish = MirStaticLifecycleTransition {
                field: *field,
                kind: if initializer.is_some() {
                    MirStaticLifecycleTransitionKind::PublishLive
                } else {
                    MirStaticLifecycleTransitionKind::ActivateZeroDefault
                },
                span: initializer.map_or(declaration.span, |body| body.publication.span),
            };
            initializer.map_or_else(
                || vec![publish],
                |body| {
                    vec![
                        MirStaticLifecycleTransition {
                            field: *field,
                            kind: MirStaticLifecycleTransitionKind::BeginInitialization,
                            span: body.span,
                        },
                        publish,
                    ]
                },
            )
        })
        .collect();
    let shutdown = program
        .lifecycle()
        .shutdown()
        .flat_map(|field| {
            let span = index.field(field).span;
            [
                MirStaticLifecycleTransition {
                    field,
                    kind: MirStaticLifecycleTransitionKind::BeginDestruction,
                    span,
                },
                MirStaticLifecycleTransition {
                    field,
                    kind: MirStaticLifecycleTransitionKind::FinishDestruction,
                    span,
                },
            ]
        })
        .collect();
    PlannedTransitions {
        activation,
        shutdown,
    }
}

struct PlannedIndex<'mir> {
    fields: BTreeMap<StaticFieldId, &'mir PreliminaryMirStaticField>,
    initializers: BTreeMap<StaticInitializerId, &'mir PreliminaryMirStaticInitializer>,
}

impl<'mir> PlannedIndex<'mir> {
    fn new(program: &'mir PlannedMirProgram) -> Self {
        Self {
            fields: program
                .static_fields()
                .map(|field| (field.field, field))
                .collect(),
            initializers: program
                .static_initializers()
                .map(|initializer| (initializer.id, initializer))
                .collect(),
        }
    }

    fn field(&self, field: StaticFieldId) -> &'mir PreliminaryMirStaticField {
        self.fields
            .get(&field)
            .copied()
            .expect("verified activation field must be inventoried")
    }

    fn initializer(
        &self,
        initializer: StaticInitializerId,
    ) -> &'mir PreliminaryMirStaticInitializer {
        self.initializers
            .get(&initializer)
            .copied()
            .expect("verified explicit activation must have an initializer body")
    }
}
