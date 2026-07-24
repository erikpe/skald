//! Lowering for the deliberately narrow first shared-owner lifetime.

use super::*;
use crate::hir::{
    HirOwnerTransfer, HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer,
};

impl BodyLowerer<'_> {
    pub(super) fn lower_exact_shared_allocation_local(
        &mut self,
        destination: StorageId,
        transfer: &HirSharedTransfer,
    ) {
        let HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) = &transfer.source
        else {
            unreachable!("the SO2 shared gate admits only allocation producers")
        };
        debug_assert_eq!(transfer.operation, HirOwnerTransfer::Adopt);
        debug_assert_eq!(transfer.target, HirSharedTarget::Class(allocation.class));

        // Source arguments are evaluated before allocation, matching ordinary
        // construction and keeping unpublished storage unobservable.
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
            span: transfer.span,
        }));
        self.full_expression_has_shared_effect = true;
    }
}
