//! Rendering for MIR values, operands, places, aggregates, I/O, and object views.

use std::fmt::Write;

use crate::dump_format::write_span;

use super::super::model::*;

pub(super) fn dump_copy_operation<I: std::fmt::Display>(
    output: &mut String,
    operation: MirSelectedCopyOperation<I>,
) {
    match operation {
        MirSelectedCopyOperation::User(id) => {
            let _ = write!(output, "user {id}");
        }
        MirSelectedCopyOperation::Synthesized(class) => {
            let _ = write!(output, "synthesized {class}");
        }
    }
}

pub(super) fn dump_cell_write_authorization(
    output: &mut String,
    authorization: Option<MirCellWriteAuthorization>,
) {
    if let Some(authorization) = authorization {
        let _ = write!(output, " cell-write {}", authorization.field);
    }
}

pub(super) fn dump_final_write_authorization(
    output: &mut String,
    authorization: Option<MirFinalWriteAuthorization>,
) {
    if let Some(authorization) = authorization {
        let _ = write!(
            output,
            " final-write {} via {}",
            authorization.field, authorization.operation
        );
    }
}

pub(super) fn dump_shared_cast(output: &mut String, cast: &MirSharedCast) {
    let transfer = match cast.transfer {
        MirSharedCastTransfer::Copy => "copy",
        MirSharedCastTransfer::Adopt => "adopt",
    };
    let _ = write!(output, "{} = {transfer} ", cast.destination,);
    match &cast.source {
        MirSharedCastSource::Owner {
            storage, target, ..
        } => {
            let _ = write!(output, "{storage}: shared {target}");
        }
        MirSharedCastSource::Field { place, target } => {
            dump_place(output, place);
            let _ = write!(output, ": shared {target}");
        }
    }
    let _ = write!(output, " -> shared {}", cast.target);
}

pub(super) fn dump_object_origin(output: &mut String, origin: &MirObjectOrigin) {
    match origin {
        MirObjectOrigin::Exact {
            complete,
            dynamic_class,
        } => {
            output.push_str("exact(");
            dump_place(output, complete);
            let _ = write!(output, " : {dynamic_class})");
        }
        MirObjectOrigin::Forwarded {
            carrier,
            static_target,
            access,
            dispatch_limit,
            ..
        } => {
            let _ = write!(output, "forwarded({carrier} : ");
            dump_view_target(output, *static_target);
            let _ = write!(output, " {access}");
            if let Some(limit) = dispatch_limit {
                let _ = write!(output, " limit {limit}");
            }
            output.push(')');
        }
        MirObjectOrigin::Shared {
            owner,
            static_target,
            access,
            exact_dynamic_class,
            ..
        } => {
            let _ = write!(output, "shared({owner} : ");
            dump_view_target(output, *static_target);
            let _ = write!(output, " {access}");
            if let Some(class) = exact_dynamic_class {
                let _ = write!(output, " exact {class}");
            }
            output.push(')');
        }
    }
}

pub(super) fn dump_view_target(output: &mut String, target: MirViewTarget) {
    match target {
        MirViewTarget::Class(class) => {
            let _ = write!(output, "class {class}");
        }
        MirViewTarget::Interface(interface) => {
            let _ = write!(output, "interface {interface}");
        }
        MirViewTarget::Obj => output.push_str("Obj"),
    }
}

