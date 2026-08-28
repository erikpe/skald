//! Deterministic cross-kind specialization cache, work stack, and provenance.

use std::collections::{HashMap, HashSet};

use super::closed_types::TypeClosingEnvironment;
use super::requests::GenericApplicationDiscovery;
use super::*;

pub(super) struct SpecializationCoordinator<'semantic, 'interner, 'diagnostics> {
    pub(super) class_semantics: &'semantic ResolvedClassTemplateSemanticTable,
    pub(super) interface_semantics: &'semantic ResolvedInterfaceTemplateSemanticTable,
    pub(super) class_templates: &'semantic ResolvedClassTemplateTable,
    pub(super) interface_templates: &'semantic ResolvedInterfaceTemplateTable,
    pub(super) interner: &'interner mut ResolvedTypeInterner,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
    class_entries: Vec<GenericSpecialization>,
    class_indices: HashMap<GenericClassInstanceKey, usize>,
    interface_entries: Vec<GenericInterfaceSpecialization>,
    interface_indices: HashMap<GenericInterfaceInstanceKey, usize>,
    active: Vec<GenericSpecializationKey>,
    rejected_edges: HashSet<(GenericSpecializationKey, GenericSpecializationKey)>,
    specialization_failures: usize,
    next_class: usize,
    next_interface: usize,
    range_template: Option<ClassTemplateId>,
}

