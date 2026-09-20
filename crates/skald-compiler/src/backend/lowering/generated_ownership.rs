//! Generated shared-handle retain and release graphs.

use super::LowerError;
use crate::backend::{
    failure::FailureMessage,
    lir::{
        BinaryOperation, BlockHandle, Call, CallArgument, CallAttribution, CallTarget, Constant,
        DraftBuilder, Edge, MemoryRepresentation, Operation, Terminator, ValueHandle,
        VerifiedCallable,
    },
    plan::{
        ArtifactId, CallableBinding, ComponentRole, DataKey, HelperFamily, MethodSlot, PlanError,
        PlanView, RuntimeService, ScalarType,
    },
    planning::PlannedProgram,
};
use crate::primitive_comparison::PrimitiveComparisonPredicate;

pub(super) fn lower_retain<'plan>(
    planned: &'plan PlannedProgram<'_>,
    owner: CallableBinding<'plan>,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = planned.plan().view();
    plan.require_same_context(owner.context())?;
    let mut graph = OwnerGraph::new(plan, owner)?;
    let invalid = graph.block()?;
    let positive = graph.block()?;
    let finite = graph.block()?;
    let increment = graph.block()?;
    let overflow = graph.block()?;
    let complete = graph.block()?;

    let non_null = graph.not_equal(graph.entry, graph.handle, graph.null)?;
    graph.branch(graph.entry, non_null, positive, invalid)?;

    let count = graph.load_count(positive)?;
    let non_zero = graph.not_equal(positive, count, graph.zero)?;
    graph.branch(positive, non_zero, finite, invalid)?;

    let immortal = graph.equal(finite, count, graph.maximum)?;
    let dynamic = graph.block()?;
    graph.branch(finite, immortal, complete, dynamic)?;

    let exhausted = graph.equal(dynamic, count, graph.maximum_dynamic)?;
    graph.branch(dynamic, exhausted, overflow, increment)?;

    let next = graph.builder.append(
        increment,
        Operation::Binary {
            operation: BinaryOperation::Add,
            left: count,
            right: graph.one,
        },
    )?[0];
    graph.store_count(increment, next)?;
    graph.jump(increment, complete)?;

    graph.builder.terminate(invalid, Terminator::HardTrap)?;
    graph.report_overflow(overflow)?;
    graph
        .builder
        .terminate(complete, Terminator::Return(vec![]))?;
    graph.finish()
}

pub(super) fn lower_release<'plan>(
    planned: &'plan PlannedProgram<'_>,
    owner: CallableBinding<'plan>,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = planned.plan().view();
    plan.require_same_context(owner.context())?;
    let mut graph = OwnerGraph::new(plan, owner)?;
    let invalid = graph.block()?;
    let positive = graph.block()?;
    let dynamic = graph.block()?;
    let decrement = graph.block()?;
    let last = graph.block()?;
    let metadata_live = graph.block()?;
    let complete = graph.block()?;

    let non_null = graph.not_equal(graph.entry, graph.handle, graph.null)?;
    graph.branch(graph.entry, non_null, positive, invalid)?;

    let count = graph.load_count(positive)?;
    let non_zero = graph.not_equal(positive, count, graph.zero)?;
    graph.branch(positive, non_zero, dynamic, invalid)?;

    let immortal = graph.equal(dynamic, count, graph.maximum)?;
    let owned = graph.block()?;
    graph.branch(dynamic, immortal, complete, owned)?;

    let is_last = graph.equal(owned, count, graph.one)?;
    graph.branch(owned, is_last, last, decrement)?;

    let next = graph.builder.append(
        decrement,
        Operation::Binary {
            operation: BinaryOperation::Subtract,
            left: count,
            right: graph.one,
        },
    )?[0];
    graph.store_count(decrement, next)?;
    graph.jump(decrement, complete)?;

    graph.store_count(last, graph.zero)?;
    let metadata = graph.load_address(last, graph.header.dynamic_metadata_offset)?;
    let has_metadata = graph.not_equal(last, metadata, graph.null)?;
    graph.branch(last, has_metadata, metadata_live, invalid)?;

    let finalizer_signature = finalizer_signature(plan)?;
    let slot = plan
        .semantic()
        .method_slot(MethodSlot::Finalizer)
        .ok_or(PlanError::UnknownDeclaration)?;
    let finalizer = graph.load_code(
        metadata_live,
        metadata,
        slot.byte_offset,
        finalizer_signature,
    )?;
    let payload = graph.offset(metadata_live, graph.handle, graph.header.header_size)?;
    graph.builder.append(
        metadata_live,
        Operation::Call(Call {
            target: CallTarget::Indirect(finalizer),
            signature: finalizer_signature,
            arguments: vec![CallArgument {
                role: ComponentRole::Parameter(0),
                value: payload,
            }],
            attribution: graph.inherited(),
        }),
    )?;
    graph.call_runtime(
        metadata_live,
        RuntimeService::Free,
        vec![CallArgument {
            role: ComponentRole::RuntimeParameter(0),
            value: graph.handle,
        }],
        CallAttribution::HardDefectOnly,
    )?;
    graph.jump(metadata_live, complete)?;

    graph.builder.terminate(invalid, Terminator::HardTrap)?;
    graph
        .builder
        .terminate(complete, Terminator::Return(vec![]))?;
    graph.finish()
}

