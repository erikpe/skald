//! Hidden lifetime anchors for inline array backing.

use crate::{
    backend::BackendError,
    mir::{MirArrayAnchorKind, MirPlace, MirPlaceBase, MirType, StorageId},
};

use super::super::{
    super::{
        layout::ARRAY_OWNER_COUNT_OFFSET,
        machine::{Instruction, Register},
        symbol,
    },
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_array_anchor_begin(
        &mut self,
        anchor: StorageId,
        owner: &MirPlace,
        kind: MirArrayAnchorKind,
    ) -> Result<(), BackendError> {
        self.load_array_owner(owner)?;
        if kind == MirArrayAnchorKind::InlineBacking {
            if matches!(owner.base, MirPlaceBase::AliasParameter(_)) {
                // The caller's anchor covers the complete call. Forwarding
                // the non-owning parameter must not reinterpret a normalized
                // shared-array descriptor as an inline backing header.
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 0,
                    destination: Register::Rax,
                });
            } else {
                self.retain_inline_array_backing();
            }
        } else if is_shared_anchor(kind) {
            let array = self.array_for_storage(anchor)?;
            let layout = self
                .data_layout
                .array(array)
                .ok_or_else(|| self.array_error(format!("array {array} has no anchor layout")))?;
            self.output.push(Instruction::MoveImmediate64 {
                bits: u64::try_from(layout.shared_element_offset() - layout.element_offset())
                    .expect("shared array alias adjustment fits u64"),
                destination: Register::R11,
            });
            self.output.push(Instruction::Add {
                source: Register::R11,
                destination: Register::Rax,
            });
        }
        value::store_rax(value::frame_storage(self.frame, anchor), self.output);
        Ok(())
    }

    pub(super) fn select_array_anchor_end(
        &mut self,
        anchor: StorageId,
    ) -> Result<(), BackendError> {
        let storage = self
            .function
            .storage(anchor)
            .expect("verified array anchor has storage");
        let MirType::Array(array) = storage.ty else {
            return Err(self.array_error("array anchor storage has no array type"));
        };
        if storage.kind
            == crate::mir::MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineBacking)
        {
            value::load_rax(value::frame_storage(self.frame, anchor), self.output);
            self.output.push(Instruction::Move {
                source: Register::Rax.into(),
                destination: Register::Rdi.into(),
            });
            self.output
                .push(Instruction::Call(symbol::array_release(array)));
        }
        self.clear_storage(anchor);
        Ok(())
    }

    pub(super) fn select_array_alias_bind(
        &mut self,
        alias: StorageId,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        self.materialize_place_address(source, Register::Rax)?;
        value::store_rax(value::frame_storage(self.frame, alias), self.output);
        let MirType::Class(class) = self
            .function
            .storage(alias)
            .expect("verified array alias has storage")
            .ty
        else {
            return Ok(());
        };
        let origins = self
            .frame
            .object_origin(alias)
            .expect("class array alias has object-origin homes");
        value::store_rax(
            value::memory(Register::Rbp, origins.complete()),
            self.output,
        );
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: symbol::dispatch_table(class),
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::Rbp, origins.metadata()),
            self.output,
        );
        Ok(())
    }

    fn retain_inline_array_backing(&mut self) {
        let empty = self.next_array_label("anchor_retain_empty");
        let failure = self.next_array_label("anchor_retain_failure");
        let complete = self.next_array_label("anchor_retain_complete");
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, ARRAY_OWNER_COUNT_OFFSET),
            destination: Register::Rcx.into(),
        });
        self.output.push(Instruction::Test(Register::Rcx));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::MAX,
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::Move {
            source: Register::Rcx.into(),
            destination: value::memory(Register::Rax, ARRAY_OWNER_COUNT_OFFSET),
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(failure));
        self.output.push(Instruction::Trap);
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::Label(complete));
    }
}

const fn is_shared_anchor(kind: MirArrayAnchorKind) -> bool {
    matches!(
        kind,
        MirArrayAnchorKind::StableSharedOwner
            | MirArrayAnchorKind::CopiedSharedOwner
            | MirArrayAnchorKind::AdoptedSharedOwner
            | MirArrayAnchorKind::SecuredOptionalSharedOwner
    )
}
