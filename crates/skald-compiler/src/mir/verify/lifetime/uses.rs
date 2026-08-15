//! Exhaustive storage-operand traversal for lifetime verification.

use crate::mir::*;

pub(super) fn visit_instruction_storage(
    instruction: &MirInstruction,
    visit: &mut impl FnMut(StorageId),
) {
    match instruction {
        MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
        MirInstruction::Assign(assignment) => visit_rvalue(&assignment.rvalue, visit),
        MirInstruction::Call(call) => {
            if let Some(receiver) = &call.receiver {
                visit_receiver(receiver, visit);
            }
            for argument in &call.arguments {
                visit_argument(argument, visit);
            }
            if let Some(storage) = call.shared_result {
                visit(storage);
            }
            if let Some(destination) = &call.destination {
                visit_place(destination, visit);
            }
        }
        MirInstruction::Cleanup(cleanup) => visit_place(&cleanup.destination, visit),
        MirInstruction::Initialize(initialize) => {
            visit_place(&initialize.destination, visit);
            for argument in &initialize.arguments {
                visit_argument(argument, visit);
            }
        }
        MirInstruction::Store(store) => visit_place(&store.destination, visit),
        MirInstruction::CopyConstruct(copy) => {
            visit_place(&copy.destination, visit);
            visit_place(&copy.source, visit);
        }
        MirInstruction::CopyAssign(copy) => {
            visit_place(&copy.destination, visit);
            visit_place(&copy.source, visit);
        }
        MirInstruction::EndFullExpression(end) => {
            for cleanup in &end.temporaries {
                visit_place(&cleanup.destination, visit);
            }
        }
        MirInstruction::BindCheckedView(binding) => {
            visit(binding.destination);
            visit_view(&binding.view, visit);
        }
        MirInstruction::EndCheckedView(end) => visit(end.carrier),
        MirInstruction::SharedAllocate(allocation) => {
            visit(allocation.allocation);
            if let MirSharedAllocationMode::Copy { source } = &allocation.mode {
                visit_place(source, visit);
            }
        }
        MirInstruction::SharedInitialize(initialize) => {
            visit(initialize.allocation);
            for argument in &initialize.arguments {
                visit_argument(argument, visit);
            }
        }
        MirInstruction::SharedPublish(publish) => visit(publish.allocation),
        MirInstruction::SharedStatic(static_owner) => visit(static_owner.destination),
        MirInstruction::SharedAdopt(adopt) => {
            visit(adopt.destination);
            visit(adopt.allocation);
        }
        MirInstruction::SharedCopy(copy) => {
            visit(copy.destination);
            visit(copy.source);
        }
        MirInstruction::SharedFieldCopy(copy) => {
            visit(copy.destination);
            visit_place(&copy.source, visit);
        }
        MirInstruction::SharedCast(cast) => visit_shared_cast(cast, visit),
        MirInstruction::SharedMove(transfer) => {
            visit(transfer.destination);
            visit(transfer.source);
        }
        MirInstruction::SharedRelease(release) => visit(release.owner),
        MirInstruction::SharedFieldInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit(initialize.source);
        }
        MirInstruction::SharedFieldReplace(replace) => {
            visit_place(&replace.destination, visit);
            visit(replace.source);
        }
        MirInstruction::StringInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit(initialize.backing);
        }
        MirInstruction::OptionalInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit_optional_source(&initialize.source, visit);
        }
        MirInstruction::OptionalAssign(assignment) => {
            visit_place(&assignment.destination, visit);
            visit_optional_source(&assignment.source, visit);
        }
        MirInstruction::AggregateOptionalInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit_aggregate_optional_source(&initialize.source, visit);
        }
        MirInstruction::AggregateOptionalAssign(assignment) => {
            visit_place(&assignment.destination, visit);
            visit_aggregate_optional_source(&assignment.source, visit);
        }
        MirInstruction::AggregateOptionalPublish(publish) => {
            visit_place(&publish.destination, visit)
        }
        MirInstruction::AggregateOptionalCleanup(cleanup) => {
            visit_place(&cleanup.destination, visit)
        }
        MirInstruction::ClassOptionalInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit_class_optional_source(&initialize.source, visit);
        }
        MirInstruction::ClassOptionalAssign(assignment) => {
            visit_place(&assignment.destination, visit);
            visit_class_optional_source(&assignment.source, visit);
        }
        MirInstruction::ClassOptionalPublish(publish) => visit_place(&publish.destination, visit),
        MirInstruction::ClassOptionalCleanup(cleanup) => visit_place(&cleanup.destination, visit),
        MirInstruction::EndOptionalView(end) => visit_place(&end.source, visit),
        MirInstruction::EndOptionalBoxView(end) => visit(end.owner),
        MirInstruction::OptionalSharedInitialize(initialize) => {
            visit_place(&initialize.destination, visit);
            visit_optional_shared_source(&initialize.source, visit);
        }
        MirInstruction::OptionalSharedAssign(assignment) => {
            visit_place(&assignment.destination, visit);
            visit_optional_shared_source(&assignment.source, visit);
        }
        MirInstruction::OptionalSharedCleanup(cleanup) => visit_place(&cleanup.destination, visit),
        MirInstruction::Array(array) => visit_array_instruction(array, visit),
        MirInstruction::Io(io) => match &io.operation {
            MirIoOperation::StandardHandle { .. } | MirIoOperation::Close { .. } => {}
            MirIoOperation::Open { path, .. } => visit_io_buffer(path, visit),
            MirIoOperation::Read {
                destination,
                offset,
                ..
            } => {
                visit_io_buffer(destination, visit);
                visit(*offset);
            }
            MirIoOperation::Write { source, offset, .. } => {
                visit_io_buffer(source, visit);
                visit(*offset);
            }
        },
    }
}

