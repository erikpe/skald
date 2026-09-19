//! Generated primitive/trivial array lifecycle bodies.

use super::LowerError;
use crate::backend::{
    lir::{
        BinaryOperation, Call, CallArgument, CallAttribution, CallTarget, Constant, DraftBuilder,
        Edge, MemoryRepresentation, Operation, Terminator, ValueHandle, VerifiedCallable,
    },
    plan::{
        ArtifactId, CallableBinding, ComponentRole, HelperFamily, HelperKey, LirCallableId,
        PlanError, PlanView, RuntimeService, ScalarType,
    },
    planning::AdmittedProgram,
};
use crate::primitive_comparison::PrimitiveComparisonPredicate;

pub(super) fn lower<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    key: HelperKey,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = admitted.plan().view();
    plan.require_same_context(owner.context())?;
    let candidates = plan
        .semantic()
        .arrays
        .iter()
        .filter(|array| array.descriptor_layout == key.layout)
        .collect::<Vec<_>>();
    let [array] = candidates.as_slice() else {
        return Err(PlanError::InvalidDomain.into());
    };
    if !primitive_trivial(array) {
        return Err(PlanError::InvalidDomain.into());
    }
    match key.family {
        HelperFamily::ArrayElementInitializer => initializer(plan, owner, array),
        HelperFamily::ArrayElementCopier => copier(plan, owner, array),
        HelperFamily::ArrayClone => clone_array(plan, owner, array),
        HelperFamily::ArrayElementDestroyer => no_op(owner, 2),
        HelperFamily::ArrayRelease => release(plan, owner, array),
        HelperFamily::ArraySharedFinalizer => shared_finalizer(plan, owner, array),
        _ => Err(PlanError::InvalidDomain.into()),
    }
}

fn primitive_trivial(array: &crate::backend::plan::ArrayLayoutFact) -> bool {
    use crate::backend::plan::{
        ArrayAssignElementFact, ArrayCopyElementFact, ArrayDefaultElementFact,
        ArrayDestroyElementFact,
    };
    array
        .default
        .is_none_or(|item| item == ArrayDefaultElementFact::Primitive)
        && array
            .copy
            .is_none_or(|item| item == ArrayCopyElementFact::Primitive)
        && array
            .assignment
            .is_none_or(|item| item == ArrayAssignElementFact::Primitive)
        && array.destruction == ArrayDestroyElementFact::Trivial
}

fn initializer<'plan>(
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let (mut builder, entry, inputs) = begin(owner)?;
    let [backing, index] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let address = element_address(
        &mut builder,
        entry,
        *backing,
        *index,
        array,
        array.element_offset,
    )?;
    let representation = element_representation(plan, array)?;
    let zero = builder.append(entry, Operation::Constant(zero(representation.scalar)?))?[0];
    builder.append(
        entry,
        Operation::Store {
            address,
            value: zero,
            representation,
        },
    )?;
    finish(builder, entry, vec![])
}

fn copier<'plan>(
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let (mut builder, entry, inputs) = begin(owner)?;
    let [destination, source, destination_index, source_index] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let representation = element_representation(plan, array)?;
    let source = element_address(
        &mut builder,
        entry,
        *source,
        *source_index,
        array,
        array.element_offset,
    )?;
    let value = builder.append(
        entry,
        Operation::Load {
            address: source,
            representation,
        },
    )?[0];
    let destination = element_address(
        &mut builder,
        entry,
        *destination,
        *destination_index,
        array,
        array.element_offset,
    )?;
    builder.append(
        entry,
        Operation::Store {
            address: destination,
            value,
            representation,
        },
    )?;
    finish(builder, entry, vec![])
}

