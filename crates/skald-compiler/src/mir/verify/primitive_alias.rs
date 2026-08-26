//! Produced primitive read-only alias storage verification.

use crate::mir::{
    BlockId, MirArgument, MirDefinitionRef, MirInstruction, MirPlace, MirStorageKind, StorageId,
};

use super::{checked_scalar, context::Verifier, lifetime::uses};

#[derive(Clone, Copy)]
struct InstructionSite {
    block: BlockId,
    index: usize,
}

#[derive(Default)]
struct PrimitiveAliasShape {
    live: Vec<InstructionSite>,
    stores: Vec<InstructionSite>,
    borrows: Vec<InstructionSite>,
    dead: Vec<InstructionSite>,
    invalid_uses: Vec<InstructionSite>,
    invalid_store_metadata: bool,
}

impl Verifier<'_> {
    pub(super) fn verify_produced_primitive_aliases(&mut self, function: MirDefinitionRef<'_>) {
        for storage in function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::PrimitiveAlias)
        {
            if !storage.ty.is_primitive() {
                self.function_error(
                    function.callable(),
                    format!(
                        "produced primitive alias storage {} has non-primitive type {}",
                        storage.id, storage.ty
                    ),
                );
            }

            let shape = primitive_alias_shape(function, storage.id);
            self.verify_primitive_alias_shape(function, storage.id, &shape);
        }
    }

    fn verify_primitive_alias_shape(
        &mut self,
        function: MirDefinitionRef<'_>,
        storage: StorageId,
        shape: &PrimitiveAliasShape,
    ) {
        let entry = function.body().entry;
        if shape.live.len() != 1 || shape.dead.len() != 1 {
            self.block_error(
                function.callable(),
                entry,
                format!(
                    "produced primitive alias storage {storage} must have one bounded lifetime, found {} starts and {} ends",
                    shape.live.len(),
                    shape.dead.len()
                ),
            );
        }
        if shape.stores.len() != 1 {
            self.block_error(
                function.callable(),
                entry,
                format!(
                    "produced primitive alias storage {storage} must be initialized exactly once, found {} stores",
                    shape.stores.len()
                ),
            );
        }
        if shape.borrows.len() != 1 {
            self.block_error(
                function.callable(),
                entry,
                format!(
                    "produced primitive alias storage {storage} must be borrowed by exactly one call argument, found {} borrows",
                    shape.borrows.len()
                ),
            );
        }
        if shape.invalid_store_metadata {
            self.block_error(
                function.callable(),
                entry,
                format!(
                    "produced primitive alias storage {storage} initialization must not carry field-write authorization"
                ),
            );
        }
        if !shape.invalid_uses.is_empty() {
            self.block_error(
                function.callable(),
                shape.invalid_uses[0].block,
                format!(
                    "produced primitive alias storage {storage} may only be initialized and passed once as an alias argument"
                ),
            );
        }

        let ([live], [store], [borrow], [dead]) = (
            shape.live.as_slice(),
            shape.stores.as_slice(),
            shape.borrows.as_slice(),
            shape.dead.as_slice(),
        ) else {
            return;
        };
        for (before, after, description) in [
            (*live, *store, "become live before initialization"),
            (*store, *borrow, "be initialized before alias use"),
            (*borrow, *dead, "remain live until after the call"),
        ] {
            if !site_precedes(function, before, after) {
                self.block_error(
                    function.callable(),
                    after.block,
                    format!("produced primitive alias storage {storage} must {description}"),
                );
            }
        }
    }
}

fn primitive_alias_shape(
    function: MirDefinitionRef<'_>,
    storage: StorageId,
) -> PrimitiveAliasShape {
    let mut shape = PrimitiveAliasShape::default();
    for block in &function.body().blocks {
        for (index, instruction) in block.instructions.iter().enumerate() {
            let site = InstructionSite {
                block: block.id,
                index,
            };
            match instruction {
                MirInstruction::StorageLive(operation) if operation.storage == storage => {
                    shape.live.push(site);
                    continue;
                }
                MirInstruction::StorageDead(operation) if operation.storage == storage => {
                    shape.dead.push(site);
                    continue;
                }
                MirInstruction::Store(store) if store.destination == MirPlace::base(storage) => {
                    shape.stores.push(site);
                    shape.invalid_store_metadata |=
                        store.authorization.is_some() || store.final_authorization.is_some();
                }
                MirInstruction::Call(call) => {
                    shape.borrows.extend(
                        call.arguments
                            .iter()
                            .filter(|argument| is_exact_alias_argument(argument, storage))
                            .map(|_| site),
                    );
                }
                MirInstruction::Initialize(initialize) => {
                    shape.borrows.extend(
                        initialize
                            .arguments
                            .iter()
                            .filter(|argument| is_exact_alias_argument(argument, storage))
                            .map(|_| site),
                    );
                }
                _ => {}
            }

            let mut references = 0;
            uses::visit_instruction_storage(instruction, &mut |candidate| {
                if candidate == storage {
                    references += 1;
                }
            });
            let allowed = match instruction {
                MirInstruction::Store(store) if store.destination == MirPlace::base(storage) => 1,
                MirInstruction::Call(call) => call
                    .arguments
                    .iter()
                    .filter(|argument| is_exact_alias_argument(argument, storage))
                    .count(),
                MirInstruction::Initialize(initialize) => initialize
                    .arguments
                    .iter()
                    .filter(|argument| is_exact_alias_argument(argument, storage))
                    .count(),
                _ => 0,
            };
            if references != allowed {
                shape.invalid_uses.push(site);
            }
        }
    }
    shape
}

fn is_exact_alias_argument(argument: &MirArgument, storage: StorageId) -> bool {
    matches!(argument, MirArgument::Place(place) if *place == MirPlace::base(storage))
}

fn site_precedes(
    function: MirDefinitionRef<'_>,
    before: InstructionSite,
    after: InstructionSite,
) -> bool {
    if before.block == after.block {
        before.index < after.index
    } else {
        checked_scalar::dominates(function, before.block, after.block)
    }
}