pub(super) fn visit_terminator_storage(
    terminator: &MirTerminator,
    visit: &mut impl FnMut(StorageId),
) {
    match terminator {
        MirTerminator::Return { .. }
        | MirTerminator::Goto { .. }
        | MirTerminator::Branch { .. }
        | MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::Terminate { .. } => {}
        MirTerminator::ShiftCountCheck { check, .. } => {
            visit(check.left);
            visit(check.count);
            visit(check.result);
        }
        MirTerminator::IntegerDivisorCheck { check, .. } => {
            visit(check.dividend);
            visit(check.divisor);
            visit(check.result);
        }
        MirTerminator::PrimitiveCastRangeCheck { check, .. } => {
            visit(check.source);
            visit(check.result);
        }
        MirTerminator::ReturnShared { owner, .. }
        | MirTerminator::ReturnOptionalShared { owner, .. } => visit(*owner),
        MirTerminator::Panic { message, .. } => visit_place(message, visit),
        MirTerminator::CheckedCast { binding, .. } => {
            visit(binding.destination);
            visit_view(&binding.view, visit);
        }
        MirTerminator::SharedCast { cast, .. } => visit_shared_cast(cast, visit),
        MirTerminator::OptionalUnwrap {
            source,
            destination,
            ..
        } => {
            visit_place(source, visit);
            visit(*destination);
        }
        MirTerminator::OptionalSharedUnwrap { unwrap, .. } => {
            visit_place(&unwrap.source, visit);
            visit(unwrap.destination);
        }
        MirTerminator::BeginOptionalView { begin, .. } => visit_place(&begin.source, visit),
        MirTerminator::BeginOptionalBoxView { begin, .. } => visit(begin.owner),
        MirTerminator::CheckOptionalMutation { source, .. } => visit_place(source, visit),
        MirTerminator::ArrayPositionCheck { position, .. } => visit(*position),
        MirTerminator::ArrayLoop {
            backing,
            index,
            length,
            ..
        } => {
            visit(*backing);
            visit(*index);
            visit(*length);
        }
    }
}

fn visit_rvalue(rvalue: &MirRvalue, visit: &mut impl FnMut(StorageId)) {
    match &rvalue.kind {
        MirRvalueKind::PathCondition(condition) => visit(condition.activation),
        MirRvalueKind::Load(place)
        | MirRvalueKind::OptionalPresence { source: place, .. }
        | MirRvalueKind::ArrayLength { source: place, .. } => visit_place(place, visit),
        MirRvalueKind::OptionalBoxPresence { owner, .. } => visit(*owner),
        MirRvalueKind::TypeTest { source, .. } => visit_view(source, visit),
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::Unary { .. }
        | MirRvalueKind::Binary { .. }
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::PrimitiveComparison { .. }
        | MirRvalueKind::PrimitiveCast { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. } => {}
    }
}

fn visit_place(place: &MirPlace, visit: &mut impl FnMut(StorageId)) {
    if let Some(storage) = place.base.local_storage() {
        visit(storage);
    }
    for projection in &place.projections {
        if let MirPlaceProjection::ArrayElement {
            normalized_index, ..
        } = projection
        {
            visit(*normalized_index);
        }
    }
}

fn visit_argument(argument: &MirArgument, visit: &mut impl FnMut(StorageId)) {
    match argument {
        MirArgument::Value(_) => {}
        MirArgument::Place(place) | MirArgument::OwnedPlace(place) => visit_place(place, visit),
        MirArgument::View(view) => visit_view(view, visit),
        MirArgument::SharedOwner(owner) => visit(*owner),
    }
}

fn visit_receiver(receiver: &MirCallReceiver, visit: &mut impl FnMut(StorageId)) {
    match receiver {
        MirCallReceiver::Method(receiver) => {
            visit_place(&receiver.place, visit);
            visit_origin(&receiver.origin, visit);
        }
        MirCallReceiver::Interface(view) => visit_view(view, visit),
    }
}

fn visit_view(view: &MirObjectView, visit: &mut impl FnMut(StorageId)) {
    visit_place(&view.source, visit);
    visit_origin(&view.origin, visit);
}