pub(super) fn dump_rvalue(output: &mut String, rvalue: &MirRvalue) {
    match &rvalue.kind {
        MirRvalueKind::ConstantI64(value) => {
            let _ = write!(output, "const.i64 {value}");
        }
        MirRvalueKind::ConstantU64(value) => {
            let _ = write!(output, "const.u64 {value}");
        }
        MirRvalueKind::ConstantU8(value) => {
            let _ = write!(output, "const.u8 {value}");
        }
        MirRvalueKind::ConstantF64Bits(bits) => {
            let _ = write!(output, "const.f64 0x{bits:016x}");
        }
        MirRvalueKind::ConstantBool(value) => {
            let _ = write!(output, "const.bool {value}");
        }
        MirRvalueKind::CallableAddress(address) => {
            let _ = write!(
                output,
                "callable-address {} : {}",
                address.target, address.function_type
            );
        }
        MirRvalueKind::PathCondition(condition) => {
            let _ = write!(
                output,
                "path-condition {} from {}",
                condition.condition, condition.activation
            );
        }
        MirRvalueKind::Load(place) => {
            output.push_str("load ");
            dump_place(output, place);
        }
        MirRvalueKind::Unary { operation, operand } => {
            let operation = match operation {
                MirUnaryOperation::NegateI64 => "neg.i64".to_owned(),
                MirUnaryOperation::NegateF64 => "neg.f64".to_owned(),
                MirUnaryOperation::LogicalNotBool => "not.bool".to_owned(),
                MirUnaryOperation::BitwiseComplement(integer) => {
                    format!("not.{}", integer.name())
                }
            };
            let _ = write!(output, "{operation} {operand}");
        }
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => {
            let operation = match operation {
                MirBinaryOperation::AddI64 => "add.i64".to_owned(),
                MirBinaryOperation::SubtractI64 => "sub.i64".to_owned(),
                MirBinaryOperation::MultiplyI64 => "mul.i64".to_owned(),
                MirBinaryOperation::AddU64 => "add.u64".to_owned(),
                MirBinaryOperation::SubtractU64 => "sub.u64".to_owned(),
                MirBinaryOperation::MultiplyU64 => "mul.u64".to_owned(),
                MirBinaryOperation::AddU8 => "add.u8".to_owned(),
                MirBinaryOperation::SubtractU8 => "sub.u8".to_owned(),
                MirBinaryOperation::MultiplyU8 => "mul.u8".to_owned(),
                MirBinaryOperation::AddF64 => "add.f64".to_owned(),
                MirBinaryOperation::SubtractF64 => "sub.f64".to_owned(),
                MirBinaryOperation::MultiplyF64 => "mul.f64".to_owned(),
                MirBinaryOperation::DivideF64 => "div.f64".to_owned(),
                MirBinaryOperation::IntegerBitwise { operation, operand } => {
                    format!("{}.{}", operation.mnemonic(), operand.name())
                }
            };
            let _ = write!(output, "{operation} {left}, {right}");
        }
        MirRvalueKind::IntegerDivision {
            operation,
            dividend,
            divisor,
        } => {
            let _ = write!(
                output,
                "{}.{} {dividend}, {divisor}",
                operation.mnemonic(),
                operation.operand.name()
            );
        }
        MirRvalueKind::Shift {
            operation,
            left,
            count,
        } => {
            let _ = write!(
                output,
                "{}.{} {left}, {count}",
                operation.mnemonic(),
                operation.left.name()
            );
        }
        MirRvalueKind::PrimitiveComparison {
            operation,
            left,
            right,
        } => {
            let _ = write!(
                output,
                "{}.{} {left}, {right}",
                operation.predicate.mnemonic(),
                operation.operand.name()
            );
        }
        MirRvalueKind::PrimitiveCast { operation, operand } => {
            let _ = write!(
                output,
                "cast.{}.{} {} {operand}",
                operation.source.name(),
                operation.target.name(),
                operation.kind().mnemonic()
            );
        }
        MirRvalueKind::CheckedF64ToInteger { relation, operand } => {
            let _ = write!(
                output,
                "checked-cast.f64.{} trunc=toward-zero {operand}",
                relation.target.name()
            );
        }
        MirRvalueKind::TypeTest { source, target } => {
            output.push_str("type-test ");
            dump_object_view(output, source);
            output.push_str(" is ");
            dump_view_target(output, *target);
        }
        MirRvalueKind::OptionalPresence { source, kind } => {
            let kind = match kind {
                MirPresenceTestKind::Some => "some",
                MirPresenceTestKind::None => "none",
            };
            let _ = write!(output, "optional-presence {kind} ");
            dump_place(output, source);
        }
        MirRvalueKind::OptionalBoxPresence {
            owner,
            target,
            layer,
            kind,
        } => {
            let kind = match kind {
                MirPresenceTestKind::Some => "some",
                MirPresenceTestKind::None => "none",
            };
            let _ = write!(
                output,
                "optional-box-presence {kind} owner={owner} target={target} layer={layer}"
            );
        }
        MirRvalueKind::ArrayLength { source, array } => {
            output.push_str("array-len ");
            dump_place(output, source);
            let _ = write!(output, " as {array}");
        }
    }
    let _ = write!(output, " : {}", rvalue.ty);
}

pub(super) fn dump_optional_source(output: &mut String, source: &MirOptionalSource) {
    match source {
        MirOptionalSource::Absent => output.push_str("absent"),
        MirOptionalSource::Present(value) => {
            let _ = write!(output, "present {value}");
        }
        MirOptionalSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
    }
}

pub(super) fn dump_aggregate_optional_source(
    output: &mut String,
    source: &MirAggregateOptionalSource,
) {
    match source {
        MirAggregateOptionalSource::Absent => output.push_str("absent"),
        MirAggregateOptionalSource::Unpublished => output.push_str("unpublished"),
        MirAggregateOptionalSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
    }
}

