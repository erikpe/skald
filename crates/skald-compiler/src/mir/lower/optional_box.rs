//! Optional-box allocation, unpublished payload initialization, and adoption.

use crate::{
    hir::{HirOptionalBoxAllocation, HirStoredValueInitialization, Type},
    mir::{
        MirInstruction, MirOptionalBoxCompletion, MirPlace, MirSharedAdopt, MirSharedAllocate,
        MirSharedAllocationMode, MirSharedAllocationOrigin, MirSharedAllocationTarget,
        MirSharedPublish, MirStorage, MirStorageKind, MirType, StorageId,
    },
};

use super::BodyLowerer;

enum PreparedInitialization<'hir> {
    Primitive {
        source: crate::mir::MirOptionalSource,
        span: crate::source::Span,
    },
    ClassCopy {
        optional: crate::identity::OptionalTypeId,
        class: crate::identity::ClassId,
        source: crate::mir::MirClassOptionalSource,
        operation: crate::mir::MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
        span: crate::source::Span,
    },
    OptionalShared {
        optional: crate::identity::OptionalTypeId,
        target: crate::mir::MirSharedTarget,
        source: crate::mir::MirOptionalSharedSource,
        span: crate::source::Span,
    },
    PointeeCopy {
        source: MirPlace,
        optional: crate::identity::OptionalTypeId,
        span: crate::source::Span,
    },
    Deferred(&'hir HirStoredValueInitialization),
}

