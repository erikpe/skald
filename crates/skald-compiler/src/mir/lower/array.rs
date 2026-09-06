//! Lowering for target-independent array ownership, projections, and slices.

use super::*;
use crate::hir::{
    HirArrayConstruction, HirArrayConstructionMode, HirArrayElementPlace, HirArrayIndex,
    HirArrayInitialize, HirArrayPlace, HirArrayReceiver, HirArrayReceiverSource, HirArraySlice,
    HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirExpressionKind,
    HirObjectDestinationInitialization, HirStoredValueInitialization, Type,
};

impl BodyLowerer<'_> {
    pub(super) fn lower_array_element_assignment(
        &mut self,
        assignment: &crate::hir::HirArrayElementAssignment,
    ) {
        let destination = self.lower_array_element_place(&assignment.destination);
        match &assignment.value {
            crate::hir::HirArrayElementValue::Value(value) => {
                let value = self
                    .lower_expression(value)
                    .expect("scalar array element value must produce a MIR value");
                self.emit(MirInstruction::Store(MirStore {
                    destination,
                    value,
                    authorization: None,
                    final_authorization: None,
                    span: assignment.span,
                }));
            }
            crate::hir::HirArrayElementValue::Array(initialization) => {
                self.lower_array_initialize(destination, initialization, true);
            }
            crate::hir::HirArrayElementValue::Object { source, operation } => {
                let source = self.lower_object_source(source);
                let MirArrayAssignElement::Class { class, .. } =
                    lower_array_assign_element(assignment.operation)
                else {
                    unreachable!("typed object array assignment selects class assignment")
                };
                self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
                    destination,
                    source,
                    class,
                    operation: lower_selected_copy_operation(*operation),
                    authorization: None,
                    final_authorization: None,
                    span: assignment.span,
                }));
            }
            crate::hir::HirArrayElementValue::Shared(transfer) => {
                let source = self.new_shared_anchor(&transfer.source, transfer.span);
                self.emit(MirInstruction::Array(MirArrayInstruction::ElementAssign {
                    destination,
                    source: MirPlace::base(source),
                    operation: lower_array_assign_element(assignment.operation),
                    span: assignment.span,
                }));
            }
            crate::hir::HirArrayElementValue::OptionalShared(value) => {
                let source = self.lower_optional_shared_source(&value.source);
                self.emit(MirInstruction::OptionalSharedAssign(
                    MirOptionalSharedAssign {
                        optional: optional_types::shared_id(
                            self.input.optional_types,
                            value.target,
                        ),
                        destination,
                        source,
                        target: lower_shared_target(value.target),
                        authorization: None,
                        final_authorization: None,
                        span: value.span,
                    },
                ));
            }
            crate::hir::HirArrayElementValue::Optional { source, payload: _ } => {
                let source = self.lower_optional_source(source);
                self.emit(MirInstruction::OptionalAssign(MirOptionalAssign {
                    destination,
                    source,
                    authorization: None,
                    final_authorization: None,
                    span: assignment.span,
                }));
            }
            crate::hir::HirArrayElementValue::ClassOptional(value) => {
                let source = self.lower_class_optional_source(&value.source);
                let source_is_absent = matches!(source, MirClassOptionalSource::Absent);
                let MirArrayAssignElement::OptionalClass {
                    class,
                    copy_constructor,
                    copy_assignment,
                } = lower_array_assign_element(assignment.operation)
                else {
                    unreachable!("typed optional-class element selects optional copy operations")
                };
                self.emit(MirInstruction::ClassOptionalAssign(
                    MirClassOptionalAssign {
                        optional: optional_types::class_id(self.input.optional_types, class),
                        destination,
                        source,
                        class,
                        copy_constructor: (!source_is_absent).then_some(copy_constructor),
                        copy_assignment: (!source_is_absent).then_some(copy_assignment),
                        authorization: None,
                        final_authorization: None,
                        span: assignment.span,
                    },
                ));
            }
            crate::hir::HirArrayElementValue::AggregateOptional(value) => {
                let source = match &value.source {
                    crate::hir::HirOptionalValueSource::Absent => {
                        MirAggregateOptionalSource::Absent
                    }
                    crate::hir::HirOptionalValueSource::Copy(source) => {
                        MirAggregateOptionalSource::Copy(
                            self.lower_aggregate_optional_place(source),
                        )
                    }
                    crate::hir::HirOptionalValueSource::Present(_)
                    | crate::hir::HirOptionalValueSource::Produced(_) => {
                        let temporary = self.new_optional_storage(
                            MirStorageKind::Temporary,
                            "aggregate-optional-element-source",
                            MirType::Optional(value.optional),
                            value.span,
                        );
                        let place = MirPlace::base(temporary);
                        self.lower_aggregate_optional_initialize_at(place.clone(), value);
                        self.full_expression.register_temporary(
                            FullExpressionTemporary::AggregateOptional(
                                MirAggregateOptionalCleanup {
                                    optional: value.optional,
                                    destination: place.clone(),
                                    span: value.span,
                                },
                            ),
                        );
                        MirAggregateOptionalSource::Copy(place)
                    }
                };
                let self_copy = matches!(&source, MirAggregateOptionalSource::Copy(source) if source == &destination);
                if !self_copy {
                    self.check_optional_mutation(destination.clone(), assignment.span);
                }
                self.emit(MirInstruction::AggregateOptionalAssign(
                    MirAggregateOptionalAssign {
                        optional: value.optional,
                        destination,
                        source,
                        authorization: None,
                        final_authorization: None,
                        span: assignment.span,
                    },
                ));
            }
        }
    }

    pub(super) fn lower_array_slice_assignment(
        &mut self,
        assignment: &crate::hir::HirArraySliceAssignment,
    ) {
        let (destination, start, end) = self.lower_array_slice_bounds(&assignment.destination);
        let (source, temporary) = match assignment.source.provenance {
            crate::hir::HirArrayProvenance::Named => (
                self.lower_array_receiver_place(&assignment.source.receiver),
                None,
            ),
            crate::hir::HirArrayProvenance::Produced => {
                let storage = self.lower_produced_array_source(&assignment.source);
                (MirPlace::base(storage), Some(storage))
            }
        };
        self.emit(MirInstruction::Array(
            MirArrayInstruction::SliceLengthCheck {
                destination_start: start,
                destination_end: end,
                source: source.clone(),
                array: assignment.source.array,
                span: assignment.span,
            },
        ));
        self.emit_array_operation_check(MirArrayFailure::SliceLengthMismatch, assignment.span);
        let source_start = self.lower_array_bound(
            source.clone(),
            assignment.source.array,
            None,
            MirArrayBoundary::Start,
            assignment.span,
        );
        self.emit(MirInstruction::Array(
            MirArrayInstruction::SliceAssignNext {
                destination,
                source,
                destination_index: start,
                source_index: source_start,
                operation: lower_array_assign_element(assignment.operation),
                span: assignment.span,
            },
        ));
        if let Some(source) = temporary {
            self.consume_array_temporary(source);
            self.emit(MirInstruction::Array(MirArrayInstruction::Release {
                owner: MirPlace::base(source),
                array: assignment.source.array,
                span: assignment.span,
            }));
        }
    }

    pub(super) fn lower_array_initialize(
        &mut self,
        destination: MirPlace,
        initialization: &HirArrayInitialize,
        replace: bool,
    ) {
        self.lower_array_transfer(destination, initialization, replace, None, None);
    }

    pub(super) fn lower_array_replace(
        &mut self,
        destination: MirPlace,
        initialization: &HirArrayInitialize,
        authorization: Option<crate::mir::MirCellWriteAuthorization>,
        final_authorization: Option<crate::mir::MirFinalWriteAuthorization>,
    ) {
        self.lower_array_transfer(
            destination,
            initialization,
            true,
            authorization,
            final_authorization,
        );
    }

    fn lower_array_transfer(
        &mut self,
        destination: MirPlace,
        initialization: &HirArrayInitialize,
        replace: bool,
        authorization: Option<crate::mir::MirCellWriteAuthorization>,
        final_authorization: Option<crate::mir::MirFinalWriteAuthorization>,
    ) {
        let produced = match initialization.operation {
            HirArrayTransfer::DeepCopy(operation) => self.lower_array_named_copy(
                &initialization.source,
                lower_array_copy_element(operation),
                initialization.span,
            ),
            HirArrayTransfer::Adopt => self.lower_produced_array_source(&initialization.source),
        };
        self.consume_array_temporary(produced);
        self.emit(MirInstruction::Array(if replace {
            MirArrayInstruction::Replace {
                destination,
                source: produced,
                array: initialization.source.array,
                authorization,
                final_authorization,
                span: initialization.span,
            }
        } else {
            MirArrayInstruction::Adopt {
                destination,
                source: produced,
                array: initialization.source.array,
                span: initialization.span,
            }
        }));
    }

    pub(super) fn lower_array_length(&mut self, length: &crate::hir::HirArrayLength) -> ValueId {
        let source = self.lower_array_receiver(&length.receiver);
        self.assign(
            MirRvalueKind::ArrayLength {
                source,
                array: length.receiver.array,
            },
            MirType::U64,
            length.span,
        )
    }

    pub(super) fn lower_array_element_place(&mut self, element: &HirArrayElementPlace) -> MirPlace {
        self.lower_array_element_place_with_anchor(element).0
    }

    pub(super) fn lower_array_alias_element_place(
        &mut self,
        element: &HirArrayElementPlace,
        access: crate::hir::HirAccess,
    ) -> MirPlace {
        self.lower_array_alias_element_place_with_anchor(element, access)
            .0
    }

    pub(super) fn lower_array_alias_element_place_with_anchor(
        &mut self,
        element: &HirArrayElementPlace,
        access: crate::hir::HirAccess,
    ) -> (MirPlace, StorageId) {
        let (source, anchor) = self.lower_array_element_place_with_anchor(element);
        let alias = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: alias,
            source: None,
            name: format!("array-alias-{}", alias.index()),
            kind: MirStorageKind::ArrayAlias(type_operations::lower_access(access)),
            ty: self.lower_type(element.element),
            span: element.span,
        });
        self.track_full_expression_storage(alias, element.span);
        self.emit(MirInstruction::Array(MirArrayInstruction::AliasBind {
            alias,
            source,
            anchor,
            span: element.span,
        }));
        (MirPlace::array_alias(alias), anchor)
    }

    fn lower_array_element_place_with_anchor(
        &mut self,
        element: &HirArrayElementPlace,
    ) -> (MirPlace, StorageId) {
        let (owner, anchor) = self.lower_array_receiver_with_anchor(&element.receiver);
        let position = self.lower_array_index(
            owner.clone(),
            element.receiver.array,
            &element.index,
            MirArrayPositionKind::Element,
        );
        (
            owner.project_array_element(element.receiver.array, position),
            anchor,
        )
    }

    pub(super) fn lower_array_place(&mut self, place: &HirArrayPlace) -> MirPlace {
        match place {
            HirArrayPlace::Binding { binding, .. } => self.lower_binding_place(*binding),
            HirArrayPlace::Field { place, .. } => self.lower_field_place(place),
            HirArrayPlace::Static { place, .. } => MirPlace::static_field(place.field),
            HirArrayPlace::Element(element) => self.lower_array_element_place(element),
        }
    }

    pub(super) fn lower_array_slice_to_produced(&mut self, slice: &HirArraySlice) -> StorageId {
        let source = self.lower_array_receiver(&slice.receiver);
        let (start, end) =
            self.lower_array_bounds(source.clone(), slice.receiver.array, &slice.bounds);
        let destination =
            self.new_array_temporary(slice.array, MirStorageKind::ArraySlice, slice.span);
        self.emit(MirInstruction::Array(MirArrayInstruction::SliceCopy {
            destination,
            source,
            start,
            end,
            array: slice.array,
            operation: lower_array_copy_element(
                slice
                    .element_copy
                    .expect("copied slice HIR must select an element copy operation"),
            ),
            span: slice.span,
        }));
        destination
    }

    pub(super) fn lower_array_slice_bounds(
        &mut self,
        slice: &HirArraySlice,
    ) -> (MirPlace, StorageId, StorageId) {
        let owner = self.lower_array_receiver(&slice.receiver);
        let (start, end) =
            self.lower_array_bounds(owner.clone(), slice.receiver.array, &slice.bounds);
        (owner, start, end)
    }

    fn lower_array_receiver(&mut self, receiver: &HirArrayReceiver) -> MirPlace {
        self.lower_array_receiver_with_anchor(receiver).0
    }

    pub(super) fn lower_array_alias_receiver_place(
        &mut self,
        receiver: &HirArrayReceiver,
    ) -> MirPlace {
        let (owner, anchor) = self.lower_array_receiver_with_anchor(receiver);
        match receiver.ownership {
            crate::hir::HirArrayReceiverOwnership::Inline => owner,
            crate::hir::HirArrayReceiverOwnership::ExplicitSharedPointee => MirPlace::base(anchor),
        }
    }

    pub(super) fn lower_optional_array_alias_place_with_anchor(
        &mut self,
        source: &crate::hir::HirOptionalOperand,
        optional: crate::identity::OptionalTypeId,
        array: crate::identity::ArrayTypeId,
        span: crate::source::Span,
    ) -> (MirPlace, StorageId) {
        let optional_place = self.lower_optional_operand(source);
        let owner = self
            .begin_optional_payload_view(optional_place, optional, MirType::Array(array), span)
            .project_checked_optional_payload(optional);
        let kind = MirArrayAnchorKind::InlineBacking;
        let anchor = self.new_array_storage(
            array,
            MirStorageKind::ArrayAnchor(kind),
            "optional-payload-anchor",
            span,
        );
        self.emit(MirInstruction::Array(MirArrayInstruction::AnchorBegin {
            anchor,
            owner: owner.clone(),
            array,
            kind,
            span,
        }));
        self.full_expression
            .register_temporary(FullExpressionTemporary::ArrayAnchor(anchor));
        (owner, anchor)
    }

    pub(super) fn lower_array_receiver_with_anchor(
        &mut self,
        receiver: &HirArrayReceiver,
    ) -> (MirPlace, StorageId) {
        let owner = match &receiver.source {
            HirArrayReceiverSource::Inline(expression) => {
                self.lower_array_expression_place(expression)
            }
            HirArrayReceiverSource::Shared(source) => {
                let anchor = self.new_shared_anchor(source, receiver.span);
                MirPlace::shared_pointee(anchor)
            }
        };
        let kind = match receiver.anchor {
            crate::hir::HirArrayAnchor::InlineOwner => MirArrayAnchorKind::InlineOwner,
            crate::hir::HirArrayAnchor::InlineBacking => MirArrayAnchorKind::InlineBacking,
            crate::hir::HirArrayAnchor::StableSharedOwner => MirArrayAnchorKind::StableSharedOwner,
            crate::hir::HirArrayAnchor::CopiedSharedOwner => MirArrayAnchorKind::CopiedSharedOwner,
            crate::hir::HirArrayAnchor::AdoptedSharedOwner => {
                MirArrayAnchorKind::AdoptedSharedOwner
            }
            crate::hir::HirArrayAnchor::SecuredOptionalSharedOwner => {
                MirArrayAnchorKind::SecuredOptionalSharedOwner
            }
        };
        let anchor = self.new_array_storage(
            receiver.array,
            MirStorageKind::ArrayAnchor(kind),
            "anchor",
            receiver.span,
        );
        self.emit(MirInstruction::Array(MirArrayInstruction::AnchorBegin {
            anchor,
            owner: owner.clone(),
            array: receiver.array,
            kind,
            span: receiver.span,
        }));
        self.full_expression
            .register_temporary(FullExpressionTemporary::ArrayAnchor(anchor));
        (owner, anchor)
    }

    pub(super) fn lower_array_receiver_place(&mut self, receiver: &HirArrayReceiver) -> MirPlace {
        self.lower_array_receiver(receiver)
    }

    fn lower_array_expression_place(&mut self, expression: &HirExpression) -> MirPlace {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => self.lower_binding_place(*binding),
            HirExpressionKind::FieldRead(place) => self.lower_field_place(place),
            HirExpressionKind::StaticRead(place) => MirPlace::static_field(place.field),
            HirExpressionKind::ArrayElement(element) => self.lower_array_element_place(element),
            HirExpressionKind::Grouped(inner) => self.lower_array_expression_place(inner),
            HirExpressionKind::DirectCall { .. }
            | HirExpressionKind::StaticCall { .. }
            | HirExpressionKind::MethodCall { .. }
            | HirExpressionKind::InterfaceCall { .. }
            | HirExpressionKind::IndirectCall(_)
            | HirExpressionKind::ArrayConstruction(_)
            | HirExpressionKind::ArraySlice(_)
            | HirExpressionKind::OptionalArrayUnwrap(_) => {
                MirPlace::base(self.lower_array_produced_expression(expression))
            }
            _ => invalid_array_hir(),
        }
    }

    fn lower_array_produced_expression(&mut self, expression: &HirExpression) -> StorageId {
        match &expression.kind {
            HirExpressionKind::DirectCall { .. }
            | HirExpressionKind::StaticCall { .. }
            | HirExpressionKind::MethodCall { .. }
            | HirExpressionKind::InterfaceCall { .. }
            | HirExpressionKind::IndirectCall(_) => {
                let Type::Array(array) = expression.ty else {
                    invalid_array_hir()
                };
                let destination =
                    self.new_array_temporary(array, MirStorageKind::ArrayProduced, expression.span);
                self.lower_array_call(expression, destination);
                destination
            }
            HirExpressionKind::ArrayConstruction(construction) => {
                self.lower_array_construction(construction)
            }
            HirExpressionKind::ArraySlice(slice) => self.lower_array_slice_to_produced(slice),
            HirExpressionKind::OptionalArrayUnwrap(unwrap) => {
                let destination = self.new_array_temporary(
                    unwrap.array,
                    MirStorageKind::ArrayProduced,
                    unwrap.span,
                );
                self.lower_optional_array_unwrap(destination, unwrap);
                destination
            }
            HirExpressionKind::Grouped(inner) => self.lower_array_produced_expression(inner),
            _ => invalid_array_hir(),
        }
    }

    fn lower_produced_array_source(&mut self, source: &HirArraySource) -> StorageId {
        let crate::hir::HirArrayReceiverSource::Inline(expression) = &source.receiver.source else {
            unreachable!("shared array sources are named owner-backed receivers")
        };
        self.lower_array_produced_expression(expression)
    }

    fn lower_array_construction(&mut self, construction: &HirArrayConstruction) -> StorageId {
        if construction.ownership != crate::hir::HirArrayOwnership::Inline {
            invalid_array_hir();
        }
        match &construction.mode {
            HirArrayConstructionMode::Empty => {
                let length = self.assign(
                    MirRvalueKind::ConstantU64(0),
                    MirType::U64,
                    construction.span,
                );
                self.lower_array_build(construction.array, length, None, None, construction.span)
            }
            HirArrayConstructionMode::DefaultLength { length, element } => {
                let length = self
                    .lower_expression(length)
                    .expect("typed array length must produce `u64`");
                self.lower_array_build(
                    construction.array,
                    length,
                    Some(lower_array_default_element(*element)),
                    None,
                    construction.span,
                )
            }
            HirArrayConstructionMode::Copy { source, element } => self.lower_array_named_copy(
                source,
                lower_array_copy_element(*element),
                construction.span,
            ),
            HirArrayConstructionMode::Indexed(initializer) => {
                let produced = self.new_array_temporary(
                    construction.array,
                    MirStorageKind::ArrayProduced,
                    construction.span,
                );
                let backing = self.lower_indexed_array(
                    construction.array,
                    initializer,
                    MirArrayOwnership::Inline,
                    construction.span,
                );
                self.emit(MirInstruction::Array(MirArrayInstruction::Publish {
                    backing,
                    destination: produced,
                    span: construction.span,
                }));
                produced
            }
            HirArrayConstructionMode::Elements(elements) => {
                let produced = self.new_array_temporary(
                    construction.array,
                    MirStorageKind::ArrayProduced,
                    construction.span,
                );
                let backing = self.lower_element_list(
                    construction.array,
                    elements,
                    MirArrayOwnership::Inline,
                    construction.span,
                );
                self.emit(MirInstruction::Array(MirArrayInstruction::Publish {
                    backing,
                    destination: produced,
                    span: construction.span,
                }));
                produced
            }
        }
    }

    pub(super) fn lower_shared_array_construction(
        &mut self,
        destination: StorageId,
        construction: &HirArrayConstruction,
    ) {
        debug_assert_eq!(
            construction.ownership,
            crate::hir::HirArrayOwnership::Shared
        );
        if let HirArrayConstructionMode::Elements(elements) = &construction.mode {
            let backing = self.lower_element_list(
                construction.array,
                elements,
                MirArrayOwnership::Shared,
                construction.span,
            );
            self.emit(MirInstruction::Array(MirArrayInstruction::PublishShared {
                backing,
                destination,
                array: construction.array,
                span: construction.span,
            }));
            return;
        }
        if let HirArrayConstructionMode::Indexed(initializer) = &construction.mode {
            let backing = self.lower_indexed_array(
                construction.array,
                initializer,
                MirArrayOwnership::Shared,
                construction.span,
            );
            self.emit(MirInstruction::Array(MirArrayInstruction::PublishShared {
                backing,
                destination,
                array: construction.array,
                span: construction.span,
            }));
            return;
        }
        let (length, default, copy) = match &construction.mode {
            HirArrayConstructionMode::Empty => (
                self.assign(
                    MirRvalueKind::ConstantU64(0),
                    MirType::U64,
                    construction.span,
                ),
                None,
                None,
            ),
            HirArrayConstructionMode::DefaultLength { length, element } => (
                self.lower_expression(length)
                    .expect("typed shared array length must produce `u64`"),
                Some(lower_array_default_element(*element)),
                None,
            ),
            HirArrayConstructionMode::Copy { source, element } => {
                let source_place = self.lower_array_receiver_place(&source.receiver);
                let length = self.assign(
                    MirRvalueKind::ArrayLength {
                        source: source_place.clone(),
                        array: source.array,
                    },
                    MirType::U64,
                    construction.span,
                );
                (
                    length,
                    None,
                    Some((source_place, lower_array_copy_element(*element))),
                )
            }
            HirArrayConstructionMode::Indexed(_) => {
                unreachable!("shared indexed construction returns after dedicated lowering")
            }
            HirArrayConstructionMode::Elements(_) => {
                unreachable!("shared element-list construction returns after dedicated lowering")
            }
        };
        let backing = self.lower_array_prefix(
            construction.array,
            length,
            MirArrayOwnership::Shared,
            default,
            copy,
            construction.span,
        );
        self.emit(MirInstruction::Array(MirArrayInstruction::PublishShared {
            backing,
            destination,
            array: construction.array,
            span: construction.span,
        }));
    }

    fn lower_indexed_array(
        &mut self,
        array: crate::identity::ArrayTypeId,
        initializer: &crate::hir::HirIndexedArrayInitialization,
        ownership: MirArrayOwnership,
        span: crate::source::Span,
    ) -> StorageId {
        let length = self
            .lower_expression(&initializer.length)
            .expect("typed indexed array length must produce `u64`");
        let length_storage =
            self.new_array_storage(array, MirStorageKind::ScalarSpill, "length", span);
        self.storage[length_storage.index()].ty = MirType::U64;
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(length_storage),
            value: length,
            authorization: None,
            final_authorization: None,
            span: initializer.length.span,
        }));

        let backing = self.new_array_storage(array, MirStorageKind::ArrayBacking, "backing", span);
        self.emit(MirInstruction::Array(MirArrayInstruction::Allocate {
            backing,
            array,
            length,
            ownership,
            failure: MirArrayFailure::AllocationSize,
            span,
        }));
        self.emit_array_operation_check(MirArrayFailure::AllocationSize, span);

        let prefix = self.new_array_storage(array, MirStorageKind::ArrayPosition, "prefix", span);
        self.storage[prefix.index()].ty = MirType::U64;
        self.emit(MirInstruction::Array(MirArrayInstruction::BeginIndexed {
            backing,
            prefix,
            length: length_storage,
            span,
        }));

        let header = self.body.allocate_block(span);
        let body = self.body.allocate_block(initializer.element.span);
        let complete = self.body.allocate_block(span);
        self.terminate(MirTerminator::Goto {
            target: header,
            span,
        });
        self.body
            .select_block(header)
            .expect("indexed array loop header exists");
        let binding = self.local_storage[initializer.binding.id.index()];
        self.terminate(MirTerminator::ArrayLoop {
            backing,
            index: prefix,
            length: length_storage,
            kind: MirArrayLoopKind::Indexed { binding },
            body_target: body,
            complete_target: complete,
            span,
        });

        self.body
            .select_block(body)
            .expect("indexed array loop body exists");
        self.begin_storage_lifetime(binding, initializer.binding.span);
        self.emit(MirInstruction::Array(MirArrayInstruction::BindIndexed {
            backing,
            prefix,
            length: length_storage,
            binding,
            span: initializer.binding.span,
        }));

        let enclosing_full_expression = std::mem::take(&mut self.full_expression);
        let enclosing_scalar_homes = std::mem::take(&mut self.scalar_result_homes);
        let enclosing_optional_guards = std::mem::take(&mut self.active_optional_guards);
        self.lower_indexed_element(array, backing, prefix, &initializer.element);
        self.finish_full_expression(initializer.element.span);
        debug_assert!(self.active_optional_guards.is_empty());
        self.full_expression = enclosing_full_expression;
        self.scalar_result_homes = enclosing_scalar_homes;
        self.active_optional_guards = enclosing_optional_guards;

        self.end_storage_lifetime(binding, initializer.binding.span);
        self.emit(MirInstruction::Array(
            MirArrayInstruction::EndIndexedElement {
                backing,
                prefix,
                length: length_storage,
                span: initializer.element.span,
            },
        ));
        self.terminate(MirTerminator::Goto {
            target: header,
            span: initializer.element.span,
        });

        self.body
            .select_block(complete)
            .expect("indexed array completion exists");
        self.emit(MirInstruction::Array(
            MirArrayInstruction::CompleteIndexed {
                backing,
                prefix,
                length: length_storage,
                span,
            },
        ));
        backing
    }

    fn lower_indexed_element(
        &mut self,
        array: crate::identity::ArrayTypeId,
        backing: StorageId,
        prefix: StorageId,
        element: &crate::hir::HirArrayElementInitialization,
    ) {
        match &element.value {
            HirStoredValueInitialization::Scalar(element) => {
                let value = self
                    .lower_expression(element)
                    .expect("typed primitive indexed element must produce a MIR value");
                self.emit(MirInstruction::Array(
                    MirArrayInstruction::InitializeIndexedElement {
                        backing,
                        prefix,
                        value,
                        span: element.span,
                    },
                ));
            }
            HirStoredValueInitialization::Class(_)
            | HirStoredValueInitialization::OptionalPrimitive { .. }
            | HirStoredValueInitialization::OptionalClass(_)
            | HirStoredValueInitialization::Array(_)
            | HirStoredValueInitialization::Shared(_)
            | HirStoredValueInitialization::OptionalShared(_)
            | HirStoredValueInitialization::Optional(_) => {
                let destination = MirPlace::base(backing).project_array_element(array, prefix);
                self.lower_stored_value_initialize_at(
                    destination,
                    element.element,
                    &element.value,
                    element.span,
                );
                self.emit(MirInstruction::Array(
                    MirArrayInstruction::AdvanceIndexedElement {
                        backing,
                        prefix,
                        span: element.span,
                    },
                ));
            }
            _ => invalid_array_hir(),
        }
    }

    fn lower_element_list(
        &mut self,
        array: crate::identity::ArrayTypeId,
        elements: &crate::hir::HirArrayElementList,
        ownership: MirArrayOwnership,
        span: crate::source::Span,
    ) -> StorageId {
        let backing = self.new_array_storage(array, MirStorageKind::ArrayBacking, "backing", span);
        let prefix = self.new_array_storage(array, MirStorageKind::ArrayPosition, "prefix", span);
        self.storage[prefix.index()].ty = MirType::U64;
        let length = u64::try_from(elements.elements.len())
            .expect("array element-list length must fit the language u64 length");
        self.emit(MirInstruction::Array(
            MirArrayInstruction::AllocateElements {
                backing,
                prefix,
                array,
                length,
                ownership,
                failure: MirArrayFailure::AllocationSize,
                span,
            },
        ));
        self.emit_array_operation_check(MirArrayFailure::AllocationSize, span);

        for (position, element) in elements.elements.iter().enumerate() {
            let position = u64::try_from(position).expect("array element position must fit u64");
            match &element.value {
                HirStoredValueInitialization::Scalar(expression) => {
                    let value = self
                        .lower_expression(expression)
                        .expect("typed primitive array element must produce a MIR value");
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::InitializeElement {
                            backing,
                            prefix,
                            position,
                            value,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::Class(initialization) => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    match initialization {
                        HirObjectDestinationInitialization::Direct { producer, .. } => {
                            self.lower_object_producer(producer, destination);
                        }
                        HirObjectDestinationInitialization::Copy {
                            source, operation, ..
                        } => {
                            let source = self.lower_object_source(source);
                            let Type::Class(class) = element.element else {
                                invalid_array_hir();
                            };
                            self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                                destination,
                                source,
                                class,
                                operation: lower_selected_copy_operation(*operation),
                                span: element.span,
                            }));
                        }
                    }
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::OptionalPrimitive { source, .. } => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.lower_optional_initialize_at(destination, source, element.span);
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::OptionalClass(initialization) => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.lower_class_optional_destination_initialize(destination, initialization);
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::Array(initialization) => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.lower_array_initialize(destination, initialization, false);
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::Shared(transfer) => {
                    let source = self.new_shared_temporary(transfer.target, transfer.span);
                    self.lower_shared_transfer(source, transfer);
                    self.consume_shared_temporary(source);
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.emit(MirInstruction::SharedFieldInitialize(
                        MirSharedFieldInitialize {
                            destination,
                            source,
                            span: element.span,
                        },
                    ));
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::OptionalShared(initialization) => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.lower_optional_shared_initialize_at(destination, initialization);
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::Optional(value) => {
                    let destination = MirPlace::base(backing).project_array_element(array, prefix);
                    self.lower_aggregate_optional_initialize_at(destination, value);
                    self.emit(MirInstruction::Array(
                        MirArrayInstruction::CompleteElement {
                            backing,
                            prefix,
                            position,
                            span: element.span,
                        },
                    ));
                }
                HirStoredValueInitialization::OptionalBoxPointeeCopy { .. } => {
                    unreachable!(
                        "optional-box pointee copies are box-payload plans, not array values"
                    )
                }
            }
        }
        backing
    }

    fn lower_array_named_copy(
        &mut self,
        source: &HirArraySource,
        operation: MirArrayCopyElement,
        span: crate::source::Span,
    ) -> StorageId {
        let array = source.array;
        let source = self.lower_array_receiver_place(&source.receiver);
        let length = self.assign(
            MirRvalueKind::ArrayLength {
                source: source.clone(),
                array,
            },
            MirType::U64,
            span,
        );
        self.lower_array_build(array, length, None, Some((source, operation)), span)
    }

    pub(super) fn lower_array_copy_from_place(
        &mut self,
        array: crate::identity::ArrayTypeId,
        source: MirPlace,
        operation: MirArrayCopyElement,
        span: crate::source::Span,
    ) -> StorageId {
        let length = self.assign(
            MirRvalueKind::ArrayLength {
                source: source.clone(),
                array,
            },
            MirType::U64,
            span,
        );
        self.lower_array_build(array, length, None, Some((source, operation)), span)
    }

    fn lower_array_build(
        &mut self,
        array: crate::identity::ArrayTypeId,
        length: ValueId,
        default: Option<MirArrayDefaultElement>,
        copy: Option<(MirPlace, MirArrayCopyElement)>,
        span: crate::source::Span,
    ) -> StorageId {
        let produced = self.new_array_temporary(array, MirStorageKind::ArrayProduced, span);
        let backing = self.lower_array_prefix(
            array,
            length,
            MirArrayOwnership::Inline,
            default,
            copy,
            span,
        );
        self.emit(MirInstruction::Array(MirArrayInstruction::Publish {
            backing,
            destination: produced,
            span,
        }));
        produced
    }

    fn lower_array_prefix(
        &mut self,
        array: crate::identity::ArrayTypeId,
        length: ValueId,
        ownership: MirArrayOwnership,
        default: Option<MirArrayDefaultElement>,
        copy: Option<(MirPlace, MirArrayCopyElement)>,
        span: crate::source::Span,
    ) -> StorageId {
        let backing = self.new_array_storage(array, MirStorageKind::ArrayBacking, "backing", span);
        let index = self.new_array_storage(array, MirStorageKind::ArrayPosition, "index", span);
        self.storage[index.index()].ty = MirType::U64;
        let length_storage =
            self.new_array_storage(array, MirStorageKind::ScalarSpill, "length", span);
        self.storage[length_storage.index()].ty = MirType::U64;
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(length_storage),
            value: length,
            authorization: None,
            final_authorization: None,
            span,
        }));
        self.emit(MirInstruction::Array(MirArrayInstruction::Allocate {
            backing,
            array,
            length,
            ownership,
            failure: MirArrayFailure::AllocationSize,
            span,
        }));
        self.emit_array_operation_check(MirArrayFailure::AllocationSize, span);
        let zero = self.assign(MirRvalueKind::ConstantU64(0), MirType::U64, span);
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(index),
            value: zero,
            authorization: None,
            final_authorization: None,
            span,
        }));

        let header = self.body.allocate_block(span);
        let body = self.body.allocate_block(span);
        let complete = self.body.allocate_block(span);
        self.terminate(MirTerminator::Goto {
            target: header,
            span,
        });
        self.body
            .select_block(header)
            .expect("array loop header exists");
        self.terminate(MirTerminator::ArrayLoop {
            backing,
            index,
            length: length_storage,
            kind: MirArrayLoopKind::Ordinary,
            body_target: body,
            complete_target: complete,
            span,
        });
        self.body
            .select_block(body)
            .expect("array loop body exists");
        if let Some(operation) = default {
            self.emit(MirInstruction::Array(MirArrayInstruction::InitializeNext {
                backing,
                index,
                operation,
                span,
            }));
        } else if let Some((source, operation)) = copy {
            self.emit(MirInstruction::Array(MirArrayInstruction::CopyNext {
                backing,
                source,
                index,
                operation,
                span,
            }));
        }
        self.terminate(MirTerminator::Goto {
            target: header,
            span,
        });
        self.body
            .select_block(complete)
            .expect("array loop completion exists");
        backing
    }

    fn lower_array_index(
        &mut self,
        owner: MirPlace,
        array: crate::identity::ArrayTypeId,
        index: &HirArrayIndex,
        kind: MirArrayPositionKind,
    ) -> StorageId {
        let value = self
            .lower_expression(&index.value)
            .expect("typed array index must produce `i64`");
        let position =
            self.new_array_storage(array, MirStorageKind::ArrayPosition, "position", index.span);
        self.storage[position.index()].ty = MirType::U64;
        self.emit(MirInstruction::Array(MirArrayInstruction::Normalize {
            destination: position,
            owner,
            index: value,
            array,
            kind,
            span: index.span,
        }));
        self.emit_array_position_check(position, kind, index.span);
        position
    }

    pub(super) fn lower_array_range_offset(
        &mut self,
        owner: MirPlace,
        array: crate::identity::ArrayTypeId,
        offset: &HirExpression,
    ) -> StorageId {
        let value = self
            .lower_expression(offset)
            .expect("typed I/O offset must produce `u64`");
        let position = self.new_array_storage(
            array,
            MirStorageKind::ArrayPosition,
            "range-offset",
            offset.span,
        );
        self.storage[position.index()].ty = MirType::U64;
        self.emit(MirInstruction::Array(MirArrayInstruction::Offset {
            destination: position,
            owner,
            offset: value,
            array,
            span: offset.span,
        }));
        self.emit_array_position_check(position, MirArrayPositionKind::RangeOffset, offset.span);
        position
    }

    fn lower_array_bounds(
        &mut self,
        owner: MirPlace,
        array: crate::identity::ArrayTypeId,
        bounds: &HirArraySliceBounds,
    ) -> (StorageId, StorageId) {
        let start = self.lower_array_bound(
            owner.clone(),
            array,
            bounds.start.as_deref(),
            MirArrayBoundary::Start,
            bounds.span,
        );
        let end = self.lower_array_bound(
            owner,
            array,
            bounds.end.as_deref(),
            MirArrayBoundary::End,
            bounds.span,
        );
        self.emit(MirInstruction::Array(
            MirArrayInstruction::SliceBoundsCheck {
                start,
                end,
                array,
                span: bounds.span,
            },
        ));
        self.emit_array_operation_check(MirArrayFailure::InvalidSliceBounds, bounds.span);
        (start, end)
    }

    fn lower_array_bound(
        &mut self,
        owner: MirPlace,
        array: crate::identity::ArrayTypeId,
        expression: Option<&HirExpression>,
        boundary: MirArrayBoundary,
        span: crate::source::Span,
    ) -> StorageId {
        let position =
            self.new_array_storage(array, MirStorageKind::ArrayPosition, "slice-bound", span);
        self.storage[position.index()].ty = MirType::U64;
        if let Some(expression) = expression {
            let index = self
                .lower_expression(expression)
                .expect("typed slice bound must produce `i64`");
            self.emit(MirInstruction::Array(MirArrayInstruction::Normalize {
                destination: position,
                owner,
                index,
                array,
                kind: MirArrayPositionKind::SliceBound,
                span,
            }));
            self.emit_array_position_check(position, MirArrayPositionKind::SliceBound, span);
        } else {
            self.emit(MirInstruction::Array(MirArrayInstruction::Boundary {
                destination: position,
                owner,
                array,
                boundary,
                span,
            }));
        }
        position
    }

    fn emit_array_position_check(
        &mut self,
        position: StorageId,
        kind: MirArrayPositionKind,
        span: crate::source::Span,
    ) {
        let success = self.body.allocate_block(span);
        let failure = self.body.allocate_block(span);
        self.terminate(MirTerminator::ArrayPositionCheck {
            position,
            kind,
            success_target: success,
            failure_target: failure,
            span,
        });
        self.body
            .select_block(failure)
            .expect("array failure block exists");
        self.terminate(MirTerminator::Terminate {
            reason: match kind {
                MirArrayPositionKind::Element => MirTerminationReason::ArrayIndexOutOfBounds,
                MirArrayPositionKind::SliceBound => MirTerminationReason::ArrayInvalidSliceBounds,
                MirArrayPositionKind::RangeOffset => MirTerminationReason::ArrayIndexOutOfBounds,
            },
            span,
        });
        self.body
            .select_block(success)
            .expect("array success block exists");
    }

    fn emit_array_operation_check(&mut self, failure: MirArrayFailure, span: crate::source::Span) {
        let success = self.body.allocate_block(span);
        let failure_target = self.body.allocate_block(span);
        self.terminate(MirTerminator::ArrayOperationCheck {
            failure,
            success_target: success,
            failure_target,
            span,
        });
        self.body
            .select_block(failure_target)
            .expect("array operation failure block exists");
        self.terminate(MirTerminator::Terminate {
            reason: match failure {
                MirArrayFailure::AllocationSize => MirTerminationReason::ArrayAllocationFailure,
                MirArrayFailure::IndexOutOfBounds => MirTerminationReason::ArrayIndexOutOfBounds,
                MirArrayFailure::InvalidSliceBounds => {
                    MirTerminationReason::ArrayInvalidSliceBounds
                }
                MirArrayFailure::SliceLengthMismatch => {
                    MirTerminationReason::ArraySliceLengthMismatch
                }
            },
            span,
        });
        self.body
            .select_block(success)
            .expect("array operation success block exists");
    }

    pub(super) fn new_array_storage(
        &mut self,
        array: crate::identity::ArrayTypeId,
        kind: MirStorageKind,
        role: &str,
        span: crate::source::Span,
    ) -> StorageId {
        let id = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id,
            source: None,
            name: format!("array-{role}-{}", id.index()),
            kind,
            ty: MirType::Array(array),
            span,
        });
        self.track_full_expression_storage(id, span);
        id
    }

    pub(super) fn new_array_temporary(
        &mut self,
        array: crate::identity::ArrayTypeId,
        kind: MirStorageKind,
        span: crate::source::Span,
    ) -> StorageId {
        let storage = self.new_array_storage(array, kind, "temporary", span);
        self.full_expression
            .register_temporary(FullExpressionTemporary::Array { storage, array });
        storage
    }

    pub(super) fn consume_array_temporary(&mut self, storage: StorageId) {
        self.full_expression.remove_temporary(|temporary| {
            matches!(
                temporary,
                FullExpressionTemporary::Array {
                    storage: candidate,
                    ..
                } if *candidate == storage
            )
        });
    }
}