impl<'semantic, 'interner, 'diagnostics>
    SpecializationCoordinator<'semantic, 'interner, 'diagnostics>
{
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        class_semantics: &'semantic ResolvedClassTemplateSemanticTable,
        interface_semantics: &'semantic ResolvedInterfaceTemplateSemanticTable,
        class_templates: &'semantic ResolvedClassTemplateTable,
        interface_templates: &'semantic ResolvedInterfaceTemplateTable,
        interner: &'interner mut ResolvedTypeInterner,
        diagnostics: &'diagnostics mut Diagnostics,
        ordinary_class_count: usize,
        ordinary_interface_count: usize,
        range_template: Option<ClassTemplateId>,
    ) -> Self {
        Self {
            class_semantics,
            interface_semantics,
            class_templates,
            interface_templates,
            interner,
            diagnostics,
            class_entries: Vec::new(),
            class_indices: HashMap::new(),
            interface_entries: Vec::new(),
            interface_indices: HashMap::new(),
            active: Vec::new(),
            rejected_edges: HashSet::new(),
            specialization_failures: 0,
            next_class: ordinary_class_count,
            next_interface: ordinary_interface_count,
            range_template,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resume(
        class_semantics: &'semantic ResolvedClassTemplateSemanticTable,
        interface_semantics: &'semantic ResolvedInterfaceTemplateSemanticTable,
        class_templates: &'semantic ResolvedClassTemplateTable,
        interface_templates: &'semantic ResolvedInterfaceTemplateTable,
        interner: &'interner mut ResolvedTypeInterner,
        diagnostics: &'diagnostics mut Diagnostics,
        ordinary_class_count: usize,
        ordinary_interface_count: usize,
        range_template: Option<ClassTemplateId>,
        discovery: GenericApplicationDiscovery,
    ) -> Self {
        let class_entries = discovery.class_specializations.into_entries();
        let interface_entries = discovery.interface_specializations.into_entries();
        let class_indices = class_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.key.clone(), index))
            .collect();
        let interface_indices = interface_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.key.clone(), index))
            .collect();
        let next_class = class_entries
            .iter()
            .filter_map(GenericSpecialization::class)
            .map(|class| class.index() + 1)
            .fold(ordinary_class_count, usize::max);
        let next_interface = interface_entries
            .iter()
            .filter_map(GenericInterfaceSpecialization::interface)
            .map(|interface| interface.index() + 1)
            .fold(ordinary_interface_count, usize::max);
        Self {
            class_semantics,
            interface_semantics,
            class_templates,
            interface_templates,
            interner,
            diagnostics,
            class_entries,
            class_indices,
            interface_entries,
            interface_indices,
            active: Vec::new(),
            rejected_edges: HashSet::new(),
            specialization_failures: 0,
            next_class,
            next_interface,
            range_template,
        }
    }

    pub(super) fn finish(mut self) -> GenericApplicationDiscovery {
        debug_assert!(self.active.is_empty());
        for entry in &mut self.interface_entries {
            entry.provenance.origins.sort_by_key(|origin| {
                (
                    origin.module.index(),
                    origin.span.range().start(),
                    origin.span.range().end(),
                )
            });
        }
        GenericApplicationDiscovery {
            class_specializations: GenericSpecializationTable::new(self.class_entries),
            interface_specializations: GenericInterfaceSpecializationTable::new(
                self.interface_entries,
            ),
        }
    }

    pub(super) fn request_class(
        &mut self,
        template: ClassTemplateId,
        arguments: Vec<ResolvedTypeKind>,
        origin: GenericApplicationOrigin,
    ) -> Option<ClassId> {
        let key = GenericClassInstanceKey {
            template,
            arguments,
        };
        let work_key = GenericSpecializationKey::Class(key.clone());
        let existing = self.class_indices.get(&key).copied();
        if self.active.contains(&work_key) {
            let index = existing.expect("active class specialization is cached");
            self.add_class_origin(index, origin);
            return self.class_entries[index].class();
        }
        if let Some(conflict) = self
            .active
            .iter()
            .find(|active| match active {
                GenericSpecializationKey::Class(active) => {
                    active.template == template && active != &key
                }
                GenericSpecializationKey::Interface(_) => false,
            })
            .cloned()
        {
            return self.reject_class_recursion(key, origin, conflict);
        }
        if let Some(index) = existing {
            self.add_class_origin(index, origin);
            return match self.class_entries[index].state {
                GenericSpecializationState::Requested => self.activate_class(index),
                GenericSpecializationState::InProgress(class)
                | GenericSpecializationState::Complete(class) => Some(class),
                GenericSpecializationState::Failed { .. } => {
                    self.specialization_failures += 1;
                    None
                }
            };
        }

        let template_span = self
            .class_templates
            .get(template)
            .expect("specialization requests reference collected templates")
            .span;
        let index = self.class_entries.len();
        self.class_indices.insert(key.clone(), index);
        self.class_entries.push(GenericSpecialization {
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
            closed_interface_claims: Vec::new(),
            closed_interface_bounds: Vec::new(),
            closed_bound_members: Vec::new(),
            closed_operator_selections: Vec::new(),
            closed_iteration_selections: Vec::new(),
            closed_range_selections: Vec::new(),
        });
        self.activate_class(index)
    }

    pub(super) fn request_interface(
        &mut self,
        template: InterfaceTemplateId,
        arguments: Vec<ResolvedTypeKind>,
        origin: GenericInterfaceApplicationOrigin,
    ) -> Option<InterfaceId> {
        let key = GenericInterfaceInstanceKey {
            template,
            arguments,
        };
        let work_key = GenericSpecializationKey::Interface(key.clone());
        let existing = self.interface_indices.get(&key).copied();
        if self.active.contains(&work_key) {
            let index = existing.expect("active interface specialization is cached");
            self.add_interface_origin(index, origin);
            return self.interface_entries[index].interface();
        }
        if let Some(conflict) = self
            .active
            .iter()
            .find(|active| match active {
                GenericSpecializationKey::Interface(active) => {
                    active.template == template && active != &key
                }
                GenericSpecializationKey::Class(_) => false,
            })
            .cloned()
        {
            return self.reject_interface_recursion(key, origin, conflict);
        }
        if let Some(index) = existing {
            self.add_interface_origin(index, origin);
            return match self.interface_entries[index].state {
                GenericInterfaceSpecializationState::Requested => self.activate_interface(index),
                GenericInterfaceSpecializationState::InProgress(interface)
                | GenericInterfaceSpecializationState::Complete(interface) => Some(interface),
                GenericInterfaceSpecializationState::Failed { .. } => {
                    self.specialization_failures += 1;
                    None
                }
            };
        }

        let template_span = self
            .interface_templates
            .get(template)
            .expect("specialization requests reference collected templates")
            .span;
        let index = self.interface_entries.len();
        self.interface_indices.insert(key.clone(), index);
        self.interface_entries.push(GenericInterfaceSpecialization {
            key,
            state: GenericInterfaceSpecializationState::Requested,
            transitions: vec![GenericInterfaceSpecializationTransition::Requested],
            provenance: GenericInterfaceSpecializationProvenance {
                template_span,
                origins: vec![origin],
                recursion_path: Vec::new(),
            },
            requirement_mappings: Vec::new(),
            closed_type_uses: Vec::new(),
            closed_requirements: Vec::new(),
            closed_interface_bounds: Vec::new(),
        });
        self.activate_interface(index)
    }

    fn activate_class(&mut self, index: usize) -> Option<ClassId> {
        let class = ClassId::new(self.next_class);
        self.next_class += 1;
        self.class_entries[index].state = GenericSpecializationState::InProgress(class);
        self.class_entries[index]
            .transitions
            .push(GenericSpecializationTransition::InProgress(class));
        let key = self.class_entries[index].key.clone();
        let work_key = GenericSpecializationKey::Class(key.clone());
        self.active.push(work_key.clone());
        let failures_before = self.specialization_failures;

        let semantics = self
            .class_semantics
            .get(key.template)
            .expect("specialization key references template semantics");
        let terms = semantics
            .type_uses
            .iter()
            .map(|use_| use_.type_term.clone())
            .collect::<Vec<_>>();
        let requirements = semantics.requirements.clone();
        let interface_claims = semantics
            .implemented_interfaces
            .iter()
            .map(|claim| (claim.interface.clone(), claim.span))
            .collect::<Vec<_>>();
        let interface_bounds = semantics
            .bounds
            .iter()
            .map(|bound| (bound.interface.clone(), bound.interface_span))
            .collect::<Vec<_>>();
        let environment = TypeClosingEnvironment::class(
            key.template,
            &key.arguments,
            self.class_templates
                .get(key.template)
                .expect("template exists")
                .module,
        );
        let closed_interface_claims = interface_claims
            .iter()
            .map(|(interface, span)| self.close_template_interface(interface, *span, environment))
            .collect::<Vec<_>>();
        let closed_interface_bounds = interface_bounds
            .iter()
            .map(|(interface, span)| self.close_template_interface(interface, *span, environment))
            .collect::<Vec<_>>();
        let closed_type_uses = terms
            .iter()
            .map(|term| self.close_template_type(term, environment))
            .collect();
        let closed_requirements = requirements
            .iter()
            .map(|requirement| {
                if requirement.capability == GenericCapability::SharedTarget {
                    self.close_template_shared_target(&requirement.type_term, environment)
                        .map(ClosedGenericRequirementSubject::SharedTarget)
                } else {
                    self.close_template_type(&requirement.type_term, environment)
                        .map(ClosedGenericRequirementSubject::Type)
                }
            })
            .collect();
        self.class_entries[index].closed_type_uses = closed_type_uses;
        self.class_entries[index].closed_requirements = closed_requirements;
        self.class_entries[index].closed_interface_claims = closed_interface_claims;
        self.class_entries[index].closed_interface_bounds = closed_interface_bounds;
        self.class_entries[index].closed_bound_members = vec![None; semantics.selections.len()];
        self.class_entries[index].closed_operator_selections =
            vec![None; semantics.selections.len()];
        self.class_entries[index].closed_iteration_selections =
            vec![None; semantics.selections.len()];
        self.class_entries[index].closed_range_selections = vec![None; semantics.selections.len()];
        for (selection_index, selection) in semantics.selections.iter().enumerate() {
            let ResolvedTemplateSelection::Range { endpoint, span, .. } = selection else {
                continue;
            };
            let Some(range_template) = self.range_template else {
                continue;
            };
            let Some(endpoint) = self.close_template_type(endpoint, environment) else {
                continue;
            };
            let range = self.request_class(
                range_template,
                vec![endpoint],
                GenericApplicationOrigin {
                    module: self
                        .class_templates
                        .get(key.template)
                        .expect("template exists")
                        .module,
                    span: *span,
                },
            );
            self.class_entries[index].closed_range_selections[selection_index] = range;
        }
        let valid = self.specialization_failures == failures_before;
        debug_assert_eq!(self.active.pop(), Some(work_key));
        if valid {
            self.class_entries[index].state = GenericSpecializationState::Complete(class);
            self.class_entries[index]
                .transitions
                .push(GenericSpecializationTransition::Complete(class));
            Some(class)
        } else {
            self.class_entries[index].state = GenericSpecializationState::Failed {
                reserved_class: Some(class),
            };
            self.class_entries[index]
                .transitions
                .push(GenericSpecializationTransition::Failed {
                    reserved_class: Some(class),
                });
            self.specialization_failures += 1;
            None
        }
    }

    fn activate_interface(&mut self, index: usize) -> Option<InterfaceId> {
        let interface = InterfaceId::new(self.next_interface);
        self.next_interface += 1;
        self.interface_entries[index].state =
            GenericInterfaceSpecializationState::InProgress(interface);
        self.interface_entries[index].transitions.push(
            GenericInterfaceSpecializationTransition::InProgress(interface),
        );
        let key = self.interface_entries[index].key.clone();
        let work_key = GenericSpecializationKey::Interface(key.clone());
        self.active.push(work_key.clone());
        let failures_before = self.specialization_failures;
        let semantics = self
            .interface_semantics
            .get(key.template)
            .expect("specialization key references interface semantics");
        let terms = semantics
            .type_uses
            .iter()
            .map(|use_| use_.type_term.clone())
            .collect::<Vec<_>>();
        let requirements = semantics.contextual_requirements.clone();
        let bounds = semantics
            .bounds
            .iter()
            .map(|bound| (bound.interface.clone(), bound.interface_span))
            .collect::<Vec<_>>();
        let environment = TypeClosingEnvironment::interface(
            key.template,
            &key.arguments,
            self.interface_templates
                .get(key.template)
                .expect("template exists")
                .module,
        );
        let closed_type_uses = terms
            .iter()
            .map(|term| self.close_template_type(term, environment))
            .collect();
        let closed_requirements = requirements
            .iter()
            .map(|requirement| {
                if requirement.capability == GenericCapability::SharedTarget {
                    self.close_template_shared_target(&requirement.type_term, environment)
                        .map(ClosedGenericRequirementSubject::SharedTarget)
                } else {
                    self.close_template_type(&requirement.type_term, environment)
                        .map(ClosedGenericRequirementSubject::Type)
                }
            })
            .collect();
        let closed_interface_bounds = bounds
            .iter()
            .map(|(bound, span)| self.close_template_interface(bound, *span, environment))
            .collect();
        self.interface_entries[index].closed_type_uses = closed_type_uses;
        self.interface_entries[index].closed_requirements = closed_requirements;
        self.interface_entries[index].closed_interface_bounds = closed_interface_bounds;
        let valid = self.specialization_failures == failures_before;
        debug_assert_eq!(self.active.pop(), Some(work_key));
        if valid {
            self.interface_entries[index].state =
                GenericInterfaceSpecializationState::Complete(interface);
            self.interface_entries[index].transitions.push(
                GenericInterfaceSpecializationTransition::Complete(interface),
            );
            Some(interface)
        } else {
            self.interface_entries[index].state = GenericInterfaceSpecializationState::Failed {
                reserved_interface: interface,
            };
            self.interface_entries[index].transitions.push(
                GenericInterfaceSpecializationTransition::Failed {
                    reserved_interface: interface,
                },
            );
            self.specialization_failures += 1;
            None
        }
    }

    fn reject_class_recursion(
        &mut self,
        key: GenericClassInstanceKey,
        origin: GenericApplicationOrigin,
        conflict: GenericSpecializationKey,
    ) -> Option<ClassId> {
        self.specialization_failures += 1;
        let attempted = GenericSpecializationKey::Class(key.clone());
        let mut path = self.active.clone();
        path.push(attempted.clone());
        if let GenericSpecializationKey::Class(conflict_key) = &conflict {
            let index = self.class_indices[conflict_key];
            if self.class_entries[index]
                .provenance
                .recursion_path
                .is_empty()
            {
                self.class_entries[index].provenance.recursion_path = path;
            }
        }
        if self.rejected_edges.insert((conflict, attempted)) {
            let template = self
                .class_templates
                .get(key.template)
                .expect("template exists");
            let (name, name_span) = (template.name.clone(), template.name_span);
            self.push_recursion_diagnostic(&name, name_span, self.root_origin_span(), origin.span);
        }
        None
    }

    fn reject_interface_recursion(
        &mut self,
        key: GenericInterfaceInstanceKey,
        origin: GenericInterfaceApplicationOrigin,
        conflict: GenericSpecializationKey,
    ) -> Option<InterfaceId> {
        self.specialization_failures += 1;
        if let Some(index) = self.interface_indices.get(&key).copied() {
            self.add_interface_origin(index, origin);
            return None;
        }
        let interface = InterfaceId::new(self.next_interface);
        self.next_interface += 1;
        let attempted = GenericSpecializationKey::Interface(key.clone());
        let mut path = self.active.clone();
        path.push(attempted.clone());
        let template = self
            .interface_templates
            .get(key.template)
            .expect("template exists");
        let (template_span, name, name_span) =
            (template.span, template.name.clone(), template.name_span);
        let index = self.interface_entries.len();
        self.interface_indices.insert(key.clone(), index);
        self.interface_entries.push(GenericInterfaceSpecialization {
            key,
            state: GenericInterfaceSpecializationState::Failed {
                reserved_interface: interface,
            },
            transitions: vec![
                GenericInterfaceSpecializationTransition::Requested,
                GenericInterfaceSpecializationTransition::InProgress(interface),
                GenericInterfaceSpecializationTransition::Failed {
                    reserved_interface: interface,
                },
            ],
            provenance: GenericInterfaceSpecializationProvenance {
                template_span,
                origins: vec![origin],
                recursion_path: path.clone(),
            },
            requirement_mappings: Vec::new(),
            closed_type_uses: Vec::new(),
            closed_requirements: Vec::new(),
            closed_interface_bounds: Vec::new(),
        });
        if let GenericSpecializationKey::Interface(conflict_key) = &conflict {
            let conflict_index = self.interface_indices[conflict_key];
            if self.interface_entries[conflict_index]
                .provenance
                .recursion_path
                .is_empty()
            {
                self.interface_entries[conflict_index]
                    .provenance
                    .recursion_path = path;
            }
        }
        if self.rejected_edges.insert((conflict, attempted)) {
            self.push_recursion_diagnostic(&name, name_span, self.root_origin_span(), origin.span);
        }
        None
    }

    fn push_recursion_diagnostic(&mut self, name: &str, name_span: Span, root: Span, nested: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::NON_TERMINATING_GENERIC_SPECIALIZATION,
                format!("recursive application of `{name}` changes its type arguments"),
            )
            .with_primary_label(
                root,
                "this application produces a non-terminating family of types",
            )
            .with_secondary_label(
                nested,
                "substitution recursively requests a different closed argument sequence",
            )
            .with_secondary_label(name_span, "template declared here"),
        );
    }

    fn root_origin_span(&self) -> Span {
        match self.active.first().expect("recursion has an active root") {
            GenericSpecializationKey::Class(key) => {
                self.class_entries[self.class_indices[key]]
                    .provenance
                    .origins[0]
                    .span
            }
            GenericSpecializationKey::Interface(key) => {
                self.interface_entries[self.interface_indices[key]]
                    .provenance
                    .origins[0]
                    .span
            }
        }
    }

    fn add_class_origin(&mut self, index: usize, origin: GenericApplicationOrigin) {
        if !self.class_entries[index]
            .provenance
            .origins
            .contains(&origin)
        {
            self.class_entries[index].provenance.origins.push(origin);
        }
    }

    fn add_interface_origin(&mut self, index: usize, origin: GenericInterfaceApplicationOrigin) {
        if !self.interface_entries[index]
            .provenance
            .origins
            .contains(&origin)
        {
            self.interface_entries[index]
                .provenance
                .origins
                .push(origin);
        }
    }
}