fn clone_array<'plan>(
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let boundary = owner.key();
    let (mut builder, entry, inputs) = begin(owner)?;
    let [source] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let null = builder.append(
        entry,
        Operation::Constant(Constant::Null(ScalarType::DataAddress)),
    )?[0];
    let present = compare(
        &mut builder,
        entry,
        PrimitiveComparisonPredicate::NotEqual,
        *source,
        null,
    )?;
    let empty = reserve_block(&mut builder)?;
    let allocate = reserve_block(&mut builder)?;
    let result = builder.reserve_value(ScalarType::DataAddress, None)?;
    let complete = builder.reserve_block()?;
    builder.define_block(complete, &[result])?;
    builder.terminate(
        entry,
        Terminator::Branch {
            condition: present,
            true_edge: edge(allocate),
            false_edge: edge(empty),
        },
    )?;
    builder.terminate(empty, Terminator::Jump(edge_with(complete, null)))?;

    let length_address = byte_offset(&mut builder, allocate, *source, array.inline_length_offset)?;
    let length = builder.append(
        allocate,
        Operation::Load {
            address: length_address,
            representation: u64_representation(),
        },
    )?[0];
    let stride = builder.append(
        allocate,
        Operation::Constant(Constant::U64(array.stride as u64)),
    )?[0];
    let element_bytes = builder.append(
        allocate,
        Operation::Binary {
            operation: BinaryOperation::Multiply,
            left: length,
            right: stride,
        },
    )?[0];
    let header = builder.append(
        allocate,
        Operation::Constant(Constant::U64(array.element_offset as u64)),
    )?[0];
    let bytes = builder.append(
        allocate,
        Operation::Binary {
            operation: BinaryOperation::Add,
            left: element_bytes,
            right: header,
        },
    )?[0];
    let results = runtime_call(
        plan,
        &mut builder,
        allocate,
        boundary,
        RuntimeService::Allocate,
        bytes,
    )?;
    let [destination] = results.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let destination = *destination;
    let one = constant_u64(&mut builder, allocate, 1)?;
    store_offset(
        &mut builder,
        allocate,
        destination,
        array.inline_owner_count_offset,
        one,
    )?;
    store_offset(
        &mut builder,
        allocate,
        destination,
        array.inline_length_offset,
        length,
    )?;

    let zero = constant_u64(&mut builder, allocate, 0)?;
    let index = builder.reserve_value(ScalarType::U64, None)?;
    let loop_header = builder.reserve_block()?;
    builder.define_block(loop_header, &[index])?;
    builder.terminate(allocate, Terminator::Jump(edge_with(loop_header, zero)))?;
    let running = compare(
        &mut builder,
        loop_header,
        PrimitiveComparisonPredicate::LessThan,
        index,
        length,
    )?;
    let body = reserve_block(&mut builder)?;
    builder.terminate(
        loop_header,
        Terminator::Branch {
            condition: running,
            true_edge: edge(body),
            false_edge: edge_with(complete, destination),
        },
    )?;
    call_helper(
        plan,
        &mut builder,
        body,
        boundary,
        array.descriptor_layout,
        HelperFamily::ArrayElementCopier,
        vec![destination, *source, index, index],
    )?;
    let one = constant_u64(&mut builder, body, 1)?;
    let next = builder.append(
        body,
        Operation::Binary {
            operation: BinaryOperation::Add,
            left: index,
            right: one,
        },
    )?[0];
    builder.terminate(body, Terminator::Jump(edge_with(loop_header, next)))?;
    finish(builder, complete, vec![result])
}

fn release<'plan>(
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let boundary = owner.key();
    let (mut builder, entry, inputs) = begin(owner)?;
    let [handle] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let null = builder.append(
        entry,
        Operation::Constant(Constant::Null(ScalarType::DataAddress)),
    )?[0];
    let present = compare(
        &mut builder,
        entry,
        PrimitiveComparisonPredicate::NotEqual,
        *handle,
        null,
    )?;
    let inspect = reserve_block(&mut builder)?;
    let complete = reserve_block(&mut builder)?;
    builder.terminate(
        entry,
        Terminator::Branch {
            condition: present,
            true_edge: edge(inspect),
            false_edge: edge(complete),
        },
    )?;
    let count_address = byte_offset(
        &mut builder,
        inspect,
        *handle,
        array.inline_owner_count_offset,
    )?;
    let count = builder.append(
        inspect,
        Operation::Load {
            address: count_address,
            representation: u64_representation(),
        },
    )?[0];
    let one = constant_u64(&mut builder, inspect, 1)?;
    let zero = constant_u64(&mut builder, inspect, 0)?;
    let valid = compare(
        &mut builder,
        inspect,
        PrimitiveComparisonPredicate::NotEqual,
        count,
        zero,
    )?;
    let owned = reserve_block(&mut builder)?;
    let invalid = reserve_block(&mut builder)?;
    builder.terminate(
        inspect,
        Terminator::Branch {
            condition: valid,
            true_edge: edge(owned),
            false_edge: edge(invalid),
        },
    )?;
    builder.terminate(invalid, Terminator::HardTrap)?;
    let last = compare(
        &mut builder,
        owned,
        PrimitiveComparisonPredicate::Equal,
        count,
        one,
    )?;
    let finalize = reserve_block(&mut builder)?;
    let decrement = reserve_block(&mut builder)?;
    builder.terminate(
        owned,
        Terminator::Branch {
            condition: last,
            true_edge: edge(finalize),
            false_edge: edge(decrement),
        },
    )?;
    let reduced = builder.append(
        decrement,
        Operation::Binary {
            operation: BinaryOperation::Subtract,
            left: count,
            right: one,
        },
    )?[0];
    builder.append(
        decrement,
        Operation::Store {
            address: count_address,
            value: reduced,
            representation: u64_representation(),
        },
    )?;
    builder.terminate(decrement, Terminator::Jump(edge(complete)))?;
    let payload = byte_offset(&mut builder, finalize, *handle, array.inline_length_offset)?;
    call_helper(
        plan,
        &mut builder,
        finalize,
        boundary,
        array.descriptor_layout,
        HelperFamily::ArraySharedFinalizer,
        vec![payload],
    )?;
    if !runtime_call(
        plan,
        &mut builder,
        finalize,
        boundary,
        RuntimeService::Free,
        *handle,
    )?
    .is_empty()
    {
        return Err(PlanError::InvalidSignature.into());
    }
    builder.terminate(finalize, Terminator::Jump(edge(complete)))?;
    finish(builder, complete, vec![])
}