impl BodyLowerer<'_> {
    pub(super) fn lower_optional_box_allocation(
        &mut self,
        destination: StorageId,
        allocation: &HirOptionalBoxAllocation,
    ) {
        let metadata = self
            .input
            .optional_box_types
            .get(allocation.exact_target)
            .expect("typed optional-box allocation must name metadata");
        debug_assert_eq!(metadata.exact_optional, Some(allocation.exact_optional));

        // Establish a whole-wrapper copy source before allocating. All other
        // selected initialization plans lower their source operands through
        // the ordinary destination-directed optional path below.
        let optional_mark = self.optional_view_mark();
        let prepared = self.prepare_optional_box_initialization(&allocation.initialization);

        let allocation_storage = self.new_optional_box_allocation_storage(allocation);
        self.emit(MirInstruction::SharedAllocate(MirSharedAllocate {
            allocation: allocation_storage,
            target: MirSharedAllocationTarget::OptionalBox {
                target: allocation.exact_target,
                optional: allocation.exact_optional,
            },
            origin: MirSharedAllocationOrigin::OptionalBox,
            mode: MirSharedAllocationMode::OptionalBox {
                completion: self.optional_box_completion(&allocation.initialization),
            },
            span: allocation.new_span,
        }));

        let payload = MirPlace::shared_allocation_payload(allocation_storage);
        match prepared {
            PreparedInitialization::Primitive { source, span } => {
                self.emit(MirInstruction::OptionalInitialize(
                    crate::mir::MirOptionalInitialize {
                        destination: payload,
                        source,
                        span,
                    },
                ));
            }
            PreparedInitialization::ClassCopy {
                optional,
                class,
                source,
                operation,
                span,
            } => self.emit(MirInstruction::ClassOptionalInitialize(
                crate::mir::MirClassOptionalInitialize {
                    optional,
                    destination: payload,
                    source,
                    class,
                    copy_constructor: Some(operation),
                    span,
                },
            )),
            PreparedInitialization::OptionalShared {
                optional,
                target,
                source,
                span,
            } => self.emit(MirInstruction::OptionalSharedInitialize(
                crate::mir::MirOptionalSharedInitialize {
                    optional,
                    destination: payload,
                    source,
                    target,
                    span,
                },
            )),
            PreparedInitialization::PointeeCopy {
                source,
                optional,
                span,
            } => self.lower_optional_copy_initialize_at(payload, optional, source, span),
            PreparedInitialization::Deferred(initialization) => self
                .lower_stored_value_initialize_at(
                    payload,
                    Type::Optional(allocation.exact_optional),
                    initialization,
                    allocation.publication_span,
                ),
        }
        self.end_optional_views_from(optional_mark, allocation.publication_span);

        self.emit(MirInstruction::SharedPublish(MirSharedPublish {
            allocation: allocation_storage,
            span: allocation.publication_span,
        }));
        self.emit(MirInstruction::SharedAdopt(MirSharedAdopt {
            destination,
            allocation: allocation_storage,
            span: allocation.span,
        }));
    }

    fn new_optional_box_allocation_storage(
        &mut self,
        allocation: &HirOptionalBoxAllocation,
    ) -> StorageId {
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("optional-box-allocation-{}", storage.index()),
            kind: MirStorageKind::SharedAllocation,
            ty: MirType::Optional(allocation.exact_optional),
            span: allocation.span,
        });
        self.track_full_expression_storage(storage, allocation.span);
        storage
    }

    fn prepare_optional_box_initialization<'hir>(
        &mut self,
        initialization: &'hir HirStoredValueInitialization,
    ) -> PreparedInitialization<'hir> {
        match initialization {
            HirStoredValueInitialization::OptionalPrimitive { source, .. } => {
                PreparedInitialization::Primitive {
                    source: self.lower_optional_source(source),
                    span: source.span(),
                }
            }
            HirStoredValueInitialization::OptionalClass(
                crate::hir::HirClassOptionalDestinationInitialization::Copy {
                    class,
                    source,
                    operation,
                    span,
                },
            ) => PreparedInitialization::ClassCopy {
                optional: super::optional_types::class_id(self.input.optional_types, *class),
                class: *class,
                source: self.lower_class_optional_copy_source(source, *class),
                operation: super::lower_selected_copy_operation(*operation),
                span: *span,
            },
            HirStoredValueInitialization::OptionalShared(initialization) => {
                PreparedInitialization::OptionalShared {
                    optional: super::optional_types::shared_id(
                        self.input.optional_types,
                        initialization.target,
                    ),
                    target: super::lower_shared_target(initialization.target),
                    source: self.lower_optional_shared_source(&initialization.source),
                    span: initialization.span,
                }
            }
            HirStoredValueInitialization::OptionalBoxPointeeCopy {
                source,
                optional,
                span,
                ..
            } => {
                let owner = self.new_shared_anchor(source, source.span());
                PreparedInitialization::PointeeCopy {
                    source: MirPlace::shared_pointee(owner),
                    optional: *optional,
                    span: *span,
                }
            }
            _ => PreparedInitialization::Deferred(initialization),
        }
    }

    fn optional_box_completion(
        &self,
        initialization: &HirStoredValueInitialization,
    ) -> MirOptionalBoxCompletion {
        use crate::hir::{
            HirClassOptionalDestinationInitialization as Class, HirOptionalStorageCategory,
            HirOptionalValueSource,
        };
        match initialization {
            HirStoredValueInitialization::OptionalPrimitive { .. } => {
                MirOptionalBoxCompletion::OptionalInitialize
            }
            HirStoredValueInitialization::OptionalClass(Class::Direct { .. }) => {
                MirOptionalBoxCompletion::ClassPublish
            }
            HirStoredValueInitialization::OptionalClass(_) => {
                MirOptionalBoxCompletion::ClassInitialize
            }
            HirStoredValueInitialization::OptionalShared(_) => {
                MirOptionalBoxCompletion::OptionalSharedInitialize
            }
            HirStoredValueInitialization::Optional(value) => match value.source {
                HirOptionalValueSource::Present(_) => MirOptionalBoxCompletion::AggregatePublish,
                HirOptionalValueSource::Produced(_) => MirOptionalBoxCompletion::DestinationCall,
                HirOptionalValueSource::Absent | HirOptionalValueSource::Copy(_) => {
                    MirOptionalBoxCompletion::AggregateInitialize
                }
            },
            HirStoredValueInitialization::OptionalBoxPointeeCopy { optional, .. } => {
                match self
                    .input
                    .optional_types
                    .get(*optional)
                    .expect("typed optional-box copy must name optional metadata")
                    .storage
                {
                    HirOptionalStorageCategory::Scalar => {
                        MirOptionalBoxCompletion::OptionalInitialize
                    }
                    HirOptionalStorageCategory::InlineClass(_) => {
                        MirOptionalBoxCompletion::ClassInitialize
                    }
                    HirOptionalStorageCategory::SharedOwner(_) => {
                        MirOptionalBoxCompletion::OptionalSharedInitialize
                    }
                    HirOptionalStorageCategory::Nested(_)
                    | HirOptionalStorageCategory::InlineArray(_) => {
                        MirOptionalBoxCompletion::AggregateInitialize
                    }
                }
            }
            _ => unreachable!("optional-box allocation must initialize an optional wrapper"),
        }
    }
}
