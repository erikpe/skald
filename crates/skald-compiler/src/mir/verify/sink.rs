//! Deterministically ordered MIR verification errors.

use crate::identity::CallableId;

use super::{BlockId, MirVerificationError};

#[derive(Default)]
pub(super) struct ErrorSink {
    errors: Vec<MirVerificationError>,
}

impl ErrorSink {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn program(&mut self, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            callable: None,
            block: None,
            message: message.into(),
        });
    }

    pub(super) fn callable(&mut self, callable: impl Into<CallableId>, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            callable: Some(callable.into()),
            block: None,
            message: message.into(),
        });
    }

    pub(super) fn block(
        &mut self,
        callable: impl Into<CallableId>,
        block: BlockId,
        message: impl Into<String>,
    ) {
        self.errors.push(MirVerificationError {
            callable: Some(callable.into()),
            block: Some(block),
            message: message.into(),
        });
    }

    pub(super) fn into_errors(self) -> Vec<MirVerificationError> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use crate::identity::FunctionId;

    use super::*;

    #[test]
    fn preserves_insertion_order_across_error_scopes() {
        let function = FunctionId::new(2);
        let block = BlockId::new(function, 3);
        let mut errors = ErrorSink::new();

        errors.program("program");
        errors.callable(function, "callable");
        errors.block(function, block, "block");

        let errors = errors.into_errors();
        assert_eq!(
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            ["program", "callable", "block"]
        );
    }
}
