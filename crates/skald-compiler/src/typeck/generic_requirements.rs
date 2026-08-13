//! Closed-type queries for contextual class-template requirements.
//!
//! This module is intentionally a facade: it composes the ordinary validators
//! and lifecycle products instead of maintaining a second generic-only type
//! matrix.

#![allow(dead_code)] // Closed specialization is the production consumer of this facade.

use std::cell::OnceCell;

use crate::{
    hir::{HirOptionalTypeTable, Type},
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

pub(crate) struct GenericCapabilityQuery<'program> {
    program: &'program ResolvedProgram,
    copy: OnceCell<CopyCapabilities>,
    optionals: OnceCell<HirOptionalTypeTable>,
}

impl<'program> GenericCapabilityQuery<'program> {
    pub(crate) fn new(program: &'program ResolvedProgram) -> Self {
        Self {
            program,
            copy: OnceCell::new(),
            optionals: OnceCell::new(),
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
            ResolvedTypeKind::Optional(optional) => self
                .optionals()
                .get(optional)
                .is_some_and(|optional| optional.lifecycle.copy.is_some()),
            ResolvedTypeKind::Unit | ResolvedTypeKind::Obj | ResolvedTypeKind::Interface(_) => {
                false
            }
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
            ResolvedTypeKind::Optional(optional) => self
                .optionals()
                .get(optional)
                .is_some_and(|optional| optional.lifecycle.assignment.is_some()),
            ResolvedTypeKind::Unit | ResolvedTypeKind::Obj | ResolvedTypeKind::Interface(_) => {
                false
            }
        }
    }

    fn copy(&self) -> &CopyCapabilities {
        self.copy
            .get_or_init(|| CopyCapabilities::compute(self.program))
    }

    fn optionals(&self) -> &HirOptionalTypeTable {
        self.optionals
            .get_or_init(|| super::optional_types::lower_optional_types(self.program, self.copy()))
    }
}

pub(crate) fn failed_specialization_declaration_requirements(
    program: &ResolvedProgram,
) -> Vec<(crate::identity::ClassId, usize)> {
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
        for (index, (requirement, subject)) in semantics
            .requirements
            .iter()
            .zip(&specialization.closed_requirements)
            .enumerate()
        {
            if !is_declaration_requirement(requirement.reason) {
                continue;
            }
            if !subject.is_some_and(|subject| query.supports(requirement, subject)) {
                failures.push((class, index));
                break;
            }
        }
    }
    failures
}

const fn is_declaration_requirement(reason: GenericRequirementReason) -> bool {
    matches!(
        reason,
        GenericRequirementReason::FieldDeclaration { .. }
            | GenericRequirementReason::StaticFieldDeclaration { .. }
            | GenericRequirementReason::ParameterDeclaration { .. }
            | GenericRequirementReason::MethodResult { .. }
            | GenericRequirementReason::OptionalType
            | GenericRequirementReason::ArrayType
            | GenericRequirementReason::SharedType
            | GenericRequirementReason::StaticZeroInitialization { .. }
    )
}

#[cfg(test)]
mod tests;
