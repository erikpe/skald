//! Tagged aggregate and inline-class optional lifecycle lowering.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Constant, MemoryRepresentation, Operation, Terminator},
        plan::{OptionalStorageFact, PlanError, ScalarType},
    },
    identity::{ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
    mir::{
        BlockId, MirAggregateOptionalAssign, MirAggregateOptionalCleanup,
        MirAggregateOptionalInitialize, MirAggregateOptionalPublish, MirAggregateOptionalSource,
        MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
        MirClassOptionalPublish, MirClassOptionalSource, MirCleanup, MirCopyCapability, MirPlace,
        MirSelectedCopyOperation,
    },
    primitive_comparison::PrimitiveComparisonPredicate,
    source::Span,
};

struct ClassOptionalCopy<'a> {
    optional: OptionalTypeId,
    class: ClassId,
    destination: &'a MirPlace,
    source: &'a MirPlace,
    assigning: bool,
    constructor: MirSelectedCopyOperation<CopyConstructorId>,
    assignment: Option<MirSelectedCopyOperation<CopyAssignmentId>>,
    span: Span,
}

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn aggregate_optional_initialize(
        &mut self,
        block: BlockId,
        operation: &MirAggregateOptionalInitialize,
    ) -> Result<(), LowerError> {
        self.require_optional(operation.optional, &operation.destination)?;
        match &operation.source {
            MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => {
                self.store_optional_state(block, &operation.destination, operation.optional, 0)
            }
            MirAggregateOptionalSource::Copy(source) => self.copy_aggregate_optional(
                block,
                operation.optional,
                &operation.destination,
                source,
                false,
                operation.span,
            ),
        }
    }

    pub(super) fn aggregate_optional_assign(
        &mut self,
        block: BlockId,
        operation: &MirAggregateOptionalAssign,
    ) -> Result<(), LowerError> {
        self.require_optional(operation.optional, &operation.destination)?;
        if matches!(&operation.source, MirAggregateOptionalSource::Copy(source) if source == &operation.destination)
        {
            return Ok(());
        }
        self.trap_if_optional_pinned(block, &operation.destination, operation.optional)?;
        match &operation.source {
            MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => self
                .cleanup_aggregate_optional(
                    block,
                    operation.optional,
                    &operation.destination,
                    operation.span,
                ),
            MirAggregateOptionalSource::Copy(source) => self.copy_aggregate_optional(
                block,
                operation.optional,
                &operation.destination,
                source,
                true,
                operation.span,
            ),
        }
    }

    pub(super) fn aggregate_optional_publish(
        &mut self,
        block: BlockId,
        operation: &MirAggregateOptionalPublish,
    ) -> Result<(), LowerError> {
        self.require_optional(operation.optional, &operation.destination)?;
        self.store_optional_state(block, &operation.destination, operation.optional, 1)
    }

    pub(super) fn aggregate_optional_cleanup(
        &mut self,
        block: BlockId,
        operation: &MirAggregateOptionalCleanup,
    ) -> Result<(), LowerError> {
        self.require_optional(operation.optional, &operation.destination)?;
        self.trap_if_optional_pinned(block, &operation.destination, operation.optional)?;
        self.cleanup_aggregate_optional(
            block,
            operation.optional,
            &operation.destination,
            operation.span,
        )
    }

    pub(super) fn class_optional_initialize(
        &mut self,
        block: BlockId,
        operation: &MirClassOptionalInitialize,
    ) -> Result<(), LowerError> {
        self.require_class_optional(operation.optional, operation.class, &operation.destination)?;
        match &operation.source {
            MirClassOptionalSource::Absent => {
                self.store_optional_state(block, &operation.destination, operation.optional, 0)
            }
            MirClassOptionalSource::Present(source) => {
                self.copy_construction_operation(
                    block,
                    operation.copy_constructor.ok_or(PlanError::InvalidDomain)?,
                    operation
                        .destination
                        .clone()
                        .project_optional_payload(operation.class),
                    source.clone(),
                    operation.span,
                )?;
                self.store_optional_state(block, &operation.destination, operation.optional, 1)
            }
            MirClassOptionalSource::Copy(source) => self.copy_class_optional(
                block,
                ClassOptionalCopy {
                    optional: operation.optional,
                    class: operation.class,
                    destination: &operation.destination,
                    source,
                    assigning: false,
                    constructor: operation.copy_constructor.ok_or(PlanError::InvalidDomain)?,
                    assignment: None,
                    span: operation.span,
                },
            ),
        }
    }

    pub(super) fn class_optional_assign(
        &mut self,
        block: BlockId,
        operation: &MirClassOptionalAssign,
    ) -> Result<(), LowerError> {
        self.require_class_optional(operation.optional, operation.class, &operation.destination)?;
        if matches!(&operation.source, MirClassOptionalSource::Copy(source) if source == &operation.destination)
        {
            return Ok(());
        }
        self.trap_if_optional_pinned(block, &operation.destination, operation.optional)?;
        match &operation.source {
            MirClassOptionalSource::Absent => self.cleanup_class_optional(
                block,
                operation.optional,
                operation.class,
                &operation.destination,
                operation.span,
            ),
            MirClassOptionalSource::Present(source) => {
                self.assign_present_class_optional(block, operation, source.clone())
            }
            MirClassOptionalSource::Copy(source) => self.copy_class_optional(
                block,
                ClassOptionalCopy {
                    optional: operation.optional,
                    class: operation.class,
                    destination: &operation.destination,
                    source,
                    assigning: true,
                    constructor: operation.copy_constructor.ok_or(PlanError::InvalidDomain)?,
                    assignment: Some(operation.copy_assignment.ok_or(PlanError::InvalidDomain)?),
                    span: operation.span,
                },
            ),
        }
    }

    pub(super) fn class_optional_publish(
        &mut self,
        block: BlockId,
        operation: &MirClassOptionalPublish,
    ) -> Result<(), LowerError> {
        self.require_class_optional(operation.optional, operation.class, &operation.destination)?;
        self.store_optional_state(block, &operation.destination, operation.optional, 1)
    }

    pub(super) fn class_optional_cleanup(
        &mut self,
        block: BlockId,
        operation: &MirClassOptionalCleanup,
    ) -> Result<(), LowerError> {
        self.require_class_optional(operation.optional, operation.class, &operation.destination)?;
        self.trap_if_optional_pinned(block, &operation.destination, operation.optional)?;
        self.cleanup_class_optional(
            block,
            operation.optional,
            operation.class,
            &operation.destination,
            operation.span,
        )
    }

    fn copy_aggregate_optional(
        &mut self,
        block: BlockId,
        optional: OptionalTypeId,
        destination: &MirPlace,
        source: &MirPlace,
        assigning: bool,
        span: Span,
    ) -> Result<(), LowerError> {
        self.require_optional(optional, source)?;
        let source_state = self.load_optional_state(block, source, optional)?;
        let source_present = self.is_nonzero(block, source_state)?;
        let present = self.new_optional_block()?;
        let absent = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        self.branch_active(block, source_present, present, absent)?;

        self.active_blocks[block.index()] = absent;
        if assigning {
            self.cleanup_aggregate_optional(block, optional, destination, span)?;
        } else {
            self.store_optional_state(block, destination, optional, 0)?;
        }
        let absent = self.active_blocks[block.index()];
        self.jump_optional(absent, complete)?;

        self.active_blocks[block.index()] = present;
        let destination_present = if assigning {
            let state = self.load_optional_state(block, destination, optional)?;
            Some(self.is_nonzero(block, state)?)
        } else {
            None
        };
        let destination_payload = destination
            .clone()
            .project_aggregate_optional_payload(optional);
        let source_payload = source.clone().project_aggregate_optional_payload(optional);
        if let Some(destination_present) = destination_present {
            let assign = self.new_optional_block()?;
            let construct = self.new_optional_block()?;
            let published = self.new_optional_block()?;
            self.branch_active(block, destination_present, assign, construct)?;
            self.active_blocks[block.index()] = assign;
            self.copy_optional_payload(
                block,
                optional,
                destination_payload.clone(),
                source_payload.clone(),
                true,
                span,
            )?;
            let assign = self.active_blocks[block.index()];
            self.jump_optional(assign, published)?;
            self.active_blocks[block.index()] = construct;
            self.copy_optional_payload(
                block,
                optional,
                destination_payload,
                source_payload,
                false,
                span,
            )?;
            self.store_optional_state(block, destination, optional, 1)?;
            let construct = self.active_blocks[block.index()];
            self.jump_optional(construct, published)?;
            self.active_blocks[block.index()] = published;
        } else {
            self.copy_optional_payload(
                block,
                optional,
                destination_payload,
                source_payload,
                false,
                span,
            )?;
            self.store_optional_state(block, destination, optional, 1)?;
        }
        let present = self.active_blocks[block.index()];
        self.jump_optional(present, complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn copy_optional_payload(
        &mut self,
        block: BlockId,
        outer: OptionalTypeId,
        destination: MirPlace,
        source: MirPlace,
        assigning: bool,
        span: Span,
    ) -> Result<(), LowerError> {
        let fact = self
            .plan()
            .semantic()
            .optional(outer)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        match fact.storage {
            OptionalStorageFact::InlineArray(array) => {
                self.copy_inline_array_payload(block, destination, source, array, assigning, span)
            }
            OptionalStorageFact::Nested(inner) => {
                let inner_fact = self
                    .plan()
                    .semantic()
                    .optional(inner)
                    .cloned()
                    .ok_or(PlanError::UnknownDeclaration)?;
                match inner_fact.storage {
                    OptionalStorageFact::Scalar => {
                        if assigning {
                            self.optional_assign(
                                block,
                                &crate::mir::MirOptionalAssign {
                                    destination,
                                    source: crate::mir::MirOptionalSource::Copy(source),
                                    authorization: None,
                                    final_authorization: None,
                                    span,
                                },
                            )
                        } else {
                            self.optional_initialize(
                                block,
                                &crate::mir::MirOptionalInitialize {
                                    destination,
                                    source: crate::mir::MirOptionalSource::Copy(source),
                                    span,
                                },
                            )
                        }
                    }
                    OptionalStorageFact::SharedOwner(target) => {
                        let target = mir_shared_target(target);
                        if assigning {
                            self.optional_shared_assign(
                                block,
                                &crate::mir::MirOptionalSharedAssign {
                                    optional: inner,
                                    destination,
                                    source: crate::mir::MirOptionalSharedSource::Copy(source),
                                    target,
                                    authorization: None,
                                    final_authorization: None,
                                    span,
                                },
                            )
                        } else {
                            self.optional_shared_initialize(
                                block,
                                &crate::mir::MirOptionalSharedInitialize {
                                    optional: inner,
                                    destination,
                                    source: crate::mir::MirOptionalSharedSource::Copy(source),
                                    target,
                                    span,
                                },
                            )
                        }
                    }
                    OptionalStorageFact::InlineClass(class) => {
                        let (constructor, assignment) = self.selected_class_copy(class)?;
                        if assigning {
                            self.class_optional_assign(
                                block,
                                &MirClassOptionalAssign {
                                    optional: inner,
                                    destination,
                                    source: MirClassOptionalSource::Copy(source),
                                    class,
                                    copy_constructor: Some(constructor),
                                    copy_assignment: Some(assignment),
                                    authorization: None,
                                    final_authorization: None,
                                    span,
                                },
                            )
                        } else {
                            self.class_optional_initialize(
                                block,
                                &MirClassOptionalInitialize {
                                    optional: inner,
                                    destination,
                                    source: MirClassOptionalSource::Copy(source),
                                    class,
                                    copy_constructor: Some(constructor),
                                    span,
                                },
                            )
                        }
                    }
                    OptionalStorageFact::Nested(_) => {
                        if assigning {
                            self.aggregate_optional_assign(
                                block,
                                &MirAggregateOptionalAssign {
                                    optional: inner,
                                    destination,
                                    source: MirAggregateOptionalSource::Copy(source),
                                    authorization: None,
                                    final_authorization: None,
                                    span,
                                },
                            )
                        } else {
                            self.aggregate_optional_initialize(
                                block,
                                &MirAggregateOptionalInitialize {
                                    optional: inner,
                                    destination,
                                    source: MirAggregateOptionalSource::Copy(source),
                                    span,
                                },
                            )
                        }
                    }
                    OptionalStorageFact::InlineArray(array) => self.copy_inline_array_payload(
                        block,
                        destination,
                        source,
                        array,
                        assigning,
                        span,
                    ),
                }
            }
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn cleanup_aggregate_optional(
        &mut self,
        block: BlockId,
        optional: OptionalTypeId,
        destination: &MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, destination, optional)?;
        let present = self.is_nonzero(block, state)?;
        let cleanup = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        self.branch_active(block, present, cleanup, complete)?;
        self.active_blocks[block.index()] = cleanup;
        let payload = destination
            .clone()
            .project_aggregate_optional_payload(optional);
        let fact = self
            .plan()
            .semantic()
            .optional(optional)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        match fact.storage {
            OptionalStorageFact::Nested(inner) => {
                self.cleanup_optional_payload(block, inner, payload, span)?
            }
            OptionalStorageFact::InlineArray(array) => {
                let handle = self.load_place(block, &payload)?;
                self.release_inline_array(block, handle, array, span)?;
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        }
        self.store_optional_state(block, destination, optional, 0)?;
        let cleanup = self.active_blocks[block.index()];
        self.jump_optional(cleanup, complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn cleanup_optional_payload(
        &mut self,
        block: BlockId,
        optional: OptionalTypeId,
        destination: MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let fact = self
            .plan()
            .semantic()
            .optional(optional)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        match fact.storage {
            OptionalStorageFact::Scalar => Ok(()),
            OptionalStorageFact::SharedOwner(target) => self.optional_shared_cleanup(
                block,
                &crate::mir::MirOptionalSharedCleanup {
                    optional,
                    destination,
                    target: mir_shared_target(target),
                    span,
                },
            ),
            OptionalStorageFact::InlineClass(class) => self.class_optional_cleanup(
                block,
                &MirClassOptionalCleanup {
                    optional,
                    destination,
                    class,
                    span,
                },
            ),
            OptionalStorageFact::Nested(_) => self.aggregate_optional_cleanup(
                block,
                &MirAggregateOptionalCleanup {
                    optional,
                    destination,
                    span,
                },
            ),
            OptionalStorageFact::InlineArray(array) => {
                let handle = self.load_place(block, &destination)?;
                self.release_inline_array(block, handle, array, span)
            }
        }
    }

    fn copy_inline_array_payload(
        &mut self,
        block: BlockId,
        destination: MirPlace,
        source: MirPlace,
        array: crate::identity::ArrayTypeId,
        assigning: bool,
        span: Span,
    ) -> Result<(), LowerError> {
        let source = self.load_place(block, &source)?;
        let replacement = self.clone_inline_array(block, source, array, span)?;
        if assigning {
            let previous = self.load_place(block, &destination)?;
            self.store_place(block, &destination, replacement)?;
            self.release_inline_array(block, previous, array, span)
        } else {
            self.store_place(block, &destination, replacement)
        }
    }

    fn copy_class_optional(
        &mut self,
        block: BlockId,
        copy: ClassOptionalCopy<'_>,
    ) -> Result<(), LowerError> {
        let ClassOptionalCopy {
            optional,
            class,
            destination,
            source,
            assigning,
            constructor,
            assignment,
            span,
        } = copy;
        let state = self.load_optional_state(block, source, optional)?;
        let source_present = self.is_nonzero(block, state)?;
        let present = self.new_optional_block()?;
        let absent = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        self.branch_active(block, source_present, present, absent)?;
        self.active_blocks[block.index()] = absent;
        if assigning {
            self.cleanup_class_optional(block, optional, class, destination, span)?;
        } else {
            self.store_optional_state(block, destination, optional, 0)?;
        }
        let absent = self.active_blocks[block.index()];
        self.jump_optional(absent, complete)?;
        self.active_blocks[block.index()] = present;
        let source_payload = source.clone().project_optional_payload(class);
        if assigning {
            let state = self.load_optional_state(block, destination, optional)?;
            let destination_present = self.is_nonzero(block, state)?;
            let assign = self.new_optional_block()?;
            let construct = self.new_optional_block()?;
            let published = self.new_optional_block()?;
            self.branch_active(block, destination_present, assign, construct)?;
            self.active_blocks[block.index()] = assign;
            self.copy_assignment_operation(
                block,
                assignment.ok_or(PlanError::InvalidDomain)?,
                destination.clone().project_optional_payload(class),
                source_payload.clone(),
                span,
            )?;
            self.jump_optional(self.active_blocks[block.index()], published)?;
            self.active_blocks[block.index()] = construct;
            self.copy_construction_operation(
                block,
                constructor,
                destination.clone().project_optional_payload(class),
                source_payload,
                span,
            )?;
            self.store_optional_state(block, destination, optional, 1)?;
            self.jump_optional(self.active_blocks[block.index()], published)?;
            self.active_blocks[block.index()] = published;
        } else {
            self.copy_construction_operation(
                block,
                constructor,
                destination.clone().project_optional_payload(class),
                source_payload,
                span,
            )?;
            self.store_optional_state(block, destination, optional, 1)?;
        }
        self.jump_optional(self.active_blocks[block.index()], complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn assign_present_class_optional(
        &mut self,
        block: BlockId,
        operation: &MirClassOptionalAssign,
        source: MirPlace,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, &operation.destination, operation.optional)?;
        let present = self.is_nonzero(block, state)?;
        let assign = self.new_optional_block()?;
        let construct = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        self.branch_active(block, present, assign, construct)?;
        self.active_blocks[block.index()] = assign;
        self.copy_assignment_operation(
            block,
            operation.copy_assignment.ok_or(PlanError::InvalidDomain)?,
            operation
                .destination
                .clone()
                .project_optional_payload(operation.class),
            source.clone(),
            operation.span,
        )?;
        self.jump_optional(self.active_blocks[block.index()], complete)?;
        self.active_blocks[block.index()] = construct;
        self.copy_construction_operation(
            block,
            operation.copy_constructor.ok_or(PlanError::InvalidDomain)?,
            operation
                .destination
                .clone()
                .project_optional_payload(operation.class),
            source,
            operation.span,
        )?;
        self.store_optional_state(block, &operation.destination, operation.optional, 1)?;
        self.jump_optional(self.active_blocks[block.index()], complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn cleanup_class_optional(
        &mut self,
        block: BlockId,
        optional: OptionalTypeId,
        class: ClassId,
        destination: &MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, destination, optional)?;
        let present = self.is_nonzero(block, state)?;
        let cleanup = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        self.branch_active(block, present, cleanup, complete)?;
        self.active_blocks[block.index()] = cleanup;
        self.cleanup(
            block,
            &MirCleanup {
                destination: destination.clone().project_optional_payload(class),
                target: class,
                span,
            },
        )?;
        self.store_optional_state(block, destination, optional, 0)?;
        self.jump_optional(self.active_blocks[block.index()], complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn selected_class_copy(
        &self,
        class: ClassId,
    ) -> Result<
        (
            MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
            MirSelectedCopyOperation<crate::identity::CopyAssignmentId>,
        ),
        LowerError,
    > {
        let class = self
            .planned
            .program()
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?;
        let constructor = match &class.copy_constructor {
            MirCopyCapability::User(copy) => MirSelectedCopyOperation::User(copy.operation),
            MirCopyCapability::Synthesized(copy) => {
                MirSelectedCopyOperation::Synthesized(copy.class)
            }
            MirCopyCapability::Unavailable => return Err(PlanError::InvalidDomain.into()),
        };
        let assignment = match &class.copy_assignment {
            MirCopyCapability::User(copy) => MirSelectedCopyOperation::User(copy.operation),
            MirCopyCapability::Synthesized(copy) => {
                MirSelectedCopyOperation::Synthesized(copy.class)
            }
            MirCopyCapability::Unavailable => return Err(PlanError::InvalidDomain.into()),
        };
        Ok((constructor, assignment))
    }

    fn require_optional(
        &self,
        optional: OptionalTypeId,
        place: &MirPlace,
    ) -> Result<(), LowerError> {
        if self.optional_fact(place)?.optional != optional {
            return Err(PlanError::InvalidDomain.into());
        }
        Ok(())
    }

    fn require_class_optional(
        &self,
        optional: OptionalTypeId,
        class: ClassId,
        place: &MirPlace,
    ) -> Result<(), LowerError> {
        self.require_optional(optional, place)?;
        if self
            .plan()
            .semantic()
            .optional(optional)
            .map(|fact| fact.storage)
            != Some(OptionalStorageFact::InlineClass(class))
        {
            return Err(PlanError::InvalidDomain.into());
        }
        Ok(())
    }

    pub(super) fn load_optional_state(
        &mut self,
        block: BlockId,
        place: &MirPlace,
        optional: OptionalTypeId,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        self.require_optional(optional, place)?;
        let fact = self.optional_fact(place)?;
        let address = self.optional_state_address(block, place, &fact)?;
        self.load_optional_at(block, address, state_representation())
    }

    pub(super) fn store_optional_state(
        &mut self,
        block: BlockId,
        place: &MirPlace,
        optional: OptionalTypeId,
        value: u64,
    ) -> Result<(), LowerError> {
        self.require_optional(optional, place)?;
        let fact = self.optional_fact(place)?;
        let address = self.optional_state_address(block, place, &fact)?;
        let value = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(value)),
        )?[0];
        self.store_optional_at(block, address, value, state_representation())
    }

    pub(super) fn is_nonzero(
        &mut self,
        block: BlockId,
        value: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let zero = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(0)),
        )?[0];
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: value,
                right: zero,
            },
        )?[0])
    }

    pub(super) fn trap_if_optional_pinned(
        &mut self,
        block: BlockId,
        place: &MirPlace,
        optional: OptionalTypeId,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, place, optional)?;
        let one = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(1)),
        )?[0];
        let zero = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(0)),
        )?[0];
        let absent = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::Equal,
                left: state,
                right: zero,
            },
        )?[0];
        let single = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::Equal,
                left: state,
                right: one,
            },
        )?[0];
        let allowed_if_absent = self.new_optional_block()?;
        let test_single = self.new_optional_block()?;
        let trap = self.new_optional_block()?;
        let allowed = self.new_optional_block()?;
        self.branch_active(block, absent, allowed_if_absent, test_single)?;
        self.builder
            .terminate(allowed_if_absent, Terminator::Jump(edge(allowed)))?;
        self.builder.terminate(
            test_single,
            Terminator::Branch {
                condition: single,
                true_edge: edge(allowed),
                false_edge: edge(trap),
            },
        )?;
        self.builder.terminate(trap, Terminator::HardTrap)?;
        self.active_blocks[block.index()] = allowed;
        Ok(())
    }
}

fn state_representation() -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    }
}

fn edge<'p>(
    target: crate::backend::lir::BlockHandle<'p>,
) -> crate::backend::lir::Edge<
    crate::backend::lir::ValueHandle<'p>,
    crate::backend::lir::BlockHandle<'p>,
> {
    crate::backend::lir::Edge {
        target,
        arguments: vec![],
    }
}

fn mir_shared_target(target: crate::backend::plan::SharedTarget) -> crate::mir::MirSharedTarget {
    match target {
        crate::backend::plan::SharedTarget::Obj => crate::mir::MirSharedTarget::Obj,
        crate::backend::plan::SharedTarget::Class(id) => crate::mir::MirSharedTarget::Class(id),
        crate::backend::plan::SharedTarget::Interface(id) => {
            crate::mir::MirSharedTarget::Interface(id)
        }
        crate::backend::plan::SharedTarget::Array(id) => crate::mir::MirSharedTarget::Array(id),
        crate::backend::plan::SharedTarget::OptionalBox(id) => {
            crate::mir::MirSharedTarget::OptionalBox(id)
        }
    }
}
