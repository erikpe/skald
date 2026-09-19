//! Generated class lifecycle bodies built from frozen semantic facts.

use super::LowerError;
use crate::{
    backend::{
        lir::{
            Call, CallArgument, CallAttribution, CallTarget, Constant, DraftBuilder, Operation,
            Terminator, ValueHandle, VerifiedCallable,
        },
        plan::{
            ArtifactId, CallableBinding, ComponentRole, DataKey, DestructionStepFact, HelperFamily,
            LayoutId, LirCallableId, PlanError, PlanView, ScalarType,
        },
        planning::AdmittedProgram,
    },
    identity::ClassId,
};

/// Emit one complete-class finalizer. Nested class fields and bases call their
/// own generated finalizers, so worklist reservation closes recursive helper
/// dependencies without recursively constructing Rust bodies.
pub(super) fn lower_class_finalizer<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    layout: LayoutId,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = admitted.plan().view();
    plan.require_same_context(owner.context())?;
    let classes = plan
        .semantic()
        .classes
        .iter()
        .filter(|class| class.complete_layout == layout)
        .collect::<Vec<_>>();
    let [class] = classes.as_slice() else {
        return Err(PlanError::InvalidDomain.into());
    };

    let mut builder = DraftBuilder::new(owner)?;
    let entry = builder.reserve_block()?;
    builder.define_block(entry, &[])?;
    builder.set_entry(entry)?;
    let inputs = builder.inputs().collect::<Vec<_>>();
    let [complete] = inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let complete = *complete;
    let mut current = entry;

    for step in class.destruction.clone() {
        match step {
            DestructionStepFact::UserBody(destructor) => {
                let metadata = symbol_address(
                    &mut builder,
                    current,
                    ArtifactId::Data(DataKey::ClassDispatch(class.class)),
                )?;
                call_source_destructor(
                    plan,
                    &mut builder,
                    current,
                    owner.key(),
                    LirCallableId::Source(destructor.into()),
                    complete,
                    metadata,
                )?;
            }
            DestructionStepFact::Field(field) => {
                let field = plan
                    .semantic()
                    .field(field)
                    .ok_or(PlanError::UnknownDeclaration)?;
                let crate::backend::plan::SemanticType::Class(target) = field.ty else {
                    return Err(PlanError::InvalidDomain.into());
                };
                let address = byte_offset(&mut builder, current, complete, field.offset)?;
                call_finalizer(plan, &mut builder, current, owner.key(), target, address)?;
            }
            DestructionStepFact::Base(target) => {
                let base = class.base.ok_or(PlanError::InvalidDomain)?;
                if base.class != target {
                    return Err(PlanError::InvalidDomain.into());
                }
                let address = byte_offset(&mut builder, current, complete, base.offset)?;
                call_finalizer(plan, &mut builder, current, owner.key(), target, address)?;
            }
            DestructionStepFact::SharedField(field) => {
                let field = plan
                    .semantic()
                    .field(field)
                    .ok_or(PlanError::UnknownDeclaration)?;
                if !matches!(field.ty, crate::backend::plan::SemanticType::Shared(_)) {
                    return Err(PlanError::InvalidDomain.into());
                }
                let address = byte_offset(&mut builder, current, complete, field.offset)?;
                let data = plan.profile().data_layout;
                let handle = builder.append(
                    current,
                    Operation::Load {
                        address,
                        representation: crate::backend::lir::MemoryRepresentation {
                            scalar: ScalarType::DataAddress,
                            bytes: data.pointer_bytes,
                            alignment: data.pointer_alignment,
                        },
                    },
                )?[0];
                call_owner_helper(
                    plan,
                    &mut builder,
                    current,
                    owner.key(),
                    HelperFamily::Release,
                    handle,
                )?;
            }
            DestructionStepFact::OptionalSharedField(field) => {
                let field = plan
                    .semantic()
                    .field(field)
                    .ok_or(PlanError::UnknownDeclaration)?;
                if !matches!(
                    field.ty,
                    crate::backend::plan::SemanticType::Optional(optional)
                        if plan
                            .semantic()
                            .optional(optional)
                            .is_some_and(|fact| fact.nullable_niche)
                ) {
                    return Err(PlanError::InvalidDomain.into());
                }
                let address = byte_offset(&mut builder, current, complete, field.offset)?;
                let data = plan.profile().data_layout;
                let handle = builder.append(
                    current,
                    Operation::Load {
                        address,
                        representation: crate::backend::lir::MemoryRepresentation {
                            scalar: ScalarType::DataAddress,
                            bytes: data.pointer_bytes,
                            alignment: data.pointer_alignment,
                        },
                    },
                )?[0];
                let null = builder.append(
                    current,
                    Operation::Constant(Constant::Null(ScalarType::DataAddress)),
                )?[0];
                let present = builder.append(
                    current,
                    Operation::Compare {
                        predicate:
                            crate::primitive_comparison::PrimitiveComparisonPredicate::NotEqual,
                        left: handle,
                        right: null,
                    },
                )?[0];
                let release = builder.reserve_block()?;
                let next = builder.reserve_block()?;
                builder.define_block(release, &[])?;
                builder.define_block(next, &[])?;
                builder.terminate(
                    current,
                    Terminator::Branch {
                        condition: present,
                        true_edge: edge(release),
                        false_edge: edge(next),
                    },
                )?;
                call_owner_helper(
                    plan,
                    &mut builder,
                    release,
                    owner.key(),
                    HelperFamily::Release,
                    handle,
                )?;
                builder.terminate(release, Terminator::Jump(edge(next)))?;
                current = next;
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        }
    }
    builder.terminate(current, Terminator::Return(vec![]))?;
    crate::backend::lir::verify_callable(builder.finish()).map_err(LowerError::Verification)
}