pub(super) fn dump_optional_shared_source(output: &mut String, source: &MirOptionalSharedSource) {
    match source {
        MirOptionalSharedSource::Absent => output.push_str("absent"),
        MirOptionalSharedSource::Present(owner) => {
            let _ = write!(output, "present {owner}");
        }
        MirOptionalSharedSource::Copy(place) => {
            output.push_str("copy ");
            dump_place(output, place);
        }
        MirOptionalSharedSource::Move(owner) => {
            let _ = write!(output, "move {owner}");
        }
    }
}

pub(super) fn dump_place(output: &mut String, place: &MirPlace) {
    match place.base {
        MirPlaceBase::StaticField(field) => {
            let _ = write!(output, "static({field})");
        }
        MirPlaceBase::StaticLifecycleDestination(field) => {
            let _ = write!(output, "static_destination({field})");
        }
        MirPlaceBase::Storage(storage) => {
            let _ = write!(output, "{storage}");
        }
        MirPlaceBase::AliasParameter(storage) => {
            let _ = write!(output, "indirect({storage})");
        }
        MirPlaceBase::CheckedView(storage) => {
            let _ = write!(output, "checked({storage})");
        }
        MirPlaceBase::ArrayAlias(storage) => {
            let _ = write!(output, "array-alias({storage})");
        }
        MirPlaceBase::SharedPointee(storage) => {
            let _ = write!(output, "shared-pointee({storage})");
        }
        MirPlaceBase::SharedAllocationPayload(storage) => {
            let _ = write!(output, "shared-allocation-payload({storage})");
        }
        MirPlaceBase::OptionalBoxPayload { owner, target } => {
            let _ = write!(output, "optional-box-payload({owner}, {target})");
        }
    }
    for projection in &place.projections {
        match projection {
            MirPlaceProjection::Base(base) => {
                let _ = write!(output, ".base({base})");
            }
            MirPlaceProjection::Field(field) => {
                let _ = write!(output, ".field({field})");
            }
            MirPlaceProjection::OptionalPayload(class) => {
                let _ = write!(output, ".optional-payload({class})");
            }
            MirPlaceProjection::AggregateOptionalPayload(optional) => {
                let _ = write!(output, ".optional-payload({optional})");
            }
            MirPlaceProjection::CheckedOptionalPayload(optional) => {
                let _ = write!(output, ".checked-optional-payload({optional})");
            }
            MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            } => {
                let _ = write!(output, "[{normalized_index}] as {array}");
            }
        }
    }
}

