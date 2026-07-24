//! Exact-class local and full-expression shared-owner lowering.

use super::*;
use crate::hir::{
    HirOwnerTransfer, HirSharedAllocation, HirSharedAssignment, HirSharedPlace, HirSharedProducer,
    HirSharedSource, HirSharedTarget, HirSharedTransfer,
};

impl BodyLowerer<'_> {
    pub(super) fn lower_shared_local(
        &mut self,
        destination: StorageId,
        transfer: &HirSharedTransfer,
    ) {
        self.lower_shared_transfer(destination, transfer);
        self.full_expression_has_shared_effect = true;
    }

    pub(super) fn lower_shared_assignment(&mut self, assignment: &HirSharedAssignment) {
        let destination = self.storage_for_binding(assignment.destination);
        let secured = self.new_shared_temporary(assignment.value.target, assignment.span);
        self.lower_shared_transfer(secured, &assignment.value);
        self.emit(MirInstruction::SharedRelease(MirSharedRelease {
            owner: destination,
            span: assignment.span,
        }));
        self.consume_shared_temporary(secured);
        self.emit(MirInstruction::SharedMove(MirSharedMove {
            destination,
            source: secured,
            span: assignment.span,
        }));
        self.full_expression_has_shared_effect = true;
    }

    fn lower_shared_transfer(&mut self, destination: StorageId, transfer: &HirSharedTransfer) {
        match &transfer.source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                debug_assert_eq!(transfer.operation, HirOwnerTransfer::Copy);
                self.emit(MirInstruction::SharedCopy(MirSharedCopy {
                    destination,
                    source: self.storage_for_binding(*binding),
                    span: transfer.span,
                }));
            }
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                debug_assert_eq!(transfer.operation, HirOwnerTransfer::Adopt);
                self.lower_shared_allocation(destination, allocation);
            }
            HirSharedSource::Place(HirSharedPlace::Field { .. })
            | HirSharedSource::Produced(HirSharedProducer::Call(_)) => {
                unreachable!("broader shared sources are rejected before MIR lowering")
            }
        }
    }

    fn lower_shared_allocation(
        &mut self,
        destination: StorageId,
        allocation: &HirSharedAllocation,
    ) {
        let arguments = self.lower_call_arguments(&allocation.arguments);
        let allocation_storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: allocation_storage,
            source: None,
            name: format!("shared-allocation-{}", allocation_storage.index()),
            kind: MirStorageKind::SharedAllocation,
            ty: MirType::Class(allocation.class),
            span: allocation.span,
        });
        self.emit(MirInstruction::SharedAllocate(MirSharedAllocate {
            allocation: allocation_storage,
            class: allocation.class,
            origin: MirSharedAllocationOrigin::New,
            span: allocation.span,
        }));
        self.emit(MirInstruction::SharedInitialize(MirSharedInitialize {
            allocation: allocation_storage,
            target: allocation.initializer,
            arguments,
            span: allocation.span,
        }));
        self.emit(MirInstruction::SharedPublish(MirSharedPublish {
            allocation: allocation_storage,
            span: allocation.span,
        }));
        self.emit(MirInstruction::SharedAdopt(MirSharedAdopt {
            destination,
            allocation: allocation_storage,
            span: allocation.span,
        }));
    }

    fn new_shared_temporary(
        &mut self,
        target: HirSharedTarget,
        span: crate::source::Span,
    ) -> StorageId {
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("shared-temporary-{}", storage.index()),
            kind: MirStorageKind::Temporary,
            ty: lower_type(Type::Shared(target)),
            span,
        });
        self.full_expression_shared_temporaries.push(storage);
        storage
    }

    fn consume_shared_temporary(&mut self, storage: StorageId) {
        let index = self
            .full_expression_shared_temporaries
            .iter()
            .rposition(|candidate| *candidate == storage)
            .expect("consumed shared temporary must belong to the current full expression");
        self.full_expression_shared_temporaries.remove(index);
    }
}
