//! Shared-owner transfer, allocation, field, and full-expression lowering.

use super::*;
use crate::hir::{
    HirSharedAllocation, HirSharedAllocationMode, HirSharedAssignment, HirSharedCast,
    HirSharedCastKind, HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace,
    HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer,
};

impl BodyLowerer<'_> {
    pub(super) fn lower_shared_local(
        &mut self,
        destination: StorageId,
        transfer: &HirSharedTransfer,
    ) {
        if matches!(
            transfer.source,
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap(_))
        ) {
            let secured = self.new_shared_temporary(transfer.target, transfer.span);
            self.lower_shared_transfer(secured, transfer);
            self.consume_shared_temporary(secured);
            self.emit(MirInstruction::SharedMove(MirSharedMove {
                destination,
                source: secured,
                span: transfer.span,
            }));
        } else {
            self.lower_shared_transfer(destination, transfer);
        }
        self.full_expression.mark_shared_effect();
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
        self.full_expression.mark_shared_effect();
    }

    pub(super) fn lower_shared_field_write(&mut self, write: &HirSharedFieldWrite) {
        let secured = self.new_shared_temporary(write.value.target, write.span);
        self.lower_shared_transfer(secured, &write.value);
        self.consume_shared_temporary(secured);
        let destination = self.lower_field_place(&write.place);
        self.emit(match write.kind {
            HirSharedFieldWriteKind::Initialize => {
                MirInstruction::SharedFieldInitialize(MirSharedFieldInitialize {
                    destination,
                    source: secured,
                    span: write.span,
                })
            }
            HirSharedFieldWriteKind::Assign => {
                MirInstruction::SharedFieldReplace(MirSharedFieldReplace {
                    destination,
                    source: secured,
                    span: write.span,
                })
            }
        });
        self.full_expression.mark_shared_effect();
    }

    pub(super) fn lower_shared_transfer(
        &mut self,
        destination: StorageId,
        transfer: &HirSharedTransfer,
    ) {
        self.lower_shared_source(destination, &transfer.source, transfer.span);
        self.full_expression.mark_shared_effect();
    }

    pub(super) fn lower_shared_source(
        &mut self,
        destination: StorageId,
        source: &HirSharedSource,
        span: crate::source::Span,
    ) {
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding { binding, .. }) => {
                self.emit(MirInstruction::SharedCopy(MirSharedCopy {
                    destination,
                    source: self.storage_for_binding(*binding),
                    span,
                }));
            }
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                self.lower_shared_allocation(destination, allocation);
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                self.lower_shared_call(call, destination);
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                self.lower_shared_cast(destination, cast);
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap(operand)) => {
                self.lower_optional_shared_unwrap(operand, destination);
            }
            HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
                self.lower_shared_array_construction(destination, construction)
            }
            HirSharedSource::Place(HirSharedPlace::Field { place, .. }) => {
                let source = self.lower_field_place(place);
                self.emit(MirInstruction::SharedFieldCopy(MirSharedFieldCopy {
                    destination,
                    source,
                    span,
                }));
            }
            HirSharedSource::Place(HirSharedPlace::ArrayElement { place, .. }) => {
                let source = self.lower_array_element_place(place);
                self.emit(MirInstruction::SharedFieldCopy(MirSharedFieldCopy {
                    destination,
                    source,
                    span,
                }));
            }
            HirSharedSource::Place(HirSharedPlace::Static { .. }) => {
                unreachable!("static shared sources require lifecycle MIR lowering")
            }
        }
    }

    fn lower_shared_cast(&mut self, destination: StorageId, cast: &HirSharedCast) {
        let (source, transfer) = match &cast.source {
            HirSharedSource::Place(HirSharedPlace::Binding {
                binding, target, ..
            }) => (
                MirSharedCastSource::Owner {
                    storage: self.storage_for_binding(*binding),
                    target: lower_shared_target(*target),
                },
                MirSharedCastTransfer::Copy,
            ),
            HirSharedSource::Place(HirSharedPlace::Field { place, target, .. }) => (
                MirSharedCastSource::Field {
                    place: self.lower_field_place(place),
                    target: lower_shared_target(*target),
                },
                MirSharedCastTransfer::Copy,
            ),
            HirSharedSource::Place(HirSharedPlace::ArrayElement { place, target, .. }) => (
                MirSharedCastSource::Field {
                    place: self.lower_array_element_place(place),
                    target: lower_shared_target(*target),
                },
                MirSharedCastTransfer::Copy,
            ),
            HirSharedSource::Place(HirSharedPlace::Static { .. }) => {
                unreachable!("static shared casts require lifecycle MIR lowering")
            }
            produced @ HirSharedSource::Produced(_) => {
                let temporary = self.new_shared_temporary(produced.target(), produced.span());
                self.lower_shared_source(temporary, produced, produced.span());
                self.consume_shared_temporary(temporary);
                (
                    MirSharedCastSource::Owner {
                        storage: temporary,
                        target: lower_shared_target(produced.target()),
                    },
                    MirSharedCastTransfer::Adopt,
                )
            }
        };
        let mir_cast = MirSharedCast {
            destination,
            source,
            target: lower_shared_target(cast.target),
            transfer,
            exact_dynamic_class: cast.exact_dynamic_class,
            span: cast.span,
        };
        match cast.kind {
            HirSharedCastKind::Static => {
                self.emit(MirInstruction::SharedCast(mir_cast));
            }
            HirSharedCastKind::RuntimeTerminate => {
                let success = self.body.allocate_block(cast.span);
                let failure = self.body.allocate_block(cast.span);
                self.terminate(MirTerminator::SharedCast {
                    cast: mir_cast,
                    success_target: success,
                    failure_target: failure,
                    span: cast.span,
                });
                self.body
                    .select_block(failure)
                    .expect("allocated shared-cast failure block must be selectable");
                self.terminate(MirTerminator::Terminate {
                    reason: MirTerminationReason::ObjectCastFailure,
                    span: cast.span,
                });
                self.body
                    .select_block(success)
                    .expect("allocated shared-cast success block must be selectable");
            }
        }
    }

    fn lower_shared_allocation(
        &mut self,
        destination: StorageId,
        allocation: &HirSharedAllocation,
    ) {
        enum Initialization {
            Initialize {
                target: crate::identity::InitializerId,
                arguments: Vec<MirArgument>,
            },
            Copy {
                source: MirPlace,
                operation: MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
            },
        }

        // Establish every borrowed source, checked view, and hidden anchor
        // before allocating. A failing checked cast therefore cannot leak an
        // unpublished allocation.
        let optional_mark = self.optional_view_mark();
        let initialization = match &allocation.mode {
            HirSharedAllocationMode::Initialize {
                initializer,
                arguments,
            } => Initialization::Initialize {
                target: *initializer,
                arguments: self.lower_call_arguments(arguments),
            },
            HirSharedAllocationMode::Copy { source, operation } => Initialization::Copy {
                source: self.lower_object_source(source),
                operation: lower_selected_copy_operation(*operation),
            },
        };
        let allocation_storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: allocation_storage,
            source: None,
            name: format!("shared-allocation-{}", allocation_storage.index()),
            kind: MirStorageKind::SharedAllocation,
            ty: MirType::Class(allocation.class),
            span: allocation.span,
        });
        self.track_full_expression_storage(allocation_storage, allocation.span);
        self.emit(MirInstruction::SharedAllocate(MirSharedAllocate {
            allocation: allocation_storage,
            class: allocation.class,
            origin: MirSharedAllocationOrigin::New,
            mode: match &initialization {
                Initialization::Initialize { .. } => MirSharedAllocationMode::Initialize,
                Initialization::Copy { source, .. } => MirSharedAllocationMode::Copy {
                    source: source.clone(),
                },
            },
            span: allocation.span,
        }));
        match initialization {
            Initialization::Initialize { target, arguments } => {
                self.emit(MirInstruction::SharedInitialize(MirSharedInitialize {
                    allocation: allocation_storage,
                    target,
                    arguments,
                    span: allocation.span,
                }));
            }
            Initialization::Copy { source, operation } => {
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination: MirPlace::shared_allocation_payload(allocation_storage),
                    source,
                    class: allocation.class,
                    operation,
                    span: allocation.span,
                }));
            }
        }
        self.end_optional_views_from(optional_mark, allocation.span);
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

    pub(super) fn new_shared_temporary(
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
        self.track_full_expression_storage(storage, span);
        self.full_expression
            .register_temporary(FullExpressionTemporary::Shared(storage));
        storage
    }

    pub(super) fn new_shared_anchor(
        &mut self,
        source: &HirSharedSource,
        span: crate::source::Span,
    ) -> StorageId {
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("shared-anchor-{}", storage.index()),
            kind: MirStorageKind::SharedAnchor,
            ty: lower_type(Type::Shared(source.target())),
            span,
        });
        self.track_full_expression_storage(storage, span);
        self.lower_shared_source(storage, source, span);
        self.full_expression
            .register_temporary(FullExpressionTemporary::Shared(storage));
        self.full_expression.mark_shared_effect();
        storage
    }

    pub(super) fn consume_shared_temporary(&mut self, storage: StorageId) {
        self.full_expression.remove_temporary(|candidate| {
            matches!(
                candidate,
                FullExpressionTemporary::Shared(candidate) if *candidate == storage
            )
        });
    }
}

fn lower_shared_target(target: HirSharedTarget) -> MirSharedTarget {
    match target {
        HirSharedTarget::Obj => MirSharedTarget::Obj,
        HirSharedTarget::Class(class) => MirSharedTarget::Class(class),
        HirSharedTarget::Interface(interface) => MirSharedTarget::Interface(interface),
        HirSharedTarget::Array(array) => MirSharedTarget::Array(array),
    }
}