pub(super) fn dump_array_instruction(output: &mut String, instruction: &MirArrayInstruction) {
    match instruction {
        MirArrayInstruction::Allocate {
            backing,
            array,
            length,
            ownership,
            failure,
            span,
        } => {
            let _ = write!(
                output,
                "array-allocate {backing} {array} length {length} {ownership:?} failure {failure:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::AllocateElements {
            backing,
            prefix,
            array,
            length,
            ownership,
            failure,
            span,
        } => {
            let _ = write!(
                output,
                "array-allocate-elements {backing} {array} length {length} prefix {prefix} {ownership:?} failure {failure:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::BeginIndexed {
            backing,
            prefix,
            length,
            span,
        } => {
            let _ = write!(
                output,
                "array-indexed-begin {backing} prefix {prefix} length {length}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::BindIndexed {
            backing,
            prefix,
            length,
            binding,
            span,
        } => {
            let _ = write!(
                output,
                "array-indexed-bind {binding} from {backing}[{prefix}] < {length}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::InitializeIndexedElement {
            backing,
            prefix,
            value,
            span,
        } => {
            let _ = write!(
                output,
                "array-indexed-initialize {backing}[{prefix}] = {value} and advance"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::AdvanceIndexedElement {
            backing,
            prefix,
            span,
        } => {
            let _ = write!(output, "array-indexed-advance-complete {backing}[{prefix}]");
            write_span(output, *span);
        }
        MirArrayInstruction::EndIndexedElement {
            backing,
            prefix,
            length,
            span,
        } => {
            let _ = write!(
                output,
                "array-indexed-end {backing} prefix {prefix} length {length}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::CompleteIndexed {
            backing,
            prefix,
            length,
            span,
        } => {
            let _ = write!(
                output,
                "array-indexed-complete {backing} prefix {prefix} == {length}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::InitializeElement {
            backing,
            prefix,
            position,
            value,
            span,
        } => {
            let _ = write!(
                output,
                "array-initialize-element {backing}[{prefix}] position {position} = {value}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::CompleteElement {
            backing,
            prefix,
            position,
            span,
        } => {
            let _ = write!(
                output,
                "array-complete-element {backing}[{prefix}] position {position}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::InitializeNext {
            backing,
            index,
            operation,
            span,
        } => {
            let _ = write!(
                output,
                "array-initialize-next {backing}[{index}] via {operation:?}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::CopyNext {
            backing,
            source,
            index,
            operation,
            span,
        } => {
            let _ = write!(output, "array-copy-next {backing}[{index}] from ");
            dump_place(output, source);
            let _ = write!(output, " via {operation:?}");
            write_span(output, *span);
        }
        MirArrayInstruction::Publish {
            backing,
            destination,
            span,
        } => {
            let _ = write!(output, "array-publish {backing} into {destination}");
            write_span(output, *span);
        }
        MirArrayInstruction::PublishShared {
            backing,
            destination,
            array,
            span,
        } => {
            let _ = write!(
                output,
                "array-publish-shared {backing} into {destination} as {array}"
            );
            write_span(output, *span);
        }
        MirArrayInstruction::Adopt {
            destination,
            source,
            array,
            span,
            ..
        }
        | MirArrayInstruction::Replace {
            destination,
            source,
            array,
            span,
            ..
        } => {
            let verb = if matches!(instruction, MirArrayInstruction::Adopt { .. }) {
                "array-adopt"
            } else {
                "array-replace"
            };
            output.push_str(verb);
            output.push(' ');
            dump_place(output, destination);
            let _ = write!(output, " from {source} as {array}");
            if let MirArrayInstruction::Replace {
                authorization,
                final_authorization,
                ..
            } = instruction
            {
                dump_cell_write_authorization(output, *authorization);
                dump_final_write_authorization(output, *final_authorization);
            }
            write_span(output, *span);
        }
        MirArrayInstruction::Offset {
            destination,
            owner,
            offset,
            array,
            span,
        } => {
            let _ = write!(output, "array-range-offset {destination} = {offset} in ");
            dump_place(output, owner);
            let _ = write!(output, " : {array}");
            write_span(output, *span);
        }
        other => {
            let _ = write!(output, "array-op {other:?}");
        }
    }
}

pub(super) fn dump_io_instruction(output: &mut String, instruction: &MirIoInstruction) {
    let _ = write!(output, "{} = io ", instruction.result);
    match &instruction.operation {
        MirIoOperation::StandardHandle { stream } => {
            let _ = write!(output, "standard-handle stream {stream}");
        }
        MirIoOperation::Open { path, mode } => {
            output.push_str("open path ");
            dump_io_buffer(output, path);
            let _ = write!(output, " mode {mode}");
        }
        MirIoOperation::Read {
            handle,
            destination,
            offset,
        } => {
            let _ = write!(output, "read handle {handle} destination ");
            dump_io_buffer(output, destination);
            let _ = write!(output, " offset {offset}");
        }
        MirIoOperation::Write {
            handle,
            source,
            offset,
        } => {
            let _ = write!(output, "write handle {handle} source ");
            dump_io_buffer(output, source);
            let _ = write!(output, " offset {offset}");
        }
        MirIoOperation::Close { handle } => {
            let _ = write!(output, "close handle {handle}");
        }
    }
    write_span(output, instruction.span);
}

fn dump_io_buffer(output: &mut String, buffer: &MirIoBuffer) {
    dump_place(output, &buffer.place);
    let _ = write!(
        output,
        " : {} {} anchor {}",
        buffer.array, buffer.access, buffer.anchor
    );
}

pub(super) fn dump_argument(output: &mut String, argument: &MirArgument) {
    match argument {
        MirArgument::Value(value) => {
            let _ = write!(output, "value({value})");
        }
        MirArgument::Place(place) => {
            output.push_str("place(");
            dump_place(output, place);
            output.push(')');
        }
        MirArgument::View(view) => {
            dump_object_view(output, view);
        }
        MirArgument::OwnedPlace(place) => {
            output.push_str("owned(");
            dump_place(output, place);
            output.push(')');
        }
        MirArgument::SharedOwner(owner) => {
            let _ = write!(output, "shared-owner({owner})");
        }
    }
}

pub(super) fn dump_object_view(output: &mut String, view: &MirObjectView) {
    output.push_str("view(");
    dump_place(output, &view.source);
    output.push_str(" -> ");
    dump_view_target(output, view.target);
    let _ = write!(output, " {}", view.access);
    if view.provenance == MirViewProvenance::Produced {
        output.push_str(" produced");
    }
    output.push_str(" origin ");
    dump_object_origin(output, &view.origin);
    output.push(')');
}