fn visit_origin(origin: &MirObjectOrigin, visit: &mut impl FnMut(StorageId)) {
    match origin {
        MirObjectOrigin::Exact { complete, .. } => visit_place(complete, visit),
        MirObjectOrigin::Forwarded { carrier, .. } => visit(*carrier),
        MirObjectOrigin::Shared { owner, .. } => visit(*owner),
    }
}

fn visit_shared_cast(cast: &MirSharedCast, visit: &mut impl FnMut(StorageId)) {
    visit(cast.destination);
    match &cast.source {
        MirSharedCastSource::Owner { storage, .. } => visit(*storage),
        MirSharedCastSource::Field { place, .. } => visit_place(place, visit),
    }
}

fn visit_optional_source(source: &MirOptionalSource, visit: &mut impl FnMut(StorageId)) {
    if let MirOptionalSource::Copy(place) = source {
        visit_place(place, visit);
    }
}

fn visit_aggregate_optional_source(
    source: &MirAggregateOptionalSource,
    visit: &mut impl FnMut(StorageId),
) {
    if let MirAggregateOptionalSource::Copy(place) = source {
        visit_place(place, visit);
    }
}

fn visit_class_optional_source(source: &MirClassOptionalSource, visit: &mut impl FnMut(StorageId)) {
    match source {
        MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) => {
            visit_place(place, visit)
        }
        MirClassOptionalSource::Absent => {}
    }
}

fn visit_optional_shared_source(
    source: &MirOptionalSharedSource,
    visit: &mut impl FnMut(StorageId),
) {
    match source {
        MirOptionalSharedSource::Present(storage) | MirOptionalSharedSource::Move(storage) => {
            visit(*storage)
        }
        MirOptionalSharedSource::Copy(place) => visit_place(place, visit),
        MirOptionalSharedSource::Absent => {}
    }
}

fn visit_array_instruction(instruction: &MirArrayInstruction, visit: &mut impl FnMut(StorageId)) {
    match instruction {
        MirArrayInstruction::Allocate { backing, .. } => visit(*backing),
        MirArrayInstruction::AllocateElements {
            backing, prefix, ..
        }
        | MirArrayInstruction::InitializeElement {
            backing, prefix, ..
        }
        | MirArrayInstruction::CompleteElement {
            backing, prefix, ..
        } => {
            visit(*backing);
            visit(*prefix);
        }
        MirArrayInstruction::InitializeNext { backing, index, .. } => {
            visit(*backing);
            visit(*index);
        }
        MirArrayInstruction::CopyNext {
            backing,
            source,
            index,
            ..
        } => {
            visit(*backing);
            visit_place(source, visit);
            visit(*index);
        }
        MirArrayInstruction::Publish {
            backing,
            destination,
            ..
        }
        | MirArrayInstruction::PublishShared {
            backing,
            destination,
            ..
        } => {
            visit(*backing);
            visit(*destination);
        }
        MirArrayInstruction::Adopt {
            destination,
            source,
            ..
        }
        | MirArrayInstruction::Replace {
            destination,
            source,
            ..
        } => {
            visit_place(destination, visit);
            visit(*source);
        }
        MirArrayInstruction::ElementAssign {
            destination,
            source,
            ..
        } => {
            visit_place(destination, visit);
            visit_place(source, visit);
        }
        MirArrayInstruction::DestroyNext { owner, index, .. } => {
            visit_place(owner, visit);
            visit(*index);
        }
        MirArrayInstruction::Release { owner, .. } => visit_place(owner, visit),
        MirArrayInstruction::AnchorBegin { anchor, owner, .. } => {
            visit(*anchor);
            visit_place(owner, visit);
        }
        MirArrayInstruction::AnchorEnd { anchor, .. } => visit(*anchor),
        MirArrayInstruction::AliasBind {
            alias,
            source,
            anchor,
            ..
        } => {
            visit(*alias);
            visit_place(source, visit);
            visit(*anchor);
        }
        MirArrayInstruction::Normalize {
            destination, owner, ..
        }
        | MirArrayInstruction::Offset {
            destination, owner, ..
        }
        | MirArrayInstruction::Boundary {
            destination, owner, ..
        } => {
            visit(*destination);
            visit_place(owner, visit);
        }
        MirArrayInstruction::SliceCopy {
            destination,
            source,
            start,
            end,
            ..
        } => {
            visit(*destination);
            visit_place(source, visit);
            visit(*start);
            visit(*end);
        }
        MirArrayInstruction::SliceLengthCheck {
            destination_start,
            destination_end,
            source,
            ..
        } => {
            visit(*destination_start);
            visit(*destination_end);
            visit_place(source, visit);
        }
        MirArrayInstruction::SliceBoundsCheck { start, end, .. } => {
            visit(*start);
            visit(*end);
        }
        MirArrayInstruction::SliceAssignNext {
            destination,
            source,
            destination_index,
            source_index,
            ..
        } => {
            visit_place(destination, visit);
            visit_place(source, visit);
            visit(*destination_index);
            visit(*source_index);
        }
    }
}

fn visit_io_buffer(buffer: &MirIoBuffer, visit: &mut impl FnMut(StorageId)) {
    visit_place(&buffer.place, visit);
    visit(buffer.anchor);
}
