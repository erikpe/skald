//! Class copy and cleanup expansion through ordinary LIR loads, stores, and calls.

use super::{context::Lowerer, generated::class_helper, LowerError};
use crate::{
    backend::{
        lir::{Call, CallArgument, CallTarget, Operation},
        plan::{
            ArtifactId, ComponentRole, DataKey, HelperFamily, LirCallableId, PlanError, ScalarType,
        },
    },
    identity::{CallableId, ClassId, CopyAssignmentId, CopyConstructorId},
    mir::{
        BlockId, MirCleanup, MirCopyAssignment, MirCopyCapability, MirCopyConstruction,
        MirEndFullExpression, MirPlace, MirSelectedCopyOperation, MirSynthesizedFieldCopy,
    },
    source::Span,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn copy_construct(
        &mut self,
        block: BlockId,
        copy: &MirCopyConstruction,
    ) -> Result<(), LowerError> {
        self.copy_construction_operation(
            block,
            copy.operation,
            copy.destination.clone(),
            copy.source.clone(),
            copy.span,
        )
    }

    pub(super) fn copy_assign(
        &mut self,
        block: BlockId,
        copy: &MirCopyAssignment,
    ) -> Result<(), LowerError> {
        self.copy_assignment_operation(
            block,
            copy.operation,
            copy.destination.clone(),
            copy.source.clone(),
            copy.span,
        )
    }

    pub(super) fn cleanup(
        &mut self,
        block: BlockId,
        cleanup: &MirCleanup,
    ) -> Result<(), LowerError> {
        let address = self.place_address(block, &cleanup.destination)?;
        let target = class_helper(self.plan(), cleanup.target, HelperFamily::ClassFinalizer)?;
        let signature = self.callable_signature(target)?;
        let attribution = self.attribution(block, cleanup.span, false)?;
        self.builder.append(
            self.blocks[block.index()],
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments: vec![CallArgument {
                    role: ComponentRole::Parameter(0),
                    value: address,
                }],
                attribution,
            }),
        )?;
        Ok(())
    }

    pub(super) fn end_full_expression(
        &mut self,
        block: BlockId,
        end: &MirEndFullExpression,
    ) -> Result<(), LowerError> {
        for cleanup in &end.temporaries {
            self.cleanup(block, cleanup)?;
        }
        Ok(())
    }

    fn copy_construction_operation(
        &mut self,
        block: BlockId,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
        destination: MirPlace,
        source: MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        let capability = self
            .admitted
            .program()
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?
            .copy_constructor
            .clone();
        match (operation, capability) {
            (MirSelectedCopyOperation::User(id), MirCopyCapability::User(copy))
                if copy.operation == id =>
            {
                if let Some(base) = copy.base {
                    self.copy_construction_operation(
                        block,
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                        span,
                    )?;
                }
                self.call_copy_body(block, id.into(), class, &destination, &source, span)
            }
            (
                MirSelectedCopyOperation::Synthesized(selected),
                MirCopyCapability::Synthesized(copy),
            ) if copy.class == selected => {
                if let Some(base) = copy.base {
                    self.copy_construction_operation(
                        block,
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                        span,
                    )?;
                }
                for field in copy.fields {
                    self.copy_construct_field(block, &destination, &source, field, span)?;
                }
                Ok(())
            }
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn copy_assignment_operation(
        &mut self,
        block: BlockId,
        operation: MirSelectedCopyOperation<CopyAssignmentId>,
        destination: MirPlace,
        source: MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        let capability = self
            .admitted
            .program()
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?
            .copy_assignment
            .clone();
        match (operation, capability) {
            (MirSelectedCopyOperation::User(id), MirCopyCapability::User(copy))
                if copy.operation == id =>
            {
                if let Some(base) = copy.base {
                    self.copy_assignment_operation(
                        block,
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                        span,
                    )?;
                }
                self.call_copy_body(block, id.into(), class, &destination, &source, span)
            }
            (
                MirSelectedCopyOperation::Synthesized(selected),
                MirCopyCapability::Synthesized(copy),
            ) if copy.class == selected => {
                if let Some(base) = copy.base {
                    self.copy_assignment_operation(
                        block,
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                        span,
                    )?;
                }
                for field in copy.fields {
                    self.copy_assign_field(block, &destination, &source, field, span)?;
                }
                Ok(())
            }
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn copy_construct_field(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        field: MirSynthesizedFieldCopy<CopyConstructorId>,
        span: Span,
    ) -> Result<(), LowerError> {
        match field {
            MirSynthesizedFieldCopy::Scalar { field } => self.copy_scalar_place(
                block,
                &destination.clone().project_field(field),
                &source.clone().project_field(field),
            ),
            MirSynthesizedFieldCopy::Class { field, operation } => self
                .copy_construction_operation(
                    block,
                    operation,
                    destination.clone().project_field(field),
                    source.clone().project_field(field),
                    span,
                ),
            MirSynthesizedFieldCopy::Shared { field } => self.shared_field_construct(
                block,
                &destination.clone().project_field(field),
                &source.clone().project_field(field),
                span,
            ),
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn copy_assign_field(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        field: MirSynthesizedFieldCopy<CopyAssignmentId>,
        span: Span,
    ) -> Result<(), LowerError> {
        match field {
            MirSynthesizedFieldCopy::Scalar { field } => self.copy_scalar_place(
                block,
                &destination.clone().project_field(field),
                &source.clone().project_field(field),
            ),
            MirSynthesizedFieldCopy::Class { field, operation } => self.copy_assignment_operation(
                block,
                operation,
                destination.clone().project_field(field),
                source.clone().project_field(field),
                span,
            ),
            MirSynthesizedFieldCopy::Shared { field } => self.shared_field_assign(
                block,
                &destination.clone().project_field(field),
                &source.clone().project_field(field),
                span,
            ),
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn copy_scalar_place(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), LowerError> {
        let representation = self.place_representation(source)?;
        if representation != self.place_representation(destination)? {
            return Err(PlanError::InvalidSignature.into());
        }
        let source = self.place_address(block, source)?;
        let value = self.builder.append(
            self.blocks[block.index()],
            Operation::Load {
                address: source,
                representation,
            },
        )?[0];
        let destination = self.place_address(block, destination)?;
        self.builder.append(
            self.blocks[block.index()],
            Operation::Store {
                address: destination,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    fn call_copy_body(
        &mut self,
        block: BlockId,
        target: CallableId,
        class: ClassId,
        destination: &MirPlace,
        source: &MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let destination = self.place_address(block, destination)?;
        let source = self.place_address(block, source)?;
        let metadata = self.class_metadata(block, class)?;
        let target = LirCallableId::Source(target);
        let signature = self.callable_signature(target)?;
        let roles = self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        let arguments = roles
            .into_iter()
            .map(|role| {
                let value = match role {
                    ComponentRole::ReceiverStatic | ComponentRole::ReceiverComplete => destination,
                    ComponentRole::ReceiverMetadata => metadata,
                    ComponentRole::AliasAddress(0) | ComponentRole::AliasComplete(0) => source,
                    ComponentRole::AliasMetadata(0) => metadata,
                    _ => return Err(PlanError::InvalidSignature.into()),
                };
                Ok(CallArgument { role, value })
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        let attribution = self.attribution(block, span, false)?;
        self.builder.append(
            self.blocks[block.index()],
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments,
                attribution,
            }),
        )?;
        Ok(())
    }

    fn class_metadata(
        &mut self,
        block: BlockId,
        class: ClassId,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            self.blocks[block.index()],
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::ClassDispatch(class)),
                ty: ScalarType::DataAddress,
            },
        )?[0])
    }
}
