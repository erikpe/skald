//! Shared MIR verifier state and diagnostic reporting.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::{
    super::model::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirPlace, MirPlaceBase, MirProgram, MirType,
        PreliminaryMirStaticField, ValueId,
    },
    sink::ErrorSink,
    MirVerificationError,
};

pub(super) struct Verifier<'mir> {
    pub(super) program: &'mir MirProgram,
    preliminary_static_fields: Option<&'mir [PreliminaryMirStaticField]>,
    pub(super) errors: ErrorSink,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn new(program: &'mir MirProgram) -> Self {
        Self {
            program,
            preliminary_static_fields: None,
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
            errors: ErrorSink::new(),
        }
    }

    pub(super) fn static_field_type_is_supported(
        &self,
        field: crate::identity::StaticFieldId,
        ty: MirType,
    ) -> bool {
        let zero_default = ty.is_scalar_value()
            || matches!(
                ty,
                MirType::OptionalPrimitive(_)
                    | MirType::OptionalClass(_)
                    | MirType::OptionalShared(_)
                    | MirType::Array(_)
            );
        let Some(fields) = self.preliminary_static_fields else {
            return zero_default;
        };
        fields
            .iter()
            .find(|candidate| candidate.field == field)
            .is_some_and(|declaration| {
                declaration.ty == ty
                    && if declaration.initializer.is_some() {
                        !matches!(ty, MirType::Unit | MirType::Obj | MirType::Interface(_))
                    } else {
                        zero_default
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
