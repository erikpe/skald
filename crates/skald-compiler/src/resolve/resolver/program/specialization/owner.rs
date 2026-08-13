//! Deterministic specialization cache, activation stack, and provenance.

use std::collections::{HashMap, HashSet};

use super::*;

pub(super) struct SpecializationOwner<'semantic, 'interner, 'diagnostics> {
    semantics: &'semantic ResolvedClassTemplateSemanticTable,
    pub(super) templates: &'semantic ResolvedClassTemplateTable,
    pub(super) interner: &'interner mut ResolvedTypeInterner,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
    entries: Vec<GenericSpecialization>,
    indices: HashMap<GenericClassInstanceKey, usize>,
    active: Vec<usize>,
    rejected_edges: HashSet<(usize, GenericClassInstanceKey)>,
    specialization_failures: usize,
    next_class: usize,
}

impl<'semantic, 'interner, 'diagnostics> SpecializationOwner<'semantic, 'interner, 'diagnostics> {
    pub(super) fn new(
        semantics: &'semantic ResolvedClassTemplateSemanticTable,
        templates: &'semantic ResolvedClassTemplateTable,
        interner: &'interner mut ResolvedTypeInterner,
        diagnostics: &'diagnostics mut Diagnostics,
        ordinary_class_count: usize,
    ) -> Self {
        Self {
            semantics,
            templates,
            interner,
            diagnostics,
            entries: Vec::new(),
            indices: HashMap::new(),
            active: Vec::new(),
            rejected_edges: HashSet::new(),
            specialization_failures: 0,
            next_class: ordinary_class_count,
        }
    }

    pub(super) fn finish(self) -> GenericSpecializationTable {
        debug_assert!(self.active.is_empty());
        GenericSpecializationTable::new(self.entries)
    }

    pub(super) fn request(
        &mut self,
        template: ClassTemplateId,
        arguments: Vec<ResolvedTypeKind>,
        origin: GenericApplicationOrigin,
    ) -> Option<ClassId> {
        let key = GenericClassInstanceKey {
            template,
            arguments,
        };
        let existing = self.indices.get(&key).copied();
        if let Some(index) = existing.filter(|index| self.active.contains(index)) {
            self.add_origin(index, origin);
            return self.entries[index].class();
        }

        if let Some(conflict) = self.active.iter().copied().find(|index| {
            let active = &self.entries[*index].key;
            active.template == template && active != &key
        }) {
            return self.reject_transformed_recursion(key, origin, conflict);
        }

        if let Some(index) = existing {
            self.add_origin(index, origin);
            return match self.entries[index].state {
                GenericSpecializationState::Requested => self.activate(index),
                GenericSpecializationState::InProgress(class)
                | GenericSpecializationState::Complete(class) => Some(class),
                GenericSpecializationState::Failed { .. } => {
                    self.specialization_failures += 1;
                    None
                }
            };
        }

        let template_span = self
            .templates
            .get(template)
            .expect("specialization requests reference collected templates")
            .span;
        let index = self.entries.len();
        self.indices.insert(key.clone(), index);
        self.entries.push(GenericSpecialization {
            key,
            state: GenericSpecializationState::Requested,
            transitions: vec![GenericSpecializationTransition::Requested],
            provenance: GenericSpecializationProvenance {
                template_span,
                origins: vec![origin],
                recursion_path: Vec::new(),
            },
            closed_type_uses: Vec::new(),
            closed_requirements: Vec::new(),
        });
        self.activate(index)
    }

    fn activate(&mut self, index: usize) -> Option<ClassId> {
        let class = ClassId::new(self.next_class);
        self.next_class += 1;
        self.entries[index].state = GenericSpecializationState::InProgress(class);
        self.entries[index]
            .transitions
            .push(GenericSpecializationTransition::InProgress(class));
        self.active.push(index);
        let failures_before_closing = self.specialization_failures;

        let key = self.entries[index].key.clone();
        let semantics = self
            .semantics
            .get(key.template)
            .expect("specialization key references template semantics");
        let terms = semantics
            .type_uses
            .iter()
            .map(|type_use| type_use.type_term.clone())
            .collect::<Vec<_>>();
        let requirements = semantics.requirements.clone();
        let closed_type_uses = terms
            .iter()
            .map(|term| {
                // A term can be contextually invalid for these arguments (for
                // example `shared T` with `T = i64`). Contextual requirement
                // validation diagnoses that use; identity discovery fails only
                // when a nested specialization itself fails.
                self.close_template_type(term, key.template, &key.arguments)
            })
            .collect();
        let closed_requirements = requirements
            .iter()
            .map(|requirement| {
                if requirement.capability == GenericCapability::SharedTarget {
                    self.close_template_shared_target(
                        &requirement.type_term,
                        key.template,
                        &key.arguments,
                    )
                    .map(ClosedGenericRequirementSubject::SharedTarget)
                } else {
                    self.close_template_type(&requirement.type_term, key.template, &key.arguments)
                        .map(ClosedGenericRequirementSubject::Type)
                }
            })
            .collect();
        self.entries[index].closed_type_uses = closed_type_uses;
        self.entries[index].closed_requirements = closed_requirements;
        let valid = self.specialization_failures == failures_before_closing;

        let popped = self.active.pop();
        debug_assert_eq!(popped, Some(index));
        if valid {
            self.entries[index].state = GenericSpecializationState::Complete(class);
            self.entries[index]
                .transitions
                .push(GenericSpecializationTransition::Complete(class));
            Some(class)
        } else {
            self.entries[index].state = GenericSpecializationState::Failed {
                reserved_class: Some(class),
            };
            self.entries[index]
                .transitions
                .push(GenericSpecializationTransition::Failed {
                    reserved_class: Some(class),
                });
            self.specialization_failures += 1;
            None
        }
    }

    fn reject_transformed_recursion(
        &mut self,
        key: GenericClassInstanceKey,
        origin: GenericApplicationOrigin,
        conflict: usize,
    ) -> Option<ClassId> {
        self.specialization_failures += 1;
        let template = self
            .templates
            .get(key.template)
            .expect("recursive request references a collected template");
        let mut recursion_path = self
            .active
            .iter()
            .map(|index| self.entries[*index].key.clone())
            .collect::<Vec<_>>();
        recursion_path.push(key.clone());
        if self.entries[conflict].provenance.recursion_path.is_empty() {
            self.entries[conflict].provenance.recursion_path = recursion_path.clone();
        }

        let is_new_rejection = self.rejected_edges.insert((conflict, key.clone()));
        if !is_new_rejection {
            return None;
        }

        let application_origin = self.entries[*self
            .active
            .first()
            .expect("transformed recursion has an active root")]
        .provenance
        .origins
        .first()
        .expect("active root specialization has an origin")
        .span;
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::NON_TERMINATING_GENERIC_SPECIALIZATION,
                format!(
                    "recursive application of `{}` changes its type arguments",
                    template.name
                ),
            )
            .with_primary_label(
                application_origin,
                "this application produces a non-terminating family of types",
            )
            .with_secondary_label(
                origin.span,
                "substitution recursively requests a different closed argument sequence",
            )
            .with_secondary_label(template.name_span, "template declared here"),
        );
        None
    }

    fn add_origin(&mut self, index: usize, origin: GenericApplicationOrigin) {
        if !self.entries[index].provenance.origins.contains(&origin) {
            self.entries[index].provenance.origins.push(origin);
        }
    }
}