fn shared_finalizer<'plan>(
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let boundary = owner.key();
    let (mut builder, entry, inputs) = begin(owner)?;
    let [payload] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let length = builder.append(
        entry,
        Operation::Load {
            address: *payload,
            representation: u64_representation(),
        },
    )?[0];
    let zero = constant_u64(&mut builder, entry, 0)?;
    let index = builder.reserve_value(ScalarType::U64, None)?;
    let header = builder.reserve_block()?;
    builder.define_block(header, &[index])?;
    builder.terminate(entry, Terminator::Jump(edge_with(header, zero)))?;
    let running = compare(
        &mut builder,
        header,
        PrimitiveComparisonPredicate::LessThan,
        index,
        length,
    )?;
    let body = reserve_block(&mut builder)?;
    let complete = reserve_block(&mut builder)?;
    builder.terminate(
        header,
        Terminator::Branch {
            condition: running,
            true_edge: edge(body),
            false_edge: edge(complete),
        },
    )?;
    // Element destroyers consume a descriptor whose element start is at the
    // planned inline offset. The payload starts at its length field, so the
    // element displacement relative to it is identical.
    call_helper(
        plan,
        &mut builder,
        body,
        boundary,
        array.descriptor_layout,
        HelperFamily::ArrayElementDestroyer,
        vec![*payload, index],
    )?;
    let one = constant_u64(&mut builder, body, 1)?;
    let next = builder.append(
        body,
        Operation::Binary {
            operation: BinaryOperation::Add,
            left: index,
            right: one,
        },
    )?[0];
    builder.terminate(body, Terminator::Jump(edge_with(header, next)))?;
    finish(builder, complete, vec![])
}

fn no_op<'plan>(
    owner: CallableBinding<'plan>,
    inputs: usize,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let (builder, entry, actual) = begin(owner)?;
    if actual.len() != inputs {
        return Err(PlanError::InvalidSignature.into());
    }
    finish(builder, entry, vec![])
}

fn begin<'plan>(
    owner: CallableBinding<'plan>,
) -> Result<
    (
        DraftBuilder<'plan>,
        crate::backend::lir::BlockHandle<'plan>,
        Vec<ValueHandle<'plan>>,
    ),
    LowerError,
> {
    let mut builder = DraftBuilder::new(owner)?;
    let entry = builder.reserve_block()?;
    builder.define_block(entry, &[])?;
    builder.set_entry(entry)?;
    let inputs = builder.inputs().collect();
    Ok((builder, entry, inputs))
}

fn finish<'plan>(
    mut builder: DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    values: Vec<ValueHandle<'plan>>,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    builder.terminate(block, Terminator::Return(values))?;
    crate::backend::lir::verify_callable(builder.finish()).map_err(LowerError::Verification)
}

fn element_address<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    backing: ValueHandle<'plan>,
    index: ValueHandle<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
    offset: usize,
) -> Result<ValueHandle<'plan>, LowerError> {
    let base = byte_offset(builder, block, backing, offset)?;
    let stride = constant_u64(builder, block, array.stride as u64)?;
    let offset = builder.append(
        block,
        Operation::Binary {
            operation: BinaryOperation::Multiply,
            left: index,
            right: stride,
        },
    )?[0];
    Ok(builder.append(block, Operation::ByteOffset { base, offset })?[0])
}

