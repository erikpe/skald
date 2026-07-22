//! Shared MIR verifier state and diagnostic reporting.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::{super::model::*, sink::ErrorSink, MirVerificationError};

pub(super) struct Verifier<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) errors: ErrorSink,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn new(program: &'mir MirProgram) -> Self {
        Self {
            program,
            errors: ErrorSink::new(),
        }
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
