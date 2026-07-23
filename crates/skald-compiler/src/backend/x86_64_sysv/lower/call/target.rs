//! Direct and dynamic call-target selection.

use crate::{
    backend::BackendError,
    identity::{CallableId, InterfaceRequirementId, MethodId, VirtualSlotId},
    mir::{
        MirCall, MirCallReceiver, MirCallTarget, MirCallableSignature, MirMethodCallTarget,
        MirProgram,
    },
};

use super::super::super::machine::{Instruction, Register};
use super::super::{
    object_abi::{ObjectOriginOperand, ReceiverOperand},
    InstructionSelector,
};

#[derive(Clone, Copy)]
pub(super) enum CallTarget {
    Direct(CallableId),
    Virtual {
        selected: MethodId,
        slot: VirtualSlotId,
    },
    Interface(InterfaceRequirementId),
}

impl CallTarget {
    pub(super) fn from_call(call: &MirCall) -> (Self, Option<ReceiverOperand<'_>>) {
        match call.target {
            MirCallTarget::Direct(function) => (Self::Direct(function.into()), None),
            MirCallTarget::Method(method) => {
                let receiver = call
                    .receiver
                    .as_ref()
                    .and_then(MirCallReceiver::as_method)
                    .expect("verified method call has a receiver");
                let target = match method {
                    MirMethodCallTarget::Direct(method) => Self::Direct(method.into()),
                    MirMethodCallTarget::Virtual { selected, slot, .. } => {
                        Self::Virtual { selected, slot }
                    }
                };
                (
                    target,
                    Some(ReceiverOperand {
                        place: &receiver.place,
                        origin: ObjectOriginOperand::Mir(&receiver.origin),
                    }),
                )
            }
            MirCallTarget::Interface(target) => {
                let receiver = call
                    .receiver
                    .as_ref()
                    .and_then(MirCallReceiver::as_interface)
                    .expect("verified interface call has a receiver");
                (
                    Self::Interface(target.requirement),
                    Some(ReceiverOperand {
                        place: &receiver.source,
                        origin: ObjectOriginOperand::Mir(&receiver.origin),
                    }),
                )
            }
        }
    }

    pub(super) const fn direct(target: CallableId) -> Self {
        Self::Direct(target)
    }

    pub(super) fn signature(self, program: &MirProgram) -> MirCallableSignature<'_> {
        match self {
            Self::Direct(target) => program
                .callable_signature(target)
                .expect("verified call target must be declared"),
            Self::Virtual { selected, .. } => program
                .callable_signature(selected.into())
                .expect("verified virtual target must be declared"),
            Self::Interface(requirement) => {
                let requirement = program
                    .interface_requirement(requirement)
                    .expect("verified interface target must be declared");
                MirCallableSignature {
                    parameters: &requirement.parameters,
                    return_type: requirement.return_type,
                }
            }
        }
    }

    pub(super) const fn is_indirect(self) -> bool {
        !matches!(self, Self::Direct(_))
    }

    pub(super) const fn direct_callable(self) -> Option<CallableId> {
        match self {
            Self::Direct(target) => Some(target),
            Self::Virtual { .. } | Self::Interface(_) => None,
        }
    }

    pub(super) fn select(
        self,
        selector: &mut InstructionSelector<'_, '_>,
        receiver: Option<ReceiverOperand<'_>>,
    ) -> Result<(), BackendError> {
        match self {
            Self::Direct(target) => {
                selector
                    .output
                    .push(Instruction::Call(super::super::super::symbol::callable(
                        selector.program,
                        target,
                    )));
            }
            Self::Virtual { slot, .. } => {
                let receiver = receiver.expect("virtual call has a receiver");
                selector.select_virtual_target(receiver.origin, slot)?;
                selector
                    .output
                    .push(Instruction::CallIndirect(Register::R11));
            }
            Self::Interface(requirement) => {
                let receiver = receiver.expect("interface call has a receiver");
                selector.select_interface_target(receiver.origin, requirement)?;
                selector
                    .output
                    .push(Instruction::CallIndirect(Register::R11));
            }
        }
        Ok(())
    }
}
