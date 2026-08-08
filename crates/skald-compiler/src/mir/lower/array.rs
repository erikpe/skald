//! Lowering for target-independent array ownership, projections, and slices.

use super::*;
use crate::hir::{
    HirArrayConstruction, HirArrayConstructionMode, HirArrayElementPlace, HirArrayIndex,
    HirArrayInitialize, HirArrayPlace, HirArrayReceiver, HirArrayReceiverSource, HirArraySlice,
    HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirExpressionKind,
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
                        destination,
                        source,
                        target: lower_shared_target(value.target),
                        span: value.span,
                    },
                ));
            }
            crate::hir::HirArrayElementValue::Optional { source, .. } => {
                let source = self.lower_optional_source(source);
                self.emit(MirInstruction::OptionalAssign(MirOptionalAssign {
                    destination,
                    source,
                    span: assignment.span,
                }));
            }
            crate::hir::HirArrayElementValue::ClassOptional(value) => {
                let source = self.lower_class_optional_source(&value.source);
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
                        destination,
                        source,
                        class,
                        copy_constructor: Some(copy_constructor),
                        copy_assignment: Some(copy_assignment),
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
            ty: lower_type(element.element),
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
            | HirExpressionKind::ArrayConstruction(_)
            | HirExpressionKind::ArraySlice(_) => {
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
            | HirExpressionKind::InterfaceCall { .. } => {
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
            HirArrayConstructionMode::Elements(_) => {
                self.reject_array_element_list(construction.span);
                let length = self.assign(
                    MirRvalueKind::ConstantU64(0),
                    MirType::U64,
                    construction.span,
                );
                self.lower_array_build(construction.array, length, None, None, construction.span)
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
            HirArrayConstructionMode::Elements(_) => {
                self.reject_array_element_list(construction.span);
                (
                    self.assign(
                        MirRvalueKind::ConstantU64(0),
                        MirType::U64,
                        construction.span,
                    ),
                    None,
                    None,
                )
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

    fn consume_array_temporary(&mut self, storage: StorageId) {
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