struct OwnerGraph<'plan> {
    plan: PlanView<'plan>,
    owner: CallableBinding<'plan>,
    builder: DraftBuilder<'plan>,
    entry: BlockHandle<'plan>,
    handle: ValueHandle<'plan>,
    null: ValueHandle<'plan>,
    zero: ValueHandle<'plan>,
    one: ValueHandle<'plan>,
    maximum_dynamic: ValueHandle<'plan>,
    maximum: ValueHandle<'plan>,
    header: crate::backend::plan::SharedHeaderLayout,
}

impl<'plan> OwnerGraph<'plan> {
    fn new(plan: PlanView<'plan>, owner: CallableBinding<'plan>) -> Result<Self, LowerError> {
        let header = plan
            .semantic()
            .shared_header
            .ok_or(PlanError::InvalidLayout)?;
        let mut builder = DraftBuilder::new(owner)?;
        let entry = builder.reserve_block()?;
        builder.define_block(entry, &[])?;
        builder.set_entry(entry)?;
        let inputs = builder.inputs().collect::<Vec<_>>();
        let [handle] = inputs.as_slice() else {
            return Err(PlanError::InvalidSignature.into());
        };
        let handle = *handle;
        let null = builder.append(
            entry,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let zero = builder.append(entry, Operation::Constant(Constant::U64(0)))?[0];
        let one = builder.append(entry, Operation::Constant(Constant::U64(1)))?[0];
        let maximum_dynamic =
            builder.append(entry, Operation::Constant(Constant::U64(u64::MAX - 1)))?[0];
        let maximum = builder.append(entry, Operation::Constant(Constant::U64(u64::MAX)))?[0];
        Ok(Self {
            plan,
            owner,
            builder,
            entry,
            handle,
            null,
            zero,
            one,
            maximum_dynamic,
            maximum,
            header,
        })
    }

    fn block(&mut self) -> Result<BlockHandle<'plan>, LowerError> {
        let block = self.builder.reserve_block()?;
        self.builder.define_block(block, &[])?;
        Ok(block)
    }

