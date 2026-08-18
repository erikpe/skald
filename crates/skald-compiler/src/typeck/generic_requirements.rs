//! Closed-type queries for contextual class-template requirements.
//!
//! This module is intentionally a facade: it composes the ordinary validators
//! and lifecycle products instead of maintaining a second generic-only type
//! matrix.

#![allow(dead_code)] // Closed specialization is the production consumer of this facade.

use std::cell::OnceCell;

use crate::{
    hir::Type,
    resolve::{
        ClosedGenericRequirementSubject, GenericCapability, GenericRequirement,
        GenericRequirementReason, ResolvedProgram, ResolvedSharedTarget, ResolvedType,
        ResolvedTypeKind,
    },
};

use super::{capabilities::CopyCapabilities, program::lower_type};

#[derive(Clone, Copy, Debug)]
pub(crate) struct GenericRequirementFailure<'requirement> {
    pub(crate) requirement: &'requirement GenericRequirement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailedSpecializationRequirement {
    pub(crate) class: crate::identity::ClassId,
    pub(crate) requirement_index: usize,
    pub(crate) lifecycle_path: Vec<super::capabilities::CopyPathElement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailedInterfaceSpecializationRequirement {
    pub(crate) interface: crate::identity::InterfaceId,
    pub(crate) requirement_index: usize,
}

pub(crate) struct GenericCapabilityQuery<'program> {
    program: &'program ResolvedProgram,
    copy: OnceCell<CopyCapabilities>,
}

impl<'program> GenericCapabilityQuery<'program> {
    pub(crate) fn new(program: &'program ResolvedProgram) -> Self {
        Self {
            program,
            copy: OnceCell::new(),
        }
    }

    /// Evaluate a complete inferred contract after specialization has closed
    /// each structural term. The closing callback belongs to specialization,
    /// which also owns compound-type interning and optional-box targets.
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

    pub(crate) fn supports(
        &self,
        requirement: &GenericRequirement,
        subject: ClosedGenericRequirementSubject,
    ) -> bool {
        if requirement.capability == GenericCapability::SharedTarget {
            return match subject {
                ClosedGenericRequirementSubject::SharedTarget(_) => true,
                ClosedGenericRequirementSubject::Type(kind) => {
                    ResolvedSharedTarget::from_direct_type(kind).is_some()
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
        let ty = self.lower(kind, requirement.origin);
        match requirement.capability {
            GenericCapability::FieldStorage | GenericCapability::StaticStorage => {
                super::program::is_stored_value_type(ty)
            }
            GenericCapability::ValueParameter => super::program::is_stored_value_type(ty),
            GenericCapability::ValueResult => !matches!(ty, Type::Obj | Type::Interface(_)),
            GenericCapability::AliasTarget(_) => {
                super::program::is_supported_alias_type(self.program, ty)
            }
            GenericCapability::OptionalPayload => {
                super::optional_validation::is_optional_payload(kind)
            }
            GenericCapability::ArrayElement => super::arrays::is_array_element(ty),
            GenericCapability::DefaultConstructible => {
                if matches!(
                    requirement.reason,
                    GenericRequirementReason::StaticZeroInitialization { .. }
                ) {
                    super::program::has_zero_default(ty)
                } else {
                    super::arrays::is_default_constructible(self.program, kind)
                }
            }
            GenericCapability::CopyConstructible => self.copy_constructible(kind),
            GenericCapability::Assignable => self.assignable(kind),
            GenericCapability::Destroyable => super::program::is_stored_value_type(ty),
            GenericCapability::SharedTarget => unreachable!("handled above"),
        }
    }

    fn lower(&self, kind: ResolvedTypeKind, span: crate::source::Span) -> Type {
        lower_type(self.program, &ResolvedType { kind, span })
    }

    fn copy_constructible(&self, kind: ResolvedTypeKind) -> bool {
        match kind {
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Shared(_) => true,
            ResolvedTypeKind::Class(class) => self.copy().constructor(class).selected().is_some(),
            ResolvedTypeKind::Array(array) => self.copy().array(array).lifecycle.copy.is_some(),
            ResolvedTypeKind::Optional(optional) => {
                super::optional_types::selected_copy_plan(self.program, self.copy(), optional)
                    .is_some()
            }
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
            ResolvedTypeKind::Class(class) => self.copy().assignment(class).selected().is_some(),
            ResolvedTypeKind::Array(array) => {
                self.copy().array(array).lifecycle.assignment.is_some()
            }
            ResolvedTypeKind::Optional(optional) => {
                super::optional_types::selected_assignment_plan(self.program, self.copy(), optional)
                    .is_some()
            }
            ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_) => false,
        }
    }

    fn copy(&self) -> &CopyCapabilities {
        self.copy
            .get_or_init(|| CopyCapabilities::compute(self.program))
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
            .expect("specialization key references interface template semantics");
        for (requirement_index, (requirement, subject)) in semantics
            .contextual_requirements
            .iter()
            .zip(&specialization.closed_requirements)
            .enumerate()
        {
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
) -> Option<Vec<super::capabilities::CopyPathElement>> {
    let ClosedGenericRequirementSubject::Type(ResolvedTypeKind::Class(class)) = subject else {
        return None;
    };
    let path = match requirement.capability {
        GenericCapability::CopyConstructible => query.copy().constructor_failure(class),
        GenericCapability::Assignable => query.copy().assignment_failure(class),
        GenericCapability::FieldStorage
        | GenericCapability::StaticStorage
        | GenericCapability::ValueParameter
        | GenericCapability::ValueResult
        | GenericCapability::AliasTarget(_)
        | GenericCapability::OptionalPayload
        | GenericCapability::ArrayElement
        | GenericCapability::SharedTarget
        | GenericCapability::DefaultConstructible
        | GenericCapability::Destroyable => None,
    }?;
    Some(path.to_vec())
}

#[cfg(test)]
mod tests;