fn edge<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
) -> crate::backend::lir::Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    crate::backend::lir::Edge {
        target,
        arguments: vec![],
    }
}

fn call_owner_helper<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    boundary: LirCallableId,
    family: HelperFamily,
    handle: ValueHandle<'plan>,
) -> Result<(), LowerError> {
    let target = super::ownership::owner_helper(plan, family)?;
    let signature = callable_signature(plan, target)?;
    builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(ArtifactId::Callable(target)),
            signature,
            arguments: vec![CallArgument {
                role: ComponentRole::Parameter(0),
                value: handle,
            }],
            attribution: CallAttribution::InheritedOperation { boundary },
        }),
    )?;
    Ok(())
}

pub(super) fn class_helper(
    plan: PlanView<'_>,
    class: ClassId,
    family: HelperFamily,
) -> Result<LirCallableId, LowerError> {
    let layout = plan
        .semantic()
        .class(class)
        .ok_or(PlanError::UnknownDeclaration)?
        .complete_layout;
    plan.resources()
        .generated
        .iter()
        .find_map(|fact| match fact.callable {
            LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                Some(fact.callable)
            }
            _ => None,
        })
        .ok_or(PlanError::UnknownDeclaration.into())
}

fn call_source_destructor<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    boundary: LirCallableId,
    target: LirCallableId,
    complete: ValueHandle<'plan>,
    metadata: ValueHandle<'plan>,
) -> Result<(), LowerError> {
    let signature = callable_signature(plan, target)?;
    let roles = plan
        .signature(plan.signature_id(signature.index())?)?
        .inputs
        .iter()
        .map(|component| component.role)
        .collect::<Vec<_>>();
    let arguments = roles
        .into_iter()
        .map(|role| {
            let value = match role {
                ComponentRole::ReceiverStatic | ComponentRole::ReceiverComplete => complete,
                ComponentRole::ReceiverMetadata => metadata,
                _ => return Err(PlanError::InvalidSignature.into()),
            };
            Ok(CallArgument { role, value })
        })
        .collect::<Result<Vec<_>, LowerError>>()?;
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

fn call_finalizer<'plan>(
    plan: PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    boundary: LirCallableId,
    class: ClassId,
    address: ValueHandle<'plan>,
) -> Result<(), LowerError> {
    let target = class_helper(plan, class, HelperFamily::ClassFinalizer)?;
    let signature = callable_signature(plan, target)?;
    builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(ArtifactId::Callable(target)),
            signature,
            arguments: vec![CallArgument {
                role: ComponentRole::Parameter(0),
                value: address,
            }],
            attribution: CallAttribution::InheritedOperation { boundary },
        }),
    )?;
    Ok(())
}

fn callable_signature(
    plan: PlanView<'_>,
    target: LirCallableId,
) -> Result<crate::backend::plan::SignatureId, LowerError> {
    let artifact = ArtifactId::Callable(target);
    Ok(plan
        .artifact(plan.artifact_id(artifact)?, artifact.category())?
        .signature
        .ok_or(PlanError::InvalidSignature)?)
}

fn symbol_address<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    symbol: ArtifactId,
) -> Result<ValueHandle<'plan>, LowerError> {
    Ok(builder.append(
        block,
        Operation::SymbolAddress {
            symbol,
            ty: ScalarType::DataAddress,
        },
    )?[0])
}

fn byte_offset<'plan>(
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    base: ValueHandle<'plan>,
    bytes: usize,
) -> Result<ValueHandle<'plan>, LowerError> {
    if bytes == 0 {
        return Ok(base);
    }
    let offset = builder.append(
        block,
        Operation::Constant(Constant::U64(
            u64::try_from(bytes).map_err(|_| PlanError::SizeOverflow)?,
        )),
    )?[0];
    Ok(builder.append(block, Operation::ByteOffset { base, offset })?[0])
}
