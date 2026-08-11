//! Program-wide validation of canonical optional identities and lifecycle plans.

use crate::mir::{
    MirOptionalAssignmentPlan as Assignment, MirOptionalBoundaryPlan as Boundary,
    MirOptionalCheckedAccess as Access, MirOptionalCleanupPlan as Cleanup,
    MirOptionalCopyPlan as Copy, MirOptionalInitializationPlan as Initialization,
    MirOptionalInjectionPlan as Injection, MirOptionalPresencePlan as Presence,
    MirOptionalRepresentation as Representation, MirOptionalStorage as Storage,
    MirOptionalUnwrapPlan as Unwrap, MirType,
};

use super::super::context::Verifier;

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_declarations(&mut self) {
        for (index, optional) in self.program.optional_types.iter().enumerate() {
            if optional.id.index() != index {
                self.program_error(format!(
                    "optional type table index {index} contains {}",
                    optional.id
                ));
            }
            let storage_valid = match optional.storage {
                Storage::Scalar => optional.primitive().is_some(),
                Storage::InlineClass(class) => {
                    optional.payload == MirType::Class(class) && self.program.class(class).is_some()
                }
                Storage::InlineArray(array) => {
                    optional.payload == MirType::Array(array)
                        && self.program.array_type(array).is_some()
                }
                Storage::SharedOwner(target) => {
                    optional.payload == MirType::Shared(target)
                        && match target {
                            crate::mir::MirSharedTarget::Obj => true,
                            crate::mir::MirSharedTarget::Class(class) => {
                                self.program.class(class).is_some()
                            }
                            crate::mir::MirSharedTarget::Interface(interface) => {
                                self.program.interface(interface).is_some()
                            }
                            crate::mir::MirSharedTarget::Array(array) => {
                                self.program.array_type(array).is_some()
                            }
                            crate::mir::MirSharedTarget::OptionalBox(target) => {
                                self.program.optional_box_type(target).is_some()
                            }
                        }
                }
                Storage::Nested(nested) => {
                    nested.index() < optional.id.index()
                        && optional.payload == MirType::Optional(nested)
                        && self.program.optional_type(nested).is_some()
                }
            };
            if !storage_valid {
                self.program_error(format!(
                    "optional {} has inconsistent payload storage metadata",
                    optional.id
                ));
            }
            let plan_valid = match optional.storage {
                Storage::Scalar => {
                    optional.representation == Representation::TaggedPayload
                        && optional.lifecycle.initialization
                            == Initialization::TaggedAbsentOrPresent
                        && optional.lifecycle.injection == Injection::StoreScalar
                        && optional.lifecycle.copy == Some(Copy::Trivial)
                        && optional.lifecycle.assignment == Some(Assignment::Trivial)
                        && optional.lifecycle.cleanup == Cleanup::Trivial
                        && optional.lifecycle.presence == Presence::OuterTag
                        && optional.lifecycle.unwrap == Unwrap::ExtractScalar
                        && optional.checked_access == Access::Value
                }
                Storage::InlineClass(class) => {
                    optional.representation == Representation::TaggedPayload
                        && optional.lifecycle.initialization
                            == Initialization::TaggedAbsentOrPresent
                        && optional.lifecycle.injection == Injection::ConstructClass(class)
                        && self.program.class(class).is_some_and(|declaration| {
                            optional.lifecycle.copy
                                == declaration
                                    .copy_constructor
                                    .selected()
                                    .map(|operation| Copy::Class { class, operation })
                                && optional.lifecycle.assignment
                                    == declaration
                                        .copy_constructor
                                        .selected()
                                        .zip(declaration.copy_assignment.selected())
                                        .map(|(copy_constructor, copy_assignment)| {
                                            Assignment::Class {
                                                class,
                                                copy_constructor,
                                                copy_assignment,
                                            }
                                        })
                        })
                        && optional.lifecycle.cleanup == Cleanup::Class(class)
                        && optional.lifecycle.presence == Presence::OuterTag
                        && optional.lifecycle.unwrap == Unwrap::CheckedInlineClass(class)
                        && optional.checked_access == Access::GuardedInline
                }
                Storage::InlineArray(array) => {
                    optional.representation == Representation::TaggedPayload
                        && optional.lifecycle.initialization
                            == Initialization::TaggedAbsentOrPresent
                        && optional.lifecycle.injection == Injection::ConstructArray(array)
                        && optional.lifecycle.copy == Some(Copy::Array(array))
                        && optional.lifecycle.assignment == Some(Assignment::Array(array))
                        && optional.lifecycle.cleanup == Cleanup::Array(array)
                        && optional.lifecycle.presence == Presence::OuterTag
                        && optional.lifecycle.unwrap == Unwrap::CheckedInlineArray(array)
                        && optional.checked_access == Access::GuardedInline
                }
                Storage::SharedOwner(target) => {
                    optional.representation == Representation::NullableSharedOwner
                        && optional.lifecycle.initialization == Initialization::NullableSharedOwner
                        && optional.lifecycle.injection == Injection::RetainShared(target)
                        && optional.lifecycle.copy == Some(Copy::Shared(target))
                        && optional.lifecycle.assignment == Some(Assignment::Shared(target))
                        && optional.lifecycle.cleanup == Cleanup::Shared(target)
                        && optional.lifecycle.presence == Presence::SharedOwnerNull
                        && optional.lifecycle.unwrap == Unwrap::SecureSharedOwner(target)
                        && optional.checked_access == Access::SecuredSharedOwner
                }
                Storage::Nested(nested) => {
                    optional.representation == Representation::TaggedPayload
                        && optional.lifecycle.initialization
                            == Initialization::TaggedAbsentOrPresent
                        && optional.lifecycle.injection == Injection::ConstructNested(nested)
                        && self.program.optional_type(nested).is_some_and(|payload| {
                            optional.lifecycle.copy
                                == payload.lifecycle.copy.map(|_| Copy::Optional(nested))
                                && optional.lifecycle.assignment
                                    == payload
                                        .lifecycle
                                        .assignment
                                        .map(|_| Assignment::Optional(nested))
                        })
                        && optional.lifecycle.cleanup == Cleanup::Optional(nested)
                        && optional.lifecycle.presence == Presence::OuterTag
                        && optional.lifecycle.unwrap == Unwrap::CheckedNested(nested)
                        && optional.checked_access == Access::GuardedInline
                }
            };
            if !plan_valid {
                self.program_error(format!(
                    "optional {} has inconsistent executable lifecycle metadata",
                    optional.id
                ));
            }
            let boundary = optional
                .lifecycle
                .copy
                .map(Boundary::Copy)
                .unwrap_or(Boundary::MoveOnly);
            if optional.boundaries.argument != boundary
                || optional.boundaries.result != boundary
                || optional.boundaries.static_storage != boundary
                || optional.boundaries.array_element != boundary
            {
                self.program_error(format!(
                    "optional {} has inconsistent boundary lifecycle metadata",
                    optional.id
                ));
            }
        }
    }
}
