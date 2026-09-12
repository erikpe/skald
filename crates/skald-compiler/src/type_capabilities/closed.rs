//! Capability queries after generic subjects have been closed to resolved types.

use std::cell::OnceCell;

use crate::{
    identity::{ClassId, InterfaceId},
    resolve::{
        ClosedGenericRequirementSubject, GenericCapability, GenericRequirement,
        GenericRequirementReason, ResolvedProgram, ResolvedSharedTarget, ResolvedTypeKind,
    },
};

use super::{resolved_type_category, LifecyclePathElement, ResolvedLifecycleCapabilities};

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct GenericRequirementFailure<'requirement> {
    pub(crate) requirement: &'requirement GenericRequirement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailedSpecializationRequirement {
    pub(crate) class: ClassId,
    pub(crate) requirement_index: usize,
    pub(crate) lifecycle_path: Vec<LifecyclePathElement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailedInterfaceSpecializationRequirement {
    pub(crate) interface: InterfaceId,
    pub(crate) requirement_index: usize,
}

pub(crate) struct GenericCapabilityQuery<'program> {
    program: &'program ResolvedProgram,
    lifecycle: OnceCell<ResolvedLifecycleCapabilities>,
    declarations_complete: bool,
}

impl<'program> GenericCapabilityQuery<'program> {
    pub(crate) fn new(program: &'program ResolvedProgram) -> Self {
        Self {
            program,
            lifecycle: OnceCell::new(),
            declarations_complete: program.generic_specializations.iter().all(|entry| {
                entry
                    .class()
                    .is_none_or(|class| program.class(class).is_some())
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn evaluate<'requirement>(
        &self,
        requirements: &'requirement [GenericRequirement],
        mut close: impl FnMut(&GenericRequirement) -> Option<ClosedGenericRequirementSubject>,
    ) -> Vec<GenericRequirementFailure<'requirement>> {
        requirements
            .iter()
            .filter_map(|requirement| {
                let supported =
                    close(requirement).is_some_and(|subject| self.supports(requirement, subject));
                (!supported).then_some(GenericRequirementFailure { requirement })
            })
            .collect()
    }

    // A rejected materialization retains closed type identities but no generated
    // declarations. Structural requirements still explain the source failure;
    // declaration-dependent queries must wait for a complete candidate graph.
    fn can_validate(&self, requirement: &GenericRequirement) -> bool {
        self.declarations_complete
            || !matches!(
                requirement.capability,
                GenericCapability::DefaultConstructible
                    | GenericCapability::CopyConstructible
                    | GenericCapability::Assignable
            )
    }

    pub(crate) fn supports(
        &self,
        requirement: &GenericRequirement,
        subject: ClosedGenericRequirementSubject,
    ) -> bool {
        if requirement.capability == GenericCapability::SharedTarget {
            return match subject {
                ClosedGenericRequirementSubject::SharedTarget(_) => true,
                ClosedGenericRequirementSubject::Type(kind) => {
                    super::supports_direct_shared_target(resolved_type_category(kind))
                }
            };
        }
        let ClosedGenericRequirementSubject::Type(kind) = subject else {
            return false;
        };
        if matches!(kind, ResolvedTypeKind::Function(_)) {
            return matches!(
                requirement.capability,
                GenericCapability::FieldStorage
                    | GenericCapability::StaticStorage
                    | GenericCapability::ValueParameter
                    | GenericCapability::ValueResult
                    | GenericCapability::CopyConstructible
                    | GenericCapability::Assignable
                    | GenericCapability::Destroyable
            );
        }

        match requirement.capability {
            GenericCapability::FieldStorage
            | GenericCapability::StaticStorage
            | GenericCapability::ValueParameter
            | GenericCapability::Destroyable => {
                super::supports_stored_value(resolved_type_category(kind))
            }
            GenericCapability::ValueResult => {
                super::supports_value_result(resolved_type_category(kind))
            }
            GenericCapability::AliasTarget(_) => super::supports_alias_target(
                resolved_type_category(kind),
                optional_payload_supports_alias(self.program, kind),
            ),
            GenericCapability::OptionalPayload => {
                super::supports_optional_payload(resolved_type_category(kind))
            }
            GenericCapability::ArrayElement => {
                super::supports_array_element(resolved_type_category(kind))
            }
            GenericCapability::DefaultConstructible => {
                if matches!(
                    requirement.reason,
                    GenericRequirementReason::StaticZeroInitialization { .. }
                ) {
                    matches!(
                        kind,
                        ResolvedTypeKind::I64
                            | ResolvedTypeKind::U64
                            | ResolvedTypeKind::U8
                            | ResolvedTypeKind::F64
                            | ResolvedTypeKind::Bool
                            | ResolvedTypeKind::Optional(_)
                            | ResolvedTypeKind::Array(_)
                    )
                } else {
                    is_default_constructible(self.program, kind)
                }
            }
            GenericCapability::CopyConstructible => self.copy_constructible(kind),
            GenericCapability::Assignable => self.assignable(kind),
            GenericCapability::SharedTarget => unreachable!("handled above"),
        }
    }

    fn copy_constructible(&self, kind: ResolvedTypeKind) -> bool {
        match kind {
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Shared(_) => true,
            ResolvedTypeKind::Class(class) => self.lifecycle().constructor(class),
            ResolvedTypeKind::Array(array) => self.lifecycle().array_copy(array),
            ResolvedTypeKind::Optional(optional) => self.optional_lifecycle(optional, true),
            ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_) => false,
        }
    }

    fn assignable(&self, kind: ResolvedTypeKind) -> bool {
        match kind {
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Shared(_) => true,
            ResolvedTypeKind::Class(class) => self.lifecycle().assignment(class),
            ResolvedTypeKind::Array(array) => self.lifecycle().array_assignment(array),
            ResolvedTypeKind::Optional(optional) => self.optional_lifecycle(optional, false),
            ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_) => false,
        }
    }

