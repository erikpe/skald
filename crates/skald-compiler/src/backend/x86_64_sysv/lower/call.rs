//! System V call-lowering facade.

use crate::{
    backend::BackendError,
    identity::{CallableId, DestructorId},
    mir::{
        MirArgument, MirCall, MirCallableSignature, MirDefinitionRef, MirInitialize, MirPlace,
        MirSharedInitialize, ValueId,
    },
};

use super::super::machine::Instruction;
use super::{
    object_abi::{ObjectOriginOperand, ReceiverOperand},
    FrameLayout, InstructionSelector,
};

mod marshal;
mod target;

use target::CallTarget;

pub(super) fn spill_parameters(
    signature: MirCallableSignature<'_>,
    function: MirDefinitionRef<'_>,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    marshal::spill_parameters(signature, function, frame, output)
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_call(&mut self, call: &MirCall) -> Result<(), BackendError> {
        let (target, receiver) = CallTarget::from_call(call);
        self.select_callable(
            target,
            call.destination.as_ref(),
            receiver,
            &call.arguments,
            call.result,
        )
    }

    pub(super) fn select_initialize(
        &mut self,
        initialize: &MirInitialize,
    ) -> Result<(), BackendError> {
        self.select_callable(
            CallTarget::direct(initialize.target.into()),
            None,
            Some(ReceiverOperand {
                place: &initialize.destination,
                origin: ObjectOriginOperand::Exact {
                    complete: &initialize.destination,
                    dynamic_class: initialize.target.class(),
                },
            }),
            &initialize.arguments,
            None,
        )
    }

    pub(super) fn select_shared_initialize(
        &mut self,
        initialize: &MirSharedInitialize,
    ) -> Result<(), BackendError> {
        let target = CallTarget::direct(initialize.target.into());
        let signature = target.signature(self.program);
        let layout = self.marshal_shared_initializer_inputs(
            signature,
            initialize.allocation,
            initialize.target.class(),
            &initialize.arguments,
        )?;
        target.select(self, None)?;
        self.finish_call(
            &layout,
            target.direct_callable(),
            signature.return_type,
            None,
        );
        Ok(())
    }

    pub(super) fn select_destructor_call(
        &mut self,
        target: DestructorId,
        receiver: &MirPlace,
    ) -> Result<(), BackendError> {
        self.select_callable(
            CallTarget::direct(target.into()),
            None,
            Some(ReceiverOperand {
                place: receiver,
                origin: ObjectOriginOperand::Exact {
                    complete: receiver,
                    dynamic_class: target.class(),
                },
            }),
            &[],
            None,
        )
    }

    pub(super) fn select_direct_callable(
        &mut self,
        target: CallableId,
        return_destination: Option<&MirPlace>,
        receiver: Option<ReceiverOperand<'_>>,
        arguments: &[MirArgument],
        result: Option<ValueId>,
    ) -> Result<(), BackendError> {
        self.select_callable(
            CallTarget::direct(target),
            return_destination,
            receiver,
            arguments,
            result,
        )
    }

    fn select_callable(
        &mut self,
        target: CallTarget,
        return_destination: Option<&MirPlace>,
        receiver: Option<ReceiverOperand<'_>>,
        arguments: &[MirArgument],
        result: Option<ValueId>,
    ) -> Result<(), BackendError> {
        let signature = target.signature(self.program);
        let layout = self.marshal_call_inputs(
            signature,
            target.is_indirect(),
            return_destination,
            receiver,
            arguments,
        )?;
        target.select(self, receiver)?;
        self.finish_call(
            &layout,
            target.direct_callable(),
            signature.return_type,
            result,
        );
        Ok(())
    }
}