fn element_representation(
    plan: PlanView<'_>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<MemoryRepresentation, LowerError> {
    let layout = plan.layout(plan.layout_id(array.element_layout.index())?)?;
    let scalar = match array.element {
        crate::backend::plan::SemanticType::I64 => ScalarType::I64,
        crate::backend::plan::SemanticType::U64 => ScalarType::U64,
        crate::backend::plan::SemanticType::U8 => ScalarType::U8,
        crate::backend::plan::SemanticType::Bool => ScalarType::Bool,
        crate::backend::plan::SemanticType::F64 => ScalarType::F64,
        _ => return Err(PlanError::InvalidDomain.into()),
    };
    Ok(MemoryRepresentation {
        scalar,
        bytes: layout.size,
        alignment: layout.alignment,
    })
}

fn zero(ty: ScalarType) -> Result<Constant, LowerError> {
    Ok(match ty {
        ScalarType::I64 => Constant::I64(0),
        ScalarType::U64 => Constant::U64(0),
        ScalarType::U8 => Constant::U8(0),
        ScalarType::Bool => Constant::Bool(false),
        ScalarType::F64 => Constant::F64(0),
        _ => return Err(PlanError::InvalidDomain.into()),
    })
}

fn call_helper<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    boundary: LirCallableId,
    layout: crate::backend::plan::LayoutId,
    family: HelperFamily,
    values: Vec<ValueHandle<'plan>>,
) -> Result<(), LowerError> {
    let target = plan
        .resources()
        .generated
        .iter()
        .find_map(|item| match item.callable {
            LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                Some(item.callable)
            }
            _ => None,
        })
        .ok_or(PlanError::UnknownDeclaration)?;
    let binding = plan.callable(target)?;
    let signature = binding.signature_id();
    let signature_fact = binding.signature()?;
    if signature_fact.inputs.len() != values.len() {
        return Err(PlanError::InvalidSignature.into());
    }
    let arguments = signature_fact
        .inputs
        .iter()
        .zip(values)
        .map(|(component, value)| CallArgument {
            role: component.role,
            value,
        })
        .collect();
    builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(ArtifactId::Callable(target)),
            signature,
            arguments,
            attribution: CallAttribution::InheritedOperation { boundary },
        }),
    )?;
    Ok(())
}

fn runtime_call<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    boundary: LirCallableId,
    service: RuntimeService,
    value: ValueHandle<'plan>,
) -> Result<Vec<ValueHandle<'plan>>, LowerError> {
    let target = ArtifactId::Runtime(service);
    let signature = plan
        .artifact(plan.artifact_id(target)?, target.category())?
        .signature
        .ok_or(PlanError::InvalidSignature)?;
    Ok(builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(target),
            signature,
            arguments: vec![CallArgument {
                role: ComponentRole::RuntimeParameter(0),
                value,
            }],
            attribution: CallAttribution::InheritedOperation { boundary },
        }),
    )?)
}

fn byte_offset<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    base: ValueHandle<'plan>,
    bytes: usize,
) -> Result<ValueHandle<'plan>, LowerError> {
    let offset = constant_u64(builder, block, bytes as u64)?;
    Ok(builder.append(block, Operation::ByteOffset { base, offset })?[0])
}

fn store_offset<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    base: ValueHandle<'plan>,
    bytes: usize,
    value: ValueHandle<'plan>,
) -> Result<(), LowerError> {
    let address = byte_offset(builder, block, base, bytes)?;
    builder.append(
        block,
        Operation::Store {
            address,
            value,
            representation: u64_representation(),
        },
    )?;
    Ok(())
}

fn constant_u64<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    value: u64,
) -> Result<ValueHandle<'plan>, LowerError> {
    Ok(builder.append(block, Operation::Constant(Constant::U64(value)))?[0])
}

fn compare<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    predicate: PrimitiveComparisonPredicate,
    left: ValueHandle<'plan>,
    right: ValueHandle<'plan>,
) -> Result<ValueHandle<'plan>, LowerError> {
    Ok(builder.append(
        block,
        Operation::Compare {
            predicate,
            left,
            right,
        },
    )?[0])
}

fn reserve_block<'plan>(
    builder: &mut DraftBuilder<'plan>,
) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
    let block = builder.reserve_block()?;
    builder.define_block(block, &[])?;
    Ok(block)
}

fn edge<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
) -> Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    Edge {
        target,
        arguments: vec![],
    }
}

fn edge_with<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
    value: ValueHandle<'plan>,
) -> Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    Edge {
        target,
        arguments: vec![value],
    }
}

fn u64_representation() -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    }
}