    fn optional_lifecycle(
        &self,
        mut optional: crate::identity::OptionalTypeId,
        copy: bool,
    ) -> bool {
        loop {
            match self
                .program
                .optional_types
                .get(optional)
                .expect("optional identity must name resolved metadata")
                .payload
                .kind
            {
                ResolvedTypeKind::I64
                | ResolvedTypeKind::U64
                | ResolvedTypeKind::U8
                | ResolvedTypeKind::F64
                | ResolvedTypeKind::Bool
                | ResolvedTypeKind::Shared(_) => return true,
                ResolvedTypeKind::Class(class) => {
                    return self.lifecycle().constructor(class)
                        && (copy || self.lifecycle().assignment(class));
                }
                ResolvedTypeKind::Array(array) => {
                    return if copy {
                        self.lifecycle().array_copy(array)
                    } else {
                        self.lifecycle().array_assignment(array)
                    };
                }
                ResolvedTypeKind::Optional(nested) => optional = nested,
                ResolvedTypeKind::Unit
                | ResolvedTypeKind::Obj
                | ResolvedTypeKind::Interface(_)
                | ResolvedTypeKind::Function(_) => return false,
            }
        }
    }

    fn lifecycle(&self) -> &ResolvedLifecycleCapabilities {
        self.lifecycle
            .get_or_init(|| ResolvedLifecycleCapabilities::compute(self.program))
    }
}

pub(crate) fn failed_specialization_requirements(
    program: &ResolvedProgram,
) -> Vec<FailedSpecializationRequirement> {
    let query = GenericCapabilityQuery::new(program);
    let mut failures = Vec::new();
    for specialization in program.generic_specializations.iter() {
        let crate::resolve::GenericSpecializationState::Complete(class) = specialization.state
        else {
            continue;
        };
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        for (requirement_index, (requirement, subject)) in semantics
            .requirements
            .iter()
            .zip(&specialization.closed_requirements)
            .enumerate()
        {
            if !query.can_validate(requirement) {
                continue;
            }
            if !subject.is_some_and(|subject| query.supports(requirement, subject)) {
                let lifecycle_path = subject
                    .and_then(|subject| requirement_failure_path(&query, requirement, subject))
                    .unwrap_or_default();
                failures.push(FailedSpecializationRequirement {
                    class,
                    requirement_index,
                    lifecycle_path,
                });
                break;
            }
        }
    }
    failures
}

pub(crate) fn failed_interface_specialization_requirements(
    program: &ResolvedProgram,
) -> Vec<FailedInterfaceSpecializationRequirement> {
    let query = GenericCapabilityQuery::new(program);
    let mut failures = Vec::new();
    for specialization in program.generic_interface_specializations.iter() {
        let crate::resolve::GenericInterfaceSpecializationState::Complete(interface) =
            specialization.state
        else {
            continue;
        };
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface-template semantics");
        for (requirement_index, (requirement, subject)) in semantics
            .contextual_requirements
            .iter()
            .zip(&specialization.closed_requirements)
            .enumerate()
        {
            if !query.can_validate(requirement) {
                continue;
            }
            if !subject.is_some_and(|subject| query.supports(requirement, subject)) {
                failures.push(FailedInterfaceSpecializationRequirement {
                    interface,
                    requirement_index,
                });
                break;
            }
        }
    }
    failures
}

fn requirement_failure_path(
    query: &GenericCapabilityQuery<'_>,
    requirement: &GenericRequirement,
    subject: ClosedGenericRequirementSubject,
) -> Option<Vec<LifecyclePathElement>> {
    let ClosedGenericRequirementSubject::Type(ResolvedTypeKind::Class(class)) = subject else {
        return None;
    };
    let path = match requirement.capability {
        GenericCapability::CopyConstructible => query.lifecycle().constructor_failure(class),
        GenericCapability::Assignable => query.lifecycle().assignment_failure(class),
        _ => None,
    }?;
    Some(path.to_vec())
}

fn optional_payload_supports_alias(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    let ResolvedTypeKind::Optional(optional) = kind else {
        return false;
    };
    let payload = program
        .optional_types
        .get(optional)
        .expect("optional identity must name resolved metadata")
        .payload
        .kind;
    super::supports_optional_payload(resolved_type_category(payload))
}

fn is_default_constructible(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    match kind {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
        | ResolvedTypeKind::Optional(_)
        | ResolvedTypeKind::Array(_) => true,
        ResolvedTypeKind::Class(class) => has_unique_zero_argument_initializer(program, class),
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class)) => {
            has_unique_zero_argument_initializer(program, class)
        }
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(_)) => true,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::OptionalBox(target)) => program
            .optional_box_types
            .get(target)
            .is_some_and(|metadata| metadata.optional.is_some()),
        ResolvedTypeKind::Shared(
            ResolvedSharedTarget::Obj | ResolvedSharedTarget::Interface(_),
        )
        | ResolvedTypeKind::Unit
        | ResolvedTypeKind::Obj
        | ResolvedTypeKind::Interface(_)
        | ResolvedTypeKind::Function(_) => false,
    }
}

fn has_unique_zero_argument_initializer(program: &ResolvedProgram, class: ClassId) -> bool {
    let Some(class) = program.class(class) else {
        return false;
    };
    let mut candidates = class
        .initializers
        .iter()
        .filter(|initializer| initializer.parameters.is_empty());
    candidates.next().is_some() && candidates.next().is_none()
}

#[cfg(test)]
#[path = "tests/closed.rs"]
mod tests;
