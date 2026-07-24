//! Runtime type tests and scoped checked-narrowing lowering.

use crate::{
    backend::BackendError,
    mir::{
        BlockId, MirCheckedViewBinding, MirNarrowedAliasBinding, MirObjectOrigin, MirObjectView,
        MirTerminationReason, MirTerminator, MirType, MirViewTarget, ValueId,
    },
};

use super::{
    super::{
        abi::ArgumentLocation,
        machine::{Instruction, Label, Register},
        symbol,
    },
    block_label,
    object_abi::ObjectOriginOperand,
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_type_test(
        &mut self,
        source: &MirObjectView,
        target: MirViewTarget,
        result: ValueId,
    ) {
        let destination = value::frame_value(self.frame, result);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);

        let matched = type_test_label(result, "matched");
        let finished = type_test_label(result, "finished");
        self.emit_membership_branches(&source.origin, target, &matched);
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(matched));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
        self.output.push(Instruction::Label(finished));
    }

    pub(super) fn select_narrowed_alias_binding(
        &mut self,
        binding: &MirNarrowedAliasBinding,
    ) -> Result<(), BackendError> {
        self.select_place_address(
            &binding.view.source,
            ArgumentLocation::IntegerRegister(Register::Rax),
        )?;
        value::store_rax(
            value::memory(Register::Rbp, self.frame.storage(binding.destination)),
            self.output,
        );
        self.store_object_origin(
            ObjectOriginOperand::Mir(&binding.view.origin),
            binding.destination,
        )
    }

    pub(super) fn select_checked_view_binding(
        &mut self,
        binding: &MirCheckedViewBinding,
    ) -> Result<(), BackendError> {
        self.select_place_address(
            &binding.view.source,
            ArgumentLocation::IntegerRegister(Register::Rax),
        )?;
        value::store_rax(
            value::memory(Register::Rbp, self.frame.storage(binding.destination)),
            self.output,
        );
        self.store_object_origin(
            ObjectOriginOperand::Mir(&binding.view.origin),
            binding.destination,
        )
    }

    pub(super) fn select_type_operation_terminator(
        &mut self,
        terminator: &MirTerminator,
        block: BlockId,
    ) -> Result<bool, BackendError> {
        match terminator {
            MirTerminator::CheckedNarrow {
                binding,
                success_target,
                failure_target,
                ..
            } => {
                let matched = narrowing_match_label(self.function.callable(), block);
                self.emit_membership_branches(&binding.view.origin, binding.view.target, &matched);
                self.output
                    .push(Instruction::Jump(block_label(*failure_target)));
                self.output.push(Instruction::Label(matched));
                self.select_narrowed_alias_binding(binding)?;
                self.output
                    .push(Instruction::Jump(block_label(*success_target)));
                Ok(true)
            }
            MirTerminator::CheckedCast {
                binding,
                success_target,
                failure_target,
                ..
            } => {
                let matched = cast_match_label(self.function.callable(), block);
                self.emit_membership_branches(&binding.view.origin, binding.view.target, &matched);
                self.output
                    .push(Instruction::Jump(block_label(*failure_target)));
                self.output.push(Instruction::Label(matched));
                self.select_checked_view_binding(binding)?;
                self.output
                    .push(Instruction::Jump(block_label(*success_target)));
                Ok(true)
            }
            MirTerminator::Terminate {
                reason:
                    MirTerminationReason::NarrowingFailure | MirTerminationReason::ObjectCastFailure,
                ..
            } => {
                self.output.push(Instruction::Trap);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn emit_membership_branches(
        &mut self,
        origin: &MirObjectOrigin,
        target: MirViewTarget,
        matched: &Label,
    ) {
        self.load_origin_metadata(ObjectOriginOperand::Mir(origin), Register::R11);
        let classes = self.dispatch.classes_providing_view(self.program, target);
        debug_assert!(!classes.is_empty(), "verified runtime target can succeed");
        for class in classes {
            self.output.push(Instruction::LoadSymbolAddress {
                symbol: self.dispatch.table_symbol(class),
                destination: Register::Rcx,
            });
            self.output.push(Instruction::Compare {
                source: Register::Rcx,
                destination: Register::R11,
            });
            self.output.push(Instruction::JumpIfEqual(matched.clone()));
        }
    }
}

fn cast_match_label(callable: crate::identity::CallableId, block: BlockId) -> Label {
    Label::new(format!(
        ".Lska_{}_cast_{}_matched",
        symbol::local_label_stem(callable),
        block.index()
    ))
}

fn type_test_label(result: ValueId, suffix: &str) -> Label {
    Label::new(format!(
        ".Lska_{}_type_test_{}_{}",
        symbol::local_label_stem(result.callable()),
        result.index(),
        suffix
    ))
}

fn narrowing_match_label(callable: crate::identity::CallableId, block: BlockId) -> Label {
    Label::new(format!(
        ".Lska_{}_narrow_{}_matched",
        symbol::local_label_stem(callable),
        block.index()
    ))
}
