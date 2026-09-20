//! Checked program-lifecycle coordinators built from frozen activation facts.

use super::{
    generated::{class_helper, emit_process_boundary_optional_cleanup},
    LowerError,
};
use crate::backend::{
    lir::{
        verify_callable, Call, CallArgument, CallAttribution, CallTarget, DraftBuilder,
        MemoryRepresentation, Operation, Terminator, ValueHandle, VerifiedCallable,
    },
    plan::{
        ArtifactId, CallableBinding, ComponentRole, Coordinator, DataKey, HelperFamily, LayoutId,
        LirCallableId, PlanError, PlanView, ScalarType, StaticActivationKind, StaticCleanupFact,
    },
    planning::PlannedProgram,
};

pub(super) fn lower<'plan>(
    planned: &'plan PlannedProgram<'_>,
    owner: CallableBinding<'plan>,
    coordinator: Coordinator,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = planned.plan().view();
    plan.require_same_context(owner.context())?;
    let mut builder = DraftBuilder::new(owner)?;
    if builder.inputs().next().is_some() {
        return Err(PlanError::InvalidSignature.into());
    }
    let entry = reserve_block(&mut builder)?;
    builder.set_entry(entry)?;
    let complete = match coordinator {
        Coordinator::Initializer => lower_initializer(plan, &mut builder, entry)?,
        Coordinator::Finalizer => lower_finalizer(plan, &mut builder, entry)?,
    };
    builder.terminate(complete, Terminator::Return(vec![]))?;
    verify_callable(builder.finish()).map_err(LowerError::Verification)
}

fn lower_initializer<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
    for activation in &plan.resources().activation {
        // Even zero-default activation names its checked storage. This makes
        // the coordinator receipt prove the exact activation domain while the
        // data initializer remains the sole owner of the zero fill.
        symbol_address(builder, block, activation.field)?;
        if let StaticActivationKind::Explicit(target) = activation.action {
            call(
                plan,
                builder,
                block,
                target,
                &[],
                CallAttribution::ProcessBoundary,
            )?;
        }
    }
    Ok(block)
}

fn lower_finalizer<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    mut block: crate::backend::lir::BlockHandle<'plan>,
) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
    for shutdown in &plan.resources().shutdown {
        let address = symbol_address(builder, block, shutdown.field)?;
        block = cleanup(plan, builder, block, shutdown.cleanup, address)?;
    }
    Ok(block)
}

fn cleanup<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    cleanup: StaticCleanupFact,
    address: ValueHandle<'plan>,
) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
    match cleanup {
        StaticCleanupFact::None => Ok(block),
        StaticCleanupFact::Class(class) => {
            let target = class_helper(plan, class, HelperFamily::ClassFinalizer)?;
            call_parameter(plan, builder, block, target, address)?;
            Ok(block)
        }
        StaticCleanupFact::Optional(optional) => {
            emit_process_boundary_optional_cleanup(plan, builder, block, optional, address)
        }
        StaticCleanupFact::Shared(_) => {
            let handle = load_address(plan, builder, block, address)?;
            let target = helper_for(
                plan,
                HelperFamily::Release,
                plan.semantic()
                    .shared_header
                    .ok_or(PlanError::InvalidLayout)?
                    .handle_layout,
            )?;
            call_parameter(plan, builder, block, target, handle)?;
            Ok(block)
        }
        StaticCleanupFact::Array(array) => {
            let handle = load_address(plan, builder, block, address)?;
            let target = helper_for(
                plan,
                HelperFamily::ArrayRelease,
                plan.semantic()
                    .array(array)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .descriptor_layout,
            )?;
            call_parameter(plan, builder, block, target, handle)?;
            Ok(block)
        }
    }
}

fn call_parameter<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    target: LirCallableId,
    value: ValueHandle<'plan>,
) -> Result<(), LowerError> {
    call(
        plan,
        builder,
        block,
        target,
        &[CallArgument {
            role: ComponentRole::Parameter(0),
            value,
        }],
        CallAttribution::ProcessBoundary,
    )?;
    Ok(())
}

fn call<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    target: LirCallableId,
    arguments: &[CallArgument<ValueHandle<'plan>>],
    attribution: CallAttribution,
) -> Result<Vec<ValueHandle<'plan>>, LowerError> {
    let artifact = ArtifactId::Callable(target);
    let signature = plan
        .artifact(plan.artifact_id(artifact)?, artifact.category())?
        .signature
        .ok_or(PlanError::InvalidSignature)?;
    Ok(builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(artifact),
            signature,
            arguments: arguments.to_vec(),
            attribution,
        }),
    )?)
}

fn helper_for(
    plan: PlanView<'_>,
    family: HelperFamily,
    layout: LayoutId,
) -> Result<LirCallableId, LowerError> {
    plan.resources()
        .generated
        .iter()
        .find_map(|generated| match generated.callable {
            LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                Some(generated.callable)
            }
            _ => None,
        })
        .ok_or(PlanError::UnknownDeclaration.into())
}

fn symbol_address<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    field: crate::identity::StaticFieldId,
) -> Result<ValueHandle<'plan>, LowerError> {
    Ok(builder.append(
        block,
        Operation::SymbolAddress {
            symbol: ArtifactId::Data(DataKey::Static(field)),
            ty: ScalarType::DataAddress,
        },
    )?[0])
}

fn load_address<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    address: ValueHandle<'plan>,
) -> Result<ValueHandle<'plan>, LowerError> {
    Ok(builder.append(
        block,
        Operation::Load {
            address,
            representation: address_representation(plan),
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

fn address_representation(plan: PlanView<'_>) -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::DataAddress,
        bytes: plan.profile().data_layout.pointer_bytes,
        alignment: plan.profile().data_layout.pointer_alignment,
    }
}
