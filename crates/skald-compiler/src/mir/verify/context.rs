//! Shared MIR verifier state and diagnostic reporting.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirPlace, MirPlaceBase, MirProgram, MirType,
        PreliminaryMirStaticField, ValueId,
    },
    contract::MirVerificationContract,
    sink::ErrorSink,
    MirVerificationError,
};

pub(super) struct Verifier<'mir> {
    pub(super) program: &'mir MirProgram,
    preliminary_static_fields: Option<&'mir [PreliminaryMirStaticField]>,
    definition_completeness: MirDefinitionCompleteness,
    verification_contract: MirVerificationContract,
    pub(super) errors: ErrorSink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirDefinitionCompleteness {
    RetainedFinal,
    CompleteProducer,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn optional_type(&self, ty: MirType) -> Option<&'mir crate::mir::MirOptionalType> {
        let MirType::Optional(optional) = ty else {
            return None;
        };
        self.program.optional_type(optional)
    }

    pub(super) fn optional_class(&self, ty: MirType) -> Option<crate::identity::ClassId> {
        self.optional_type(ty)
            .and_then(crate::mir::MirOptionalType::inline_class)
    }

    pub(super) fn optional_shared(&self, ty: MirType) -> Option<crate::mir::MirSharedTarget> {
        self.optional_type(ty)
            .and_then(crate::mir::MirOptionalType::shared_owner)
    }

    pub(super) fn optional_primitive(&self, ty: MirType) -> Option<crate::mir::MirPrimitiveType> {
        self.optional_type(ty)
            .and_then(crate::mir::MirOptionalType::primitive)
    }

    pub(super) fn new(program: &'mir MirProgram) -> Self {
        Self {
            program,
            preliminary_static_fields: None,
            definition_completeness: MirDefinitionCompleteness::RetainedFinal,
            verification_contract: MirVerificationContract::ProofRich,
            errors: ErrorSink::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn new_normalized(program: &'mir MirProgram) -> Self {
        Self {
            program,
            preliminary_static_fields: None,
            definition_completeness: MirDefinitionCompleteness::RetainedFinal,
            verification_contract: MirVerificationContract::Normalized,
            errors: ErrorSink::new(),
        }
    }

    pub(super) fn new_preliminary(
        program: &'mir MirProgram,
        static_fields: &'mir [PreliminaryMirStaticField],
    ) -> Self {
        Self {
            program,
            preliminary_static_fields: Some(static_fields),
            definition_completeness: MirDefinitionCompleteness::CompleteProducer,
            verification_contract: MirVerificationContract::ProofRich,
            errors: ErrorSink::new(),
        }
    }

    pub(super) const fn verification_contract(&self) -> MirVerificationContract {
        self.verification_contract
    }

    pub(super) const fn requires_complete_producer_definitions(&self) -> bool {
        matches!(
            self.definition_completeness,
            MirDefinitionCompleteness::CompleteProducer
        )
    }

    pub(super) fn static_field_type_is_supported(
        &self,
        field: crate::identity::StaticFieldId,
        ty: MirType,
    ) -> bool {
        let zero_default = (ty.is_scalar_value() && !matches!(ty, MirType::Function(_)))
            || matches!(ty, MirType::Optional(_) | MirType::Array(_));
        if let Some(fields) = self.preliminary_static_fields {
            return fields
                .iter()
                .find(|candidate| candidate.field == field)
                .is_some_and(|declaration| {
                    declaration.ty == ty
                        && if declaration.initializer.is_some() {
                            !matches!(ty, MirType::Unit | MirType::Obj | MirType::Interface(_))
                        } else {
                            zero_default
                        }
                });
        }
        self.program.static_field(field).is_some_and(|declaration| {
            declaration.ty == ty
                && match declaration.initialization {
                    crate::mir::MirStaticFieldInitialization::Explicit(_) => {
                        !matches!(ty, MirType::Unit | MirType::Obj | MirType::Interface(_))
                            && self.program.static_lifecycle.is_some()
                    }
                    crate::mir::MirStaticFieldInitialization::ZeroDefault => zero_default,
                }
        })
    }

    /// Whether this destination is the complete program-owned slot belonging
    /// to the preliminary initializer body currently being verified.
    pub(super) fn is_static_initializer_destination(
        &self,
        function: MirDefinitionRef<'_>,
        place: &MirPlace,
        ty: MirType,
    ) -> bool {
        matches!(
            (function.callable(), place.base, place.projections.as_slice()),
            (
                CallableId::StaticInitializer(initializer),
                MirPlaceBase::StaticLifecycleDestination(field),
                _
            ) if initializer.field() == field
                && self.program.static_field(field).is_some_and(|entry| entry.ty == ty)
        )
    }

    pub(super) fn into_errors(self) -> Vec<MirVerificationError> {
        self.errors.into_errors()
    }

    pub(super) fn verify_value_use(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is used before it is defined in this block"),
            );
        }
        Some(metadata.ty)
    }

    pub(super) fn program_error(&mut self, message: impl Into<String>) {
        self.errors.program(message);
    }

    pub(super) fn function_error(
        &mut self,
        callable: impl Into<CallableId>,
        message: impl Into<String>,
    ) {
        self.errors.callable(callable, message);
    }

    pub(super) fn block_error(
        &mut self,
        callable: impl Into<CallableId>,
        block: BlockId,
        message: impl Into<String>,
    ) {
        self.errors.block(callable, block, message);
    }
}
