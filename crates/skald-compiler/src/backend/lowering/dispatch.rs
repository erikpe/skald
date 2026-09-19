//! Checked object identity, dispatch-table access, and non-owning view binding.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{BinaryOperation, CallTarget, Constant, Edge, Operation, Terminator, ValueHandle},
        plan::{ArtifactId, DataKey, MethodSlot, ObjectViewTarget, PlanError, ScalarType},
    },
    mir::{BlockId, MirCheckedViewBinding, MirObjectView, MirViewTarget, ValueId},
    primitive_comparison::PrimitiveComparisonPredicate,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn dispatch_target(
        &mut self,
        block: BlockId,
        origin: &crate::mir::MirObjectOrigin,
        slot: MethodSlot,
        signature: crate::backend::plan::SignatureId,
    ) -> Result<CallTarget<ValueHandle<'plan>>, LowerError> {
        let origin = self.object_origin(block, origin)?;
        let offset = self
            .plan()
            .semantic()
            .method_slot(slot)
            .ok_or(PlanError::UnknownDeclaration)?
            .byte_offset;
        let address = self.byte_offset(block, origin.metadata, offset)?;
        let representation = crate::backend::lir::MemoryRepresentation {
            scalar: ScalarType::CodeAddress(signature),
            bytes: self.plan().profile().data_layout.pointer_bytes,
            alignment: self.plan().profile().data_layout.pointer_alignment,
        };
        let target = self.builder.append(
            self.blocks[block.index()],
            Operation::Load {
                address,
                representation,
            },
        )?[0];
        Ok(CallTarget::Indirect(target))
    }

    pub(super) fn type_test(
        &mut self,
        block: BlockId,
        result: ValueId,
        source: &MirObjectView,
        target: MirViewTarget,
    ) -> Result<(), LowerError> {
        let metadata = self.object_origin(block, &source.origin)?.metadata;
        let membership = self.membership(block, metadata, target)?;
        let false_value = self.builder.append(
            self.blocks[block.index()],
            Operation::Constant(Constant::Bool(false)),
        )?[0];
        self.builder.append_into(
            self.blocks[block.index()],
            Operation::Binary {
                operation: BinaryOperation::Or,
                left: membership,
                right: false_value,
            },
            &[self.values[result.index()]],
        )?;
        Ok(())
    }

    pub(super) fn bind_checked_view(
        &mut self,
        block: BlockId,
        binding: &MirCheckedViewBinding,
    ) -> Result<(), LowerError> {
        let address = self.place_address(block, &binding.view.source)?;
        let origin = self.object_origin(block, &binding.view.origin)?;
        self.store(block, binding.destination, address)?;
        self.bind_origin(binding.destination, origin)
    }

    pub(super) fn checked_cast(
        &mut self,
        block: BlockId,
        binding: &MirCheckedViewBinding,
        success_target: BlockId,
        failure_target: BlockId,
    ) -> Result<(), LowerError> {
        let address = self.place_address(block, &binding.view.source)?;
        let origin = self.object_origin(block, &binding.view.origin)?;
        let condition = self.membership(block, origin.metadata, binding.view.target)?;
        let bind = self.builder.reserve_block()?;
        self.builder.define_block(bind, &[])?;

        let object = self.objects[binding.destination.index()].ok_or(PlanError::InvalidDomain)?;
        let carrier = self
            .builder
            .append(bind, Operation::ObjectAddress(object))?[0];
        self.builder.append(
            bind,
            Operation::Store {
                address: carrier,
                value: address,
                representation: self.address_representation(),
            },
        )?;
        self.builder
            .terminate(bind, Terminator::Jump(self.edge(success_target)))?;
        self.bind_origin(binding.destination, origin)?;
        self.builder.terminate(
            self.blocks[block.index()],
            Terminator::Branch {
                condition,
                true_edge: Edge {
                    target: bind,
                    arguments: vec![],
                },
                false_edge: self.edge(failure_target),
            },
        )?;
        Ok(())
    }

    pub(super) fn membership(
        &mut self,
        block: BlockId,
        metadata: ValueHandle<'plan>,
        target: MirViewTarget,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let members = self
            .plan()
            .semantic()
            .object_view(object_view_target(target))
            .ok_or(PlanError::UnknownDeclaration)?
            .members
            .clone();
        let mut matches = Vec::with_capacity(members.len());
        for class in members {
            let plan = self.plan();
            let symbols = std::iter::once(DataKey::ClassDispatch(class))
                .chain(
                    plan.semantic()
                        .optional_boxes
                        .iter()
                        .filter(move |fact| fact.exact_dynamic_class == Some(class))
                        .map(|fact| DataKey::OptionalBoxDescriptor(fact.optional_box))
                        .filter(|key| plan.artifact_id(ArtifactId::Data(*key)).is_ok()),
                )
                .collect::<Vec<_>>();
            for symbol in symbols {
                let expected = self.builder.append(
                    self.blocks[block.index()],
                    Operation::SymbolAddress {
                        symbol: ArtifactId::Data(symbol),
                        ty: ScalarType::DataAddress,
                    },
                )?[0];
                matches.push(
                    self.builder.append(
                        self.blocks[block.index()],
                        Operation::Compare {
                            predicate: PrimitiveComparisonPredicate::Equal,
                            left: metadata,
                            right: expected,
                        },
                    )?[0],
                );
            }
        }
        let Some(mut result) = matches.pop() else {
            return Ok(self.builder.append(
                self.blocks[block.index()],
                Operation::Constant(Constant::Bool(false)),
            )?[0]);
        };
        while let Some(value) = matches.pop() {
            result = self.builder.append(
                self.blocks[block.index()],
                Operation::Binary {
                    operation: BinaryOperation::Or,
                    left: result,
                    right: value,
                },
            )?[0];
        }
        Ok(result)
    }
}

fn object_view_target(target: MirViewTarget) -> ObjectViewTarget {
    match target {
        MirViewTarget::Class(class) => ObjectViewTarget::Class(class),
        MirViewTarget::Interface(interface) => ObjectViewTarget::Interface(interface),
        MirViewTarget::Obj => ObjectViewTarget::Obj,
    }
}