    fn equal(
        &mut self,
        block: BlockHandle<'plan>,
        left: ValueHandle<'plan>,
        right: ValueHandle<'plan>,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::Equal,
                left,
                right,
            },
        )?[0])
    }

    fn not_equal(
        &mut self,
        block: BlockHandle<'plan>,
        left: ValueHandle<'plan>,
        right: ValueHandle<'plan>,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left,
                right,
            },
        )?[0])
    }

    fn branch(
        &mut self,
        block: BlockHandle<'plan>,
        condition: ValueHandle<'plan>,
        yes: BlockHandle<'plan>,
        no: BlockHandle<'plan>,
    ) -> Result<(), LowerError> {
        self.builder.terminate(
            block,
            Terminator::Branch {
                condition,
                true_edge: edge(yes),
                false_edge: edge(no),
            },
        )?;
        Ok(())
    }

    fn jump(
        &mut self,
        block: BlockHandle<'plan>,
        target: BlockHandle<'plan>,
    ) -> Result<(), LowerError> {
        self.builder
            .terminate(block, Terminator::Jump(edge(target)))?;
        Ok(())
    }

    fn offset(
        &mut self,
        block: BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        bytes: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        if bytes == 0 {
            return Ok(base);
        }
        let offset = self.builder.append(
            block,
            Operation::Constant(Constant::U64(
                u64::try_from(bytes).map_err(|_| PlanError::SizeOverflow)?,
            )),
        )?[0];
        Ok(self
            .builder
            .append(block, Operation::ByteOffset { base, offset })?[0])
    }

    fn load_count(&mut self, block: BlockHandle<'plan>) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.offset(block, self.handle, self.header.owner_count_offset)?;
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation: count_representation(),
            },
        )?[0])
    }

    fn store_count(
        &mut self,
        block: BlockHandle<'plan>,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.offset(block, self.handle, self.header.owner_count_offset)?;
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation: count_representation(),
            },
        )?;
        Ok(())
    }

    fn load_address(
        &mut self,
        block: BlockHandle<'plan>,
        offset: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.offset(block, self.handle, offset)?;
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation: address_representation(self.plan),
            },
        )?[0])
    }

    fn load_code(
        &mut self,
        block: BlockHandle<'plan>,
        metadata: ValueHandle<'plan>,
        offset: usize,
        signature: crate::backend::plan::SignatureId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.offset(block, metadata, offset)?;
        let data = self.plan.profile().data_layout;
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation: MemoryRepresentation {
                    scalar: ScalarType::CodeAddress(signature),
                    bytes: data.pointer_bytes,
                    alignment: data.pointer_alignment,
                },
            },
        )?[0])
    }

    fn report_overflow(&mut self, block: BlockHandle<'plan>) -> Result<(), LowerError> {
        let reason = FailureMessage::OwnershipCountOverflow;
        let message = self.builder.append(
            block,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::FailureMessage(reason)),
                ty: ScalarType::DataAddress,
            },
        )?[0];
        let length = self.builder.append(
            block,
            Operation::Constant(Constant::U64(reason.bytes().len() as u64)),
        )?[0];
        let call = self.runtime(
            RuntimeService::Panic,
            vec![
                CallArgument {
                    role: ComponentRole::RuntimeParameter(0),
                    value: message,
                },
                CallArgument {
                    role: ComponentRole::RuntimeParameter(1),
                    value: length,
                },
            ],
            self.inherited(),
        )?;
        self.builder
            .terminate(block, Terminator::ReportFailure { call, reason })?;
        Ok(())
    }

    fn call_runtime(
        &mut self,
        block: BlockHandle<'plan>,
        service: RuntimeService,
        arguments: Vec<CallArgument<ValueHandle<'plan>>>,
        attribution: CallAttribution,
    ) -> Result<(), LowerError> {
        let call = self.runtime(service, arguments, attribution)?;
        self.builder.append(block, Operation::Call(call))?;
        Ok(())
    }

    fn runtime(
        &self,
        service: RuntimeService,
        arguments: Vec<CallArgument<ValueHandle<'plan>>>,
        attribution: CallAttribution,
    ) -> Result<Call<ValueHandle<'plan>>, LowerError> {
        let target = ArtifactId::Runtime(service);
        let signature = self
            .plan
            .artifact(self.plan.artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        Ok(Call {
            target: CallTarget::Direct(target),
            signature,
            arguments,
            attribution,
        })
    }

    fn inherited(&self) -> CallAttribution {
        CallAttribution::InheritedOperation {
            boundary: self.owner.key(),
        }
    }

    fn finish(self) -> Result<VerifiedCallable<'plan>, LowerError> {
        crate::backend::lir::verify_callable(self.builder.finish())
            .map_err(LowerError::Verification)
    }
}

fn finalizer_signature(
    plan: PlanView<'_>,
) -> Result<crate::backend::plan::SignatureId, LowerError> {
    let helper = plan
        .resources()
        .generated
        .iter()
        .find_map(|fact| match fact.callable {
            crate::backend::plan::LirCallableId::Helper(key)
                if matches!(
                    key.family,
                    HelperFamily::ClassFinalizer | HelperFamily::ArraySharedFinalizer
                ) =>
            {
                Some(fact.callable)
            }
            _ => None,
        })
        .ok_or(PlanError::UnknownDeclaration)?;
    Ok(plan.callable(helper)?.signature_id())
}

fn edge<'plan>(target: BlockHandle<'plan>) -> Edge<ValueHandle<'plan>, BlockHandle<'plan>> {
    Edge {
        target,
        arguments: vec![],
    }
}

fn count_representation() -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    }
}

fn address_representation(plan: PlanView<'_>) -> MemoryRepresentation {
    let data = plan.profile().data_layout;
    MemoryRepresentation {
        scalar: ScalarType::DataAddress,
        bytes: data.pointer_bytes,
        alignment: data.pointer_alignment,
    }
}
