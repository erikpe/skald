//! Exhaustive target-independent MIR dependency extraction.

use std::collections::BTreeSet;

use crate::{
    identity::{CallableId, FunctionTypeId},
    mir::{
        MirArrayLifecycleOperation, MirClassLifecycleOperation, MirDefinitionRef, MirExecutionNode,
        MirInstruction, MirProgram, MirStaticInitializerBody, MirTerminator, PreliminaryMirProgram,
    },
    source::Span,
};

use super::{
    definitions::{declared_executable_callables, MirExecutableDefinitionView},
    mir_dependency_edge_kind_key, mir_execution_node_key, mir_span_key,
    target::{MirResolvedCallTarget, MirTargetResolver},
    MirCallableAddressFormation, MirDependencyEdge, MirDependencyEdgeKind,
    MirDependencyExtractionError, MirDependencyRecord, MirDependencyRegion, MirDependencyTarget,
    MirIndirectCallSite, MirRuntimeEntity,
};

/// Deterministic dependency inventory before root closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirDependencyExtraction {
    nodes: Vec<MirExecutionNode>,
    dependencies: Vec<MirDependencyRecord>,
    callable_addresses: Vec<MirCallableAddressFormation>,
    indirect_calls: Vec<MirIndirectCallSite>,
}

impl MirDependencyExtraction {
    pub(crate) fn nodes(&self) -> &[MirExecutionNode] {
        &self.nodes
    }

    pub(crate) fn dependencies(&self) -> &[MirDependencyRecord] {
        &self.dependencies
    }

    pub(crate) fn callable_addresses(&self) -> &[MirCallableAddressFormation] {
        &self.callable_addresses
    }

    pub(crate) fn indirect_calls(&self) -> &[MirIndirectCallSite] {
        &self.indirect_calls
    }

    /// Returns the whole-program exact-signature candidates needed by the
    /// existing static-effect consumer. Reachability closure instead consumes
    /// formations by their containing source node.
    pub(crate) fn all_indirect_targets(
        &self,
        function_type: FunctionTypeId,
    ) -> impl Iterator<Item = CallableId> + '_ {
        let mut previous = None;
        self.callable_addresses
            .iter()
            .filter(move |formation| formation.function_type() == function_type)
            .filter_map(move |formation| {
                let target = formation.target();
                if previous == Some(target) {
                    None
                } else {
                    previous = Some(target);
                    Some(target)
                }
            })
    }
}

pub(crate) fn extract_preliminary_dependencies(
    program: &PreliminaryMirProgram,
) -> Result<MirDependencyExtraction, MirDependencyExtractionError> {
    extract_dependencies(MirExecutableDefinitionView::preliminary(program))
}

pub(crate) fn extract_final_dependencies(
    program: &MirProgram,
) -> Result<MirDependencyExtraction, MirDependencyExtractionError> {
    extract_dependencies(MirExecutableDefinitionView::final_program(program))
}

pub(crate) fn extract_final_dependency_parts(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
) -> Result<MirDependencyExtraction, MirDependencyExtractionError> {
    extract_dependencies(MirExecutableDefinitionView::from_parts(
        program,
        initializers,
    ))
}

pub(crate) fn extract_dependencies(
    definitions: MirExecutableDefinitionView<'_>,
) -> Result<MirDependencyExtraction, MirDependencyExtractionError> {
    let mut extractor = MirDependencyExtractor::new(definitions);
    extractor.seed_nodes();
    extractor.extract_implicit_lifecycle()?;
    extractor.extract_bodies()?;
    Ok(extractor.finish())
}

pub(super) struct MirDependencyExtractor<'mir> {
    pub(super) definitions: MirExecutableDefinitionView<'mir>,
    pub(super) dependencies: Vec<MirDependencyRecord>,
    pub(super) callable_addresses: Vec<MirCallableAddressFormation>,
    pub(super) indirect_calls: Vec<MirIndirectCallSite>,
    nodes: Vec<MirExecutionNode>,
}

impl<'mir> MirDependencyExtractor<'mir> {
    fn new(definitions: MirExecutableDefinitionView<'mir>) -> Self {
        Self {
            definitions,
            dependencies: Vec::new(),
            callable_addresses: Vec::new(),
            indirect_calls: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub(super) const fn program(&self) -> &'mir MirProgram {
        self.definitions.program()
    }

    fn seed_nodes(&mut self) {
        self.nodes
            .extend(declared_executable_callables(self.program()).map(MirExecutionNode::callable));
        self.nodes.extend(
            self.definitions
                .iter()
                .map(|definition| MirExecutionNode::callable(definition.callable())),
        );
        for class in self.program().classes.iter() {
            for operation in [
                MirClassLifecycleOperation::CopyConstructor,
                MirClassLifecycleOperation::CopyAssignment,
                MirClassLifecycleOperation::CompleteFinalizer,
            ] {
                self.nodes
                    .push(MirExecutionNode::class(class.id, operation));
            }
        }
        for array in self.program().array_types.iter() {
            for operation in [
                MirArrayLifecycleOperation::Default,
                MirArrayLifecycleOperation::Copy,
                MirArrayLifecycleOperation::Assignment,
                MirArrayLifecycleOperation::Destruction,
            ] {
                self.nodes
                    .push(MirExecutionNode::array(array.id, operation));
            }
        }
    }

    fn finish(mut self) -> MirDependencyExtraction {
        self.nodes.sort_by_key(|node| mir_execution_node_key(*node));
        self.nodes.dedup();
        self.dependencies.sort_by_key(dependency_key);
        self.dependencies.dedup();
        self.callable_addresses.sort_by_key(|formation| {
            (
                formation.function_type(),
                formation.target(),
                mir_execution_node_key(formation.source()),
                mir_span_key(formation.span()),
            )
        });
        self.callable_addresses.dedup();
        self.indirect_calls.sort_by_key(|site| {
            (
                mir_execution_node_key(site.source()),
                site.function_type(),
                site.region(),
                mir_span_key(site.span()),
            )
        });
        self.indirect_calls.dedup();
        MirDependencyExtraction {
            nodes: self.nodes,
            dependencies: self.dependencies,
            callable_addresses: self.callable_addresses,
            indirect_calls: self.indirect_calls,
        }
    }

    pub(super) fn add_dependency(
        &mut self,
        source: MirExecutionNode,
        target: MirDependencyTarget,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) {
        self.dependencies.push(MirDependencyRecord::new(
            MirDependencyEdge::new(source, target, kind, span),
            region,
        ));
    }

    pub(super) fn add_execution_dependency(
        &mut self,
        source: MirExecutionNode,
        target: MirExecutionNode,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) {
        self.add_dependency(
            source,
            MirDependencyTarget::Execution(target),
            kind,
            region,
            span,
        );
    }

    pub(super) fn add_runtime_entity(
        &mut self,
        source: MirExecutionNode,
        entity: MirRuntimeEntity,
        region: MirDependencyRegion,
        span: Span,
    ) {
        self.add_dependency(
            source,
            MirDependencyTarget::RuntimeEntity(entity),
            MirDependencyEdgeKind::RuntimeEntityReference,
            region,
            span,
        );
    }

    fn extract_bodies(&mut self) -> Result<(), MirDependencyExtractionError> {
        for definition in self.definitions.iter() {
            let source = MirExecutionNode::callable(definition.callable());
            let after_publication = after_publication_blocks(definition);
            for block in &definition.body().blocks {
                let region = definition_region(definition, &after_publication, block.id);
                for instruction in &block.instructions {
                    self.extract_instruction(source, definition, region, instruction)?;
                }
                if let Some(terminator) = &block.terminator {
                    self.extract_terminator(source, region, terminator);
                }
            }
        }
        Ok(())
    }

    fn extract_instruction(
        &mut self,
        source: MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        instruction: &MirInstruction,
    ) -> Result<(), MirDependencyExtractionError> {
        let span = instruction.span();
        match instruction {
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
            MirInstruction::Assign(assignment) => {
                self.extract_rvalue(source, region, &assignment.rvalue.kind, span)?
            }
            MirInstruction::Call(call) => {
                match MirTargetResolver::new(self.program()).resolve_call(call.target)? {
                    MirResolvedCallTarget::Dependencies(targets) => {
                        for (target, kind) in targets {
                            self.add_dependency(source, target, kind, region, span);
                        }
                    }
                    MirResolvedCallTarget::Indirect(function_type) => {
                        self.indirect_calls.push(MirIndirectCallSite::new(
                            source,
                            function_type,
                            region,
                            span,
                        ));
                        self.add_runtime_entity(
                            source,
                            MirRuntimeEntity::FunctionType(function_type),
                            region,
                            span,
                        );
                    }
                }
            }
            MirInstruction::Cleanup(cleanup) => self.add_complete_finalizer(
                source,
                cleanup.target,
                MirDependencyEdgeKind::CompleteFinalizer,
                region,
                span,
            )?,
            MirInstruction::Initialize(initialize) => self.add_initializer(
                source,
                initialize.target,
                MirDependencyEdgeKind::Initializer,
                region,
                span,
            )?,
            MirInstruction::CopyConstruct(copy) => self.add_copy_constructor(
                source,
                copy.operation,
                MirDependencyEdgeKind::CopyConstructor,
                region,
                span,
            )?,
            MirInstruction::CopyAssign(copy) => self.add_copy_assignment(
                source,
                copy.operation,
                MirDependencyEdgeKind::CopyAssignment,
                region,
                span,
            )?,
            MirInstruction::EndFullExpression(end) => {
                for cleanup in &end.temporaries {
                    self.add_complete_finalizer(
                        source,
                        cleanup.target,
                        MirDependencyEdgeKind::TemporaryCleanup,
                        region,
                        cleanup.span,
                    )?;
                }
            }
            MirInstruction::SharedInitialize(initialize) => self.add_initializer(
                source,
                initialize.target,
                MirDependencyEdgeKind::Initializer,
                region,
                span,
            )?,
            MirInstruction::SharedRelease(release) => {
                let ty = definition
                    .storage(release.owner)
                    .ok_or(MirDependencyExtractionError::UnknownStorage(release.owner))?
                    .ty;
                self.add_shared_type_finalizers(source, ty, region, span)?;
            }
            MirInstruction::SharedFieldReplace(replace) => {
                let ty = self.place_type(definition, &replace.destination)?;
                self.add_shared_type_finalizers(source, ty, region, span)?;
            }
            MirInstruction::StringInitialize(initialize) => self.add_runtime_entity(
                source,
                MirRuntimeEntity::LiteralBacking(initialize.data),
                region,
                span,
            ),
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                if matches!(
                    initialize.source,
                    crate::mir::MirAggregateOptionalSource::Copy(_)
                ) {
                    self.add_optional_copy(source, initialize.optional, region, span)?;
                }
            }
            MirInstruction::AggregateOptionalAssign(assign) => {
                self.add_optional_assignment(source, assign.optional, region, span)?
            }
            MirInstruction::AggregateOptionalCleanup(cleanup) => {
                self.add_optional_cleanup(source, cleanup.optional, region, span)?
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                if let Some(operation) = initialize.copy_constructor {
                    self.add_copy_constructor(
                        source,
                        operation,
                        MirDependencyEdgeKind::CopyConstructor,
                        region,
                        span,
                    )?;
                }
            }
            MirInstruction::ClassOptionalAssign(assign) => {
                self.add_complete_finalizer(
                    source,
                    assign.class,
                    MirDependencyEdgeKind::OptionalLifecycle,
                    region,
                    span,
                )?;
                if let Some(operation) = assign.copy_constructor {
                    self.add_copy_constructor(
                        source,
                        operation,
                        MirDependencyEdgeKind::CopyConstructor,
                        region,
                        span,
                    )?;
                }
                if let Some(operation) = assign.copy_assignment {
                    self.add_copy_assignment(
                        source,
                        operation,
                        MirDependencyEdgeKind::CopyAssignment,
                        region,
                        span,
                    )?;
                }
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => self.add_complete_finalizer(
                source,
                cleanup.class,
                MirDependencyEdgeKind::OptionalLifecycle,
                region,
                span,
            )?,
            MirInstruction::OptionalSharedAssign(assign) => self.add_shared_finalizers(
                source,
                assign.target,
                MirDependencyEdgeKind::OptionalLifecycle,
                region,
                span,
            )?,
            MirInstruction::OptionalSharedCleanup(cleanup) => self.add_shared_finalizers(
                source,
                cleanup.target,
                MirDependencyEdgeKind::OptionalLifecycle,
                region,
                span,
            )?,
            MirInstruction::Array(array) => {
                self.extract_array_instruction(source, region, array)?
            }
            MirInstruction::Store(_)
            | MirInstruction::BindCheckedView(_)
            | MirInstruction::EndCheckedView(_)
            | MirInstruction::SharedAllocate(_)
            | MirInstruction::SharedPublish(_)
            | MirInstruction::SharedStatic(_)
            | MirInstruction::SharedAdopt(_)
            | MirInstruction::SharedCopy(_)
            | MirInstruction::SharedFieldCopy(_)
            | MirInstruction::SharedCast(_)
            | MirInstruction::SharedMove(_)
            | MirInstruction::SharedFieldInitialize(_)
            | MirInstruction::OptionalInitialize(_)
            | MirInstruction::OptionalAssign(_)
            | MirInstruction::AggregateOptionalPublish(_)
            | MirInstruction::ClassOptionalPublish(_)
            | MirInstruction::EndOptionalView(_)
            | MirInstruction::EndOptionalBoxView(_)
            | MirInstruction::OptionalSharedInitialize(_)
            | MirInstruction::Io(_) => {}
        }
        Ok(())
    }

    fn extract_rvalue(
        &mut self,
        source: MirExecutionNode,
        region: MirDependencyRegion,
        rvalue: &crate::mir::MirRvalueKind,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        match rvalue {
            crate::mir::MirRvalueKind::CallableAddress(address) => {
                MirTargetResolver::new(self.program())
                    .validate_callable_address(address.target, address.function_type)?;
                self.callable_addresses
                    .push(MirCallableAddressFormation::new(
                        source,
                        address.function_type,
                        address.target,
                        span,
                    ));
                self.add_execution_dependency(
                    source,
                    MirExecutionNode::callable(address.target),
                    MirDependencyEdgeKind::CallableAddressRetention,
                    region,
                    span,
                );
                self.add_runtime_entity(
                    source,
                    MirRuntimeEntity::FunctionType(address.function_type),
                    region,
                    span,
                );
            }
            crate::mir::MirRvalueKind::TypeTest { target, .. } => {
                self.add_view_target_entity(source, *target, region, span)
            }
            crate::mir::MirRvalueKind::OptionalBoxPresence { target, .. } => self
                .add_runtime_entity(
                    source,
                    MirRuntimeEntity::OptionalBoxLayout(*target),
                    region,
                    span,
                ),
            crate::mir::MirRvalueKind::OptionalPresence { kind, .. } => match kind {
                crate::mir::MirPresenceTestKind::Some | crate::mir::MirPresenceTestKind::None => {}
            },
            crate::mir::MirRvalueKind::ArrayLength { array, .. } => self.add_runtime_entity(
                source,
                MirRuntimeEntity::ArrayLifecycle(*array),
                region,
                span,
            ),
            crate::mir::MirRvalueKind::ConstantI64(_)
            | crate::mir::MirRvalueKind::ConstantU64(_)
            | crate::mir::MirRvalueKind::ConstantU8(_)
            | crate::mir::MirRvalueKind::ConstantF64Bits(_)
            | crate::mir::MirRvalueKind::ConstantBool(_)
            | crate::mir::MirRvalueKind::PathCondition(_)
            | crate::mir::MirRvalueKind::Load(_)
            | crate::mir::MirRvalueKind::Unary { .. }
            | crate::mir::MirRvalueKind::Binary { .. }
            | crate::mir::MirRvalueKind::IntegerDivision { .. }
            | crate::mir::MirRvalueKind::Shift { .. }
            | crate::mir::MirRvalueKind::PrimitiveComparison { .. }
            | crate::mir::MirRvalueKind::PrimitiveCast { .. }
            | crate::mir::MirRvalueKind::CheckedF64ToInteger { .. } => {}
        }
        Ok(())
    }

    fn extract_terminator(
        &mut self,
        source: MirExecutionNode,
        region: MirDependencyRegion,
        terminator: &MirTerminator,
    ) {
        let span = terminator.span();
        match terminator {
            MirTerminator::CheckedCast { binding, .. } => {
                self.add_view_target_entity(source, binding.view.target, region, span)
            }
            MirTerminator::BeginOptionalBoxView { begin, .. } => self.add_runtime_entity(
                source,
                MirRuntimeEntity::OptionalBoxLayout(begin.box_target),
                region,
                span,
            ),
            MirTerminator::Return { .. }
            | MirTerminator::ReturnShared { .. }
            | MirTerminator::ReturnOptionalShared { .. }
            | MirTerminator::Panic { .. }
            | MirTerminator::Goto { .. }
            | MirTerminator::Branch { .. }
            | MirTerminator::ShiftCountCheck { .. }
            | MirTerminator::IntegerDivisorCheck { .. }
            | MirTerminator::PrimitiveCastRangeCheck { .. }
            | MirTerminator::SharedCast { .. }
            | MirTerminator::OptionalUnwrap { .. }
            | MirTerminator::OptionalSharedUnwrap { .. }
            | MirTerminator::BeginOptionalView { .. }
            | MirTerminator::CheckOptionalMutation { .. }
            | MirTerminator::ArrayPositionCheck { .. }
            | MirTerminator::ArrayOperationCheck { .. }
            | MirTerminator::ArrayLoop { .. }
            | MirTerminator::Terminate { .. } => {}
        }
    }

    fn add_view_target_entity(
        &mut self,
        source: MirExecutionNode,
        target: crate::mir::MirViewTarget,
        region: MirDependencyRegion,
        span: Span,
    ) {
        match target {
            crate::mir::MirViewTarget::Class(class) => self.add_runtime_entity(
                source,
                MirRuntimeEntity::ClassDispatch(class),
                region,
                span,
            ),
            crate::mir::MirViewTarget::Interface(_) | crate::mir::MirViewTarget::Obj => {}
        }
    }

    fn place_type(
        &self,
        definition: MirDefinitionRef<'_>,
        place: &crate::mir::MirPlace,
    ) -> Result<crate::mir::MirType, MirDependencyExtractionError> {
        let mut ty = match place.base {
            crate::mir::MirPlaceBase::StaticField(field)
            | crate::mir::MirPlaceBase::StaticLifecycleDestination(field) => self
                .program()
                .static_field(field)
                .map(|declaration| declaration.ty)
                .ok_or(MirDependencyExtractionError::UnknownStaticField(field))?,
            base => {
                let storage = base
                    .local_storage()
                    .ok_or(MirDependencyExtractionError::InvalidPlaceBase(base))?;
                definition
                    .storage(storage)
                    .map(|declaration| declaration.ty)
                    .ok_or(MirDependencyExtractionError::UnknownStorage(storage))?
            }
        };
        for projection in &place.projections {
            ty = match *projection {
                crate::mir::MirPlaceProjection::Base(class)
                | crate::mir::MirPlaceProjection::OptionalPayload(class) => {
                    if self.program().class(class).is_none() {
                        return Err(MirDependencyExtractionError::UnknownClass(class));
                    }
                    crate::mir::MirType::Class(class)
                }
                crate::mir::MirPlaceProjection::Field(field) => self
                    .program()
                    .field(field)
                    .map(|declaration| declaration.ty)
                    .ok_or(MirDependencyExtractionError::UnknownField(field))?,
                crate::mir::MirPlaceProjection::AggregateOptionalPayload(optional)
                | crate::mir::MirPlaceProjection::CheckedOptionalPayload(optional) => self
                    .program()
                    .optional_type(optional)
                    .map(|metadata| metadata.payload)
                    .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?,
                crate::mir::MirPlaceProjection::ArrayElement { array, .. } => self
                    .program()
                    .array_type(array)
                    .map(|metadata| metadata.element)
                    .ok_or(MirDependencyExtractionError::UnknownArrayType(array))?,
            };
        }
        Ok(ty)
    }
}

type MirDependencySortKey = (
    (u8, usize, usize, usize),
    u8,
    MirDependencyTarget,
    MirDependencyRegion,
    (usize, usize, usize),
);

fn dependency_key(dependency: &MirDependencyRecord) -> MirDependencySortKey {
    let edge = dependency.edge();
    (
        mir_execution_node_key(edge.source()),
        mir_dependency_edge_kind_key(edge.kind()),
        edge.target(),
        dependency.region(),
        mir_span_key(edge.span()),
    )
}

fn after_publication_blocks(definition: MirDefinitionRef<'_>) -> BTreeSet<crate::mir::BlockId> {
    let MirDefinitionRef::StaticInitializer(initializer) = definition else {
        return BTreeSet::new();
    };
    let mut found = BTreeSet::new();
    let mut pending = vec![initializer.publication.cleanup_entry];
    while let Some(block) = pending.pop() {
        if !found.insert(block) {
            continue;
        }
        if let Some(terminator) = initializer
            .block(block)
            .and_then(|block| block.terminator.as_ref())
        {
            pending.extend(terminator.successors());
        }
    }
    found
}

fn definition_region(
    definition: MirDefinitionRef<'_>,
    after_publication: &BTreeSet<crate::mir::BlockId>,
    block: crate::mir::BlockId,
) -> MirDependencyRegion {
    match definition {
        MirDefinitionRef::StaticInitializer(_) if after_publication.contains(&block) => {
            MirDependencyRegion::StaticInitializerAfterPublication
        }
        MirDefinitionRef::StaticInitializer(_) => {
            MirDependencyRegion::StaticInitializerBeforePublication
        }
        MirDefinitionRef::Function(_) | MirDefinitionRef::Member(_) => {
            MirDependencyRegion::Ordinary
        }
    }
}
