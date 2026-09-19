//! Projection of complete immutable data, generated work, and retention facts.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    backend::{failure::FailureMessage, plan::*, BackendInput, BackendRequiredRuntimeEntity},
    mir::{MirProgram, MirSharedTarget, MirStaticActivationWork, MirStaticValueCleanup, MirType},
};

use super::{AdmissionError, TraceFacts};

pub(super) fn project(
    input: BackendInput<'_>,
    layouts: &[(MirType, LayoutId)],
    trace: &TraceFacts,
    facts: &mut PlanFacts,
) -> Result<(), AdmissionError> {
    let program = input.program();
    declare_runtime_services(facts)?;
    declare_static_resources(input, program, layouts, facts)?;
    let mut entry_dependencies = [
        ArtifactId::Runtime(RuntimeService::AbiMarker),
        ArtifactId::Callable(LirCallableId::Source(program.entry_function.into())),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    entry_dependencies.extend(facts.resources.generated.iter().filter_map(|generated| {
        match generated.callable {
            LirCallableId::Coordinator(_) => Some(ArtifactId::Callable(generated.callable)),
            _ => None,
        }
    }));
    facts.resources.generated.push(GeneratedCallableFact {
        callable: LirCallableId::Entry,
        attribution: GeneratedAttribution::EntryWrapper,
        dependencies: entry_dependencies,
    });
    declare_generated_resources(input, facts)?;
    declare_metadata(input, facts)?;
    declare_literals(input, program, facts)?;
    declare_failure_messages(facts)?;
    declare_trace(trace, facts)?;
    declare_roots(input, facts);
    facts.resources.statics.sort_by_key(|item| item.field);
    facts.resources.data.sort_by_key(|item| item.key);
    facts.resources.generated.sort_by_key(|item| item.callable);
    Ok(())
}

fn declare_runtime_services(facts: &mut PlanFacts) -> Result<(), PlanError> {
    for service in [
        RuntimeService::Allocate,
        RuntimeService::Free,
        RuntimeService::Panic,
        RuntimeService::IoStandardHandle,
        RuntimeService::IoOpen,
        RuntimeService::IoRead,
        RuntimeService::IoWrite,
        RuntimeService::IoClose,
        RuntimeService::AbiMarker,
    ] {
        if facts
            .artifacts
            .iter()
            .any(|artifact| artifact.key == ArtifactId::Runtime(service))
        {
            continue;
        }
        let signature = facts.add_signature(service_signature(service))?;
        facts.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Runtime(service),
            signature: Some(signature),
            layout: None,
        });
    }
    Ok(())
}

fn declare_static_resources(
    input: BackendInput<'_>,
    program: &MirProgram,
    layouts: &[(MirType, LayoutId)],
    facts: &mut PlanFacts,
) -> Result<(), AdmissionError> {
    let active = input
        .active_static_fields()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let referenced = input
        .retained_static_fields()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if input.reachable_artifacts_only() && !referenced.is_subset(&active) {
        return Err(PlanError::InvalidDomain.into());
    }
    let retained = if input.reachable_artifacts_only() {
        active.clone()
    } else {
        referenced
            .into_iter()
            .chain(active.iter().copied())
            .collect()
    };
    for field in retained {
        let declaration = program
            .static_field(field)
            .ok_or(PlanError::UnknownDeclaration)?;
        let ty = semantic_type(declaration.ty);
        let layout = layout_id(layouts, declaration.ty)?;
        let disposition = if active.contains(&field) {
            StaticStorageDisposition::Active
        } else {
            StaticStorageDisposition::RetainedInactive
        };
        facts.resources.statics.push(StaticStorageFact {
            field,
            ty,
            layout,
            disposition,
        });
        facts.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Static(field)),
            signature: None,
            layout: Some(layout),
        });
        let size = facts.layouts[layout.index()].size;
        facts.resources.data.push(DataFact {
            key: DataKey::Static(field),
            purpose: DataPurpose::StaticStorage(disposition),
            layout,
            initializers: vec![DataInitializerFact::Zero(size)],
            dependencies: BTreeSet::new(),
        });
    }

    if let Some(coordinator) = &program.static_lifecycle {
        for activation in coordinator.activation() {
            facts.resources.activation.push(StaticActivationFact {
                field: activation.field,
                action: match activation.work {
                    MirStaticActivationWork::ZeroDefault => StaticActivationKind::ZeroDefault,
                    MirStaticActivationWork::Explicit(initializer) => {
                        StaticActivationKind::Explicit(LirCallableId::Source(initializer.into()))
                    }
                },
            });
        }
        for shutdown in coordinator.shutdown() {
            facts.resources.shutdown.push(StaticShutdownFact {
                field: shutdown.field,
                cleanup: static_cleanup(&shutdown.cleanup),
            });
        }
        if !coordinator.activation().is_empty() {
            declare_generated(
                facts,
                LirCallableId::Coordinator(Coordinator::Initializer),
                unit_signature(),
                GeneratedAttribution::ProgramLifecycle,
                coordinator
                    .activation()
                    .iter()
                    .flat_map(|activation| {
                        let storage = ArtifactId::Data(DataKey::Static(activation.field));
                        match activation.work {
                            MirStaticActivationWork::ZeroDefault => vec![storage],
                            MirStaticActivationWork::Explicit(initializer) => vec![
                                storage,
                                ArtifactId::Callable(LirCallableId::Source(initializer.into())),
                            ],
                        }
                    })
                    .collect(),
            )?;
            declare_generated(
                facts,
                LirCallableId::Coordinator(Coordinator::Finalizer),
                unit_signature(),
                GeneratedAttribution::ProgramLifecycle,
                coordinator
                    .shutdown()
                    .iter()
                    .map(|shutdown| ArtifactId::Data(DataKey::Static(shutdown.field)))
                    .collect(),
            )?;
        }
    }
    Ok(())
}

fn declare_generated_resources(
    input: BackendInput<'_>,
    facts: &mut PlanFacts,
) -> Result<(), PlanError> {
    let required = input.required_runtime_entities().collect::<Vec<_>>();
    let class_artifacts = required_class_artifacts(input, &required, facts)?;
    for array in &facts.semantic.arrays.clone() {
        if input.reachable_artifacts_only()
            && !required.contains(&BackendRequiredRuntimeEntity::ArrayLifecycle(array.array))
        {
            continue;
        }
        let shapes = [
            (
                HelperFamily::ArrayElementInitializer,
                helper_signature(
                    &[ScalarType::DataAddress, ScalarType::U64],
                    ReturnShape::Unit,
                ),
            ),
            (
                HelperFamily::ArrayElementCopier,
                helper_signature(
                    &[
                        ScalarType::DataAddress,
                        ScalarType::DataAddress,
                        ScalarType::U64,
                        ScalarType::U64,
                    ],
                    ReturnShape::Unit,
                ),
            ),
            (
                HelperFamily::ArrayClone,
                helper_signature(
                    &[ScalarType::DataAddress],
                    ReturnShape::Scalar(ScalarType::DataAddress),
                ),
            ),
            (
                HelperFamily::ArrayElementDestroyer,
                helper_signature(
                    &[ScalarType::DataAddress, ScalarType::U64],
                    ReturnShape::Unit,
                ),
            ),
            (
                HelperFamily::ArrayRelease,
                helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
            ),
            (
                HelperFamily::ArraySharedFinalizer,
                helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
            ),
        ];
        let mut keys = BTreeMap::new();
        for (family, signature) in shapes {
            let key = declare_helper(
                facts,
                family,
                array.descriptor_layout,
                signature,
                BTreeSet::new(),
            )?;
            keys.insert(family, key);
        }
        add_generated_dependency(
            facts,
            keys[&HelperFamily::ArrayClone],
            ArtifactId::Callable(keys[&HelperFamily::ArrayElementCopier]),
        );
        add_generated_dependency(
            facts,
            keys[&HelperFamily::ArrayClone],
            ArtifactId::Runtime(RuntimeService::Allocate),
        );
        add_generated_dependency(
            facts,
            keys[&HelperFamily::ArrayRelease],
            ArtifactId::Callable(keys[&HelperFamily::ArraySharedFinalizer]),
        );
        add_generated_dependency(
            facts,
            keys[&HelperFamily::ArrayRelease],
            ArtifactId::Runtime(RuntimeService::Free),
        );
        add_generated_dependency(
            facts,
            keys[&HelperFamily::ArraySharedFinalizer],
            ArtifactId::Callable(keys[&HelperFamily::ArrayElementDestroyer]),
        );
    }
    let (needs_retain, needs_release) = owner_helper_needs(input, &class_artifacts, facts);
    if needs_retain || needs_release {
        let layout = facts
            .semantic
            .shared_header
            .ok_or(PlanError::InvalidLayout)?
            .handle_layout;
        if needs_retain {
            let retain = declare_helper(
                facts,
                HelperFamily::Retain,
                layout,
                helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
                BTreeSet::new(),
            )?;
            for dependency in [
                ArtifactId::Callable(retain),
                ArtifactId::Runtime(RuntimeService::Panic),
                ArtifactId::Data(DataKey::FailureMessage(
                    FailureMessage::OwnershipCountOverflow,
                )),
            ] {
                add_generated_dependency(facts, retain, dependency);
            }
        }
        if needs_release {
            let release = declare_helper(
                facts,
                HelperFamily::Release,
                layout,
                helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
                BTreeSet::new(),
            )?;
            for dependency in [
                ArtifactId::Callable(release),
                ArtifactId::Runtime(RuntimeService::Free),
            ] {
                add_generated_dependency(facts, release, dependency);
            }
        }
    }
    let classes = facts
        .semantic
        .classes
        .clone()
        .into_iter()
        .filter(|class| !input.reachable_artifacts_only() || class_artifacts.contains(&class.class))
        .collect::<Vec<_>>();
    for class in &classes {
        declare_helper(
            facts,
            HelperFamily::ClassFinalizer,
            class.complete_layout,
            helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
            BTreeSet::new(),
        )?;
    }
    for class in &classes {
        let finalizer = helper_for(facts, HelperFamily::ClassFinalizer, class.complete_layout)?;
        if !class.destruction.is_empty() {
            // Inherited attribution names the generated boundary explicitly,
            // so the verified receipt includes that boundary alongside the
            // actual callee and data references.
            add_generated_dependency(facts, finalizer, ArtifactId::Callable(finalizer));
        }
        for step in &class.destruction {
            let dependency = match *step {
                DestructionStepFact::UserBody(id) => {
                    add_generated_dependency(
                        facts,
                        finalizer,
                        ArtifactId::Data(DataKey::ClassDispatch(class.class)),
                    );
                    ArtifactId::Callable(LirCallableId::Source(id.into()))
                }
                DestructionStepFact::Field(field) => {
                    let field = facts
                        .semantic
                        .field(field)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    let SemanticType::Class(target) = field.ty else {
                        return Err(PlanError::InvalidDomain);
                    };
                    ArtifactId::Callable(helper_for(
                        facts,
                        HelperFamily::ClassFinalizer,
                        facts.semantic.classes[target.index()].complete_layout,
                    )?)
                }
                DestructionStepFact::Base(target) => ArtifactId::Callable(helper_for(
                    facts,
                    HelperFamily::ClassFinalizer,
                    facts.semantic.classes[target.index()].complete_layout,
                )?),
                DestructionStepFact::SharedField(_)
                | DestructionStepFact::OptionalSharedField(_) => ArtifactId::Callable(helper_for(
                    facts,
                    HelperFamily::Release,
                    facts
                        .semantic
                        .shared_header
                        .ok_or(PlanError::InvalidLayout)?
                        .handle_layout,
                )?),
                DestructionStepFact::OptionalClassField(field)
                | DestructionStepFact::OptionalField { field, .. } => {
                    let field = facts
                        .semantic
                        .field(field)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    let SemanticType::Optional(optional) = field.ty else {
                        return Err(PlanError::InvalidDomain);
                    };
                    for dependency in optional_cleanup_dependencies(facts, optional)? {
                        add_generated_dependency(facts, finalizer, dependency);
                    }
                    continue;
                }
                _ => continue,
            };
            add_generated_dependency(facts, finalizer, dependency);
        }
        // The dispatch table requires every retained class finalizer. Raw
        // address copy wrappers are array-element machinery and acquire roots
        // only when the array helper family is introduced.
    }
    for optional_box in facts.semantic.optional_boxes.clone() {
        if input.reachable_artifacts_only()
            && !required.contains(&BackendRequiredRuntimeEntity::OptionalBoxLayout(
                optional_box.optional_box,
            ))
        {
            continue;
        }
        if optional_box.allocation.is_none() {
            continue;
        }
        let Some(optional) = optional_box.exact_optional else {
            continue;
        };
        let layout = facts
            .semantic
            .layout(SemanticType::Optional(optional))
            .ok_or(PlanError::InvalidLayout)?;
        let finalizer = declare_helper(
            facts,
            HelperFamily::OptionalBoxFinalizer,
            layout,
            helper_signature(&[ScalarType::DataAddress], ReturnShape::Unit),
            BTreeSet::new(),
        )?;
        let dependencies = optional_cleanup_dependencies(facts, optional)?;
        if !dependencies.is_empty() {
            // Inherited attribution names the generated boundary explicitly,
            // so the receipt includes the boundary as well as actual callees.
            add_generated_dependency(facts, finalizer, ArtifactId::Callable(finalizer));
        }
        for dependency in dependencies {
            add_generated_dependency(facts, finalizer, dependency);
        }
    }
    Ok(())
}

fn optional_cleanup_dependencies(
    facts: &PlanFacts,
    root: crate::identity::OptionalTypeId,
) -> Result<BTreeSet<ArtifactId>, PlanError> {
    let mut dependencies = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(optional) = pending.pop() {
        let fact = facts
            .semantic
            .optional(optional)
            .ok_or(PlanError::UnknownDeclaration)?;
        match fact.storage {
            crate::backend::plan::OptionalStorageFact::Scalar => {}
            crate::backend::plan::OptionalStorageFact::SharedOwner(_) => {
                dependencies.insert(ArtifactId::Callable(helper_for(
                    facts,
                    HelperFamily::Release,
                    facts
                        .semantic
                        .shared_header
                        .ok_or(PlanError::InvalidLayout)?
                        .handle_layout,
                )?));
            }
            crate::backend::plan::OptionalStorageFact::InlineClass(class) => {
                dependencies.insert(ArtifactId::Callable(helper_for(
                    facts,
                    HelperFamily::ClassFinalizer,
                    facts
                        .semantic
                        .class(class)
                        .ok_or(PlanError::UnknownDeclaration)?
                        .complete_layout,
                )?));
            }
            crate::backend::plan::OptionalStorageFact::Nested(inner) => pending.push(inner),
            crate::backend::plan::OptionalStorageFact::InlineArray(_) => {}
        }
    }
    Ok(dependencies)
}

fn declare_metadata(input: BackendInput<'_>, facts: &mut PlanFacts) -> Result<(), PlanError> {
    let required = input.required_runtime_entities().collect::<Vec<_>>();
    let class_artifacts = required_class_artifacts(input, &required, facts)?;
    let word = facts.profile.data_layout.pointer_bytes;
    for dispatch in facts.semantic.dispatch_tables.clone() {
        if input.reachable_artifacts_only() && !class_artifacts.contains(&dispatch.class) {
            continue;
        }
        let finalizer = helper_for(
            facts,
            HelperFamily::ClassFinalizer,
            facts.semantic.classes[dispatch.class.index()].complete_layout,
        )?;
        let initializers = dispatch
            .targets
            .into_iter()
            .map(callable_word)
            .chain([callable_word(Some(finalizer))])
            .collect::<Vec<_>>();
        add_data(
            facts,
            DataKey::ClassDispatch(dispatch.class),
            DataPurpose::ClassDispatch(dispatch.class),
            initializers
                .len()
                .checked_mul(word)
                .ok_or(PlanError::SizeOverflow)?,
            word,
            initializers,
        )?;
    }
    for array in facts.semantic.arrays.clone() {
        if input.reachable_artifacts_only()
            && !required.contains(&BackendRequiredRuntimeEntity::ArrayLifecycle(array.array))
        {
            continue;
        }
        let finalizer = helper_for(
            facts,
            HelperFamily::ArraySharedFinalizer,
            array.descriptor_layout,
        )?;
        let finalizer_index = facts
            .semantic
            .method_slots
            .iter()
            .find(|slot| slot.slot == MethodSlot::Finalizer)
            .map(|slot| slot.index)
            .unwrap_or(0);
        let mut initializers = vec![DataInitializerFact::Zero(
            finalizer_index
                .checked_mul(word)
                .ok_or(PlanError::SizeOverflow)?,
        )];
        initializers.push(callable_word(Some(finalizer)));
        add_data(
            facts,
            DataKey::ArrayDescriptor(array.array),
            DataPurpose::ArrayDescriptor(array.array),
            (finalizer_index + 1)
                .checked_mul(word)
                .ok_or(PlanError::SizeOverflow)?,
            word,
            initializers,
        )?;
    }
    for optional_box in facts.semantic.optional_boxes.clone() {
        if input.reachable_artifacts_only()
            && !required.contains(&BackendRequiredRuntimeEntity::OptionalBoxLayout(
                optional_box.optional_box,
            ))
        {
            continue;
        }
        if optional_box.exact_optional.is_none() {
            continue;
        }
        let layout = optional_box
            .exact_optional
            .and_then(|optional| facts.semantic.layout(SemanticType::Optional(optional)))
            .ok_or(PlanError::InvalidLayout)?;
        let finalizer = helper_for(facts, HelperFamily::OptionalBoxFinalizer, layout)?;
        let targets = optional_box
            .exact_dynamic_class
            .and_then(|class| facts.semantic.dispatch_table(class))
            .map_or_else(Vec::new, |table| table.targets.clone());
        let finalizer_index = facts
            .semantic
            .method_slots
            .iter()
            .find(|slot| slot.slot == MethodSlot::Finalizer)
            .map(|slot| slot.index)
            .unwrap_or(0);
        let mut initializers = targets
            .into_iter()
            .take(finalizer_index)
            .map(callable_word)
            .collect::<Vec<_>>();
        initializers.resize(finalizer_index, DataInitializerFact::Zero(word));
        initializers.push(callable_word(Some(finalizer)));
        add_data(
            facts,
            DataKey::OptionalBoxDescriptor(optional_box.optional_box),
            DataPurpose::OptionalBoxDescriptor(optional_box.optional_box),
            (finalizer_index + 1)
                .checked_mul(word)
                .ok_or(PlanError::SizeOverflow)?,
            word,
            initializers,
        )?;
    }
    Ok(())
}

fn required_class_artifacts(
    input: BackendInput<'_>,
    required: &[BackendRequiredRuntimeEntity],
    facts: &PlanFacts,
) -> Result<BTreeSet<crate::identity::ClassId>, PlanError> {
    if !input.reachable_artifacts_only() {
        return Ok(facts
            .semantic
            .classes
            .iter()
            .map(|class| class.class)
            .collect());
    }
    let reachable = input
        .reachable_callables()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut classes = required
        .iter()
        .filter_map(|entity| match entity {
            BackendRequiredRuntimeEntity::ClassDispatch(class) => Some(*class),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for callable in &reachable {
        match *callable {
            crate::identity::CallableId::Initializer(id) => {
                classes.insert(id.class());
            }
            crate::identity::CallableId::CopyConstructor(id) => {
                classes.insert(id.class());
            }
            crate::identity::CallableId::CopyAssignment(id) => {
                classes.insert(id.class());
            }
            crate::identity::CallableId::Destructor(id) => {
                classes.insert(id.class());
            }
            crate::identity::CallableId::Method(id) => {
                classes.insert(id.class());
            }
            crate::identity::CallableId::Function(_)
            | crate::identity::CallableId::StaticInitializer(_) => {}
        }
    }
    for definition in input.program().executable_definitions() {
        if !reachable.contains(&definition.callable()) {
            continue;
        }
        for instruction in definition
            .body()
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
        {
            match instruction {
                crate::mir::MirInstruction::Initialize(initialize) => {
                    classes.insert(initialize.target.class());
                }
                crate::mir::MirInstruction::CopyConstruct(copy) => {
                    classes.insert(copy.class);
                }
                crate::mir::MirInstruction::CopyAssign(copy) => {
                    classes.insert(copy.class);
                }
                crate::mir::MirInstruction::Cleanup(cleanup) => {
                    classes.insert(cleanup.target);
                }
                crate::mir::MirInstruction::EndFullExpression(end) => {
                    classes.extend(end.temporaries.iter().map(|cleanup| cleanup.target));
                }
                _ => {}
            }
        }
    }
    let mut pending = classes.iter().copied().collect::<Vec<_>>();
    while let Some(class) = pending.pop() {
        let fact = facts
            .semantic
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?;
        for step in &fact.destruction {
            let target = match *step {
                DestructionStepFact::Field(field) => {
                    let field = facts
                        .semantic
                        .field(field)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    let SemanticType::Class(target) = field.ty else {
                        return Err(PlanError::InvalidDomain);
                    };
                    Some(target)
                }
                DestructionStepFact::Base(target) => Some(target),
                _ => None,
            };
            if let Some(target) = target {
                if classes.insert(target) {
                    pending.push(target);
                }
            }
        }
    }
    Ok(classes)
}

fn owner_helper_needs(
    input: BackendInput<'_>,
    classes: &BTreeSet<crate::identity::ClassId>,
    facts: &PlanFacts,
) -> (bool, bool) {
    let reachable = if input.reachable_artifacts_only() {
        input
            .reachable_callables()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    } else {
        input
            .program()
            .executable_definitions()
            .map(|definition| definition.callable())
            .collect()
    };
    let mut retain = false;
    let mut release = classes.iter().any(|class| {
        facts.semantic.class(*class).is_some_and(|class| {
            class.destruction.iter().any(|step| {
                matches!(
                    step,
                    DestructionStepFact::SharedField(_)
                        | DestructionStepFact::OptionalSharedField(_)
                )
            })
        })
    });
    for definition in input.program().executable_definitions() {
        if !reachable.contains(&definition.callable()) {
            continue;
        }
        for instruction in definition
            .body()
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
        {
            match instruction {
                crate::mir::MirInstruction::SharedCopy(_)
                | crate::mir::MirInstruction::SharedFieldCopy(_) => retain = true,
                crate::mir::MirInstruction::SharedCast(cast)
                    if cast.transfer == crate::mir::MirSharedCastTransfer::Copy =>
                {
                    retain = true
                }
                crate::mir::MirInstruction::SharedRelease(_)
                | crate::mir::MirInstruction::SharedFieldReplace(_) => release = true,
                crate::mir::MirInstruction::OptionalSharedInitialize(initialize) => {
                    retain |= matches!(
                        initialize.source,
                        crate::mir::MirOptionalSharedSource::Copy(_)
                    );
                }
                crate::mir::MirInstruction::OptionalSharedAssign(assign) => {
                    retain |= matches!(assign.source, crate::mir::MirOptionalSharedSource::Copy(_));
                    release = true;
                }
                crate::mir::MirInstruction::OptionalSharedCleanup(_) => release = true,
                crate::mir::MirInstruction::CopyConstruct(copy) => {
                    retain |= constructor_copy_uses_shared(input.program(), copy.operation)
                }
                crate::mir::MirInstruction::CopyAssign(copy) => {
                    let shared = assignment_copy_uses_shared(input.program(), copy.operation);
                    retain |= shared;
                    release |= shared;
                }
                _ => {}
            }
        }
        for block in &definition.body().blocks {
            if matches!(
                block.terminator,
                Some(crate::mir::MirTerminator::SharedCast {
                    cast: crate::mir::MirSharedCast {
                        transfer: crate::mir::MirSharedCastTransfer::Copy,
                        ..
                    },
                    ..
                }) | Some(crate::mir::MirTerminator::OptionalSharedUnwrap { .. })
            ) {
                retain = true;
            }
        }
    }
    (retain, release)
}

fn constructor_copy_uses_shared(
    program: &MirProgram,
    root: crate::mir::MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
) -> bool {
    copy_uses_shared(program, root, |class| &class.copy_constructor)
}

fn assignment_copy_uses_shared(
    program: &MirProgram,
    root: crate::mir::MirSelectedCopyOperation<crate::identity::CopyAssignmentId>,
) -> bool {
    copy_uses_shared(program, root, |class| &class.copy_assignment)
}

fn copy_uses_shared<I: Copy>(
    program: &MirProgram,
    root: crate::mir::MirSelectedCopyOperation<I>,
    capability: impl Fn(&crate::mir::MirClassDeclaration) -> &crate::mir::MirCopyCapability<I>,
) -> bool
where
    crate::mir::MirSelectedCopyOperation<I>: Copy,
{
    let mut pending = vec![root];
    while let Some(operation) = pending.pop() {
        let class = match operation {
            crate::mir::MirSelectedCopyOperation::User(_) => continue,
            crate::mir::MirSelectedCopyOperation::Synthesized(class) => class,
        };
        let Some(class) = program.class(class) else {
            continue;
        };
        let crate::mir::MirCopyCapability::Synthesized(copy) = capability(class) else {
            continue;
        };
        if let Some(base) = copy.base {
            pending.push(base.operation);
        }
        for field in &copy.fields {
            match *field {
                crate::mir::MirSynthesizedFieldCopy::Shared { .. }
                | crate::mir::MirSynthesizedFieldCopy::OptionalShared { .. } => return true,
                crate::mir::MirSynthesizedFieldCopy::Class { operation, .. } => {
                    pending.push(operation)
                }
                _ => {}
            }
        }
    }
    false
}

fn declare_literals(
    input: BackendInput<'_>,
    program: &MirProgram,
    facts: &mut PlanFacts,
) -> Result<(), PlanError> {
    let reachable = input
        .required_runtime_entities()
        .filter_map(|entity| match entity {
            BackendRequiredRuntimeEntity::LiteralBacking(data) => Some(data),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for literal in program.literal_data.iter() {
        if input.reachable_artifacts_only() && !reachable.contains(&literal.id) {
            continue;
        }
        let descriptor = ArtifactId::Data(DataKey::ArrayDescriptor(literal.array));
        let mut initializers = vec![
            DataInitializerFact::Bytes(u64::MAX.to_le_bytes().to_vec()),
            address(descriptor),
            DataInitializerFact::Bytes(literal.length.to_le_bytes().to_vec()),
        ];
        initializers.push(DataInitializerFact::Bytes(literal.bytes.clone()));
        add_data(
            facts,
            DataKey::Literal(literal.id),
            DataPurpose::LiteralBacking,
            24usize
                .checked_add(literal.bytes.len())
                .ok_or(PlanError::SizeOverflow)?,
            8,
            initializers,
        )?;
    }
    Ok(())
}

fn declare_failure_messages(facts: &mut PlanFacts) -> Result<(), PlanError> {
    for message in FailureMessage::ALL {
        add_data(
            facts,
            DataKey::FailureMessage(message),
            DataPurpose::FailureMessage,
            message.bytes().len(),
            1,
            vec![DataInitializerFact::Bytes(message.bytes().to_vec())],
        )?;
    }
    Ok(())
}

fn declare_trace(trace: &TraceFacts, facts: &mut PlanFacts) -> Result<(), PlanError> {
    for (index, bytes) in trace.strings.iter().enumerate() {
        add_data(
            facts,
            DataKey::TraceBytes(index),
            DataPurpose::TraceBytes,
            bytes.len(),
            1,
            vec![DataInitializerFact::Bytes(bytes.clone())],
        )?;
    }
    for (index, context) in trace.contexts.iter().enumerate() {
        let length = |key| match key {
            DataKey::TraceBytes(index) => Ok(trace.strings[index].len() as u64),
            _ => Err(PlanError::InvalidDomain),
        };
        add_data(
            facts,
            DataKey::TraceContext(index),
            DataPurpose::TraceContext,
            32,
            8,
            vec![
                address(ArtifactId::Data(context.name)),
                DataInitializerFact::Bytes(length(context.name)?.to_le_bytes().to_vec()),
                address(ArtifactId::Data(context.path)),
                DataInitializerFact::Bytes(length(context.path)?.to_le_bytes().to_vec()),
            ],
        )?;
    }
    for (index, location) in trace.locations.iter().enumerate() {
        add_data(
            facts,
            DataKey::TraceLocation(index),
            DataPurpose::TraceLocation,
            24,
            8,
            vec![
                address(ArtifactId::Data(location.context)),
                DataInitializerFact::Bytes(location.line.to_le_bytes().to_vec()),
                DataInitializerFact::Bytes(location.column.to_le_bytes().to_vec()),
            ],
        )?;
    }
    if facts.runtime_trace == crate::backend::RuntimeTracePolicy::Enabled {
        let layout = facts.add_layout(LayoutFact {
            size: 8,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        })?;
        facts.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::TraceTls,
            signature: None,
            layout: Some(layout),
        });
        facts.resources.tls = Some(TlsFact {
            layout,
            initializers: vec![DataInitializerFact::Zero(8)],
        });
    }
    Ok(())
}

fn declare_roots(input: BackendInput<'_>, facts: &mut PlanFacts) {
    let complete_source = facts
        .callables
        .iter()
        .filter(|declaration| {
            declaration.body == BodyDisposition::Required
                && matches!(declaration.key, LirCallableId::Source(_))
        })
        .map(|declaration| declaration.key)
        .collect::<Vec<_>>();
    for callable in complete_source {
        facts.resources.complete_roots.insert(ArtifactRootFact {
            artifact: ArtifactId::Callable(callable),
            reason: ArtifactRootReason::CompleteDefinition,
        });
    }
    let reachable = input
        .reachable_callables()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for callable in reachable {
        facts.resources.reachable_roots.insert(ArtifactRootFact {
            artifact: ArtifactId::Callable(LirCallableId::Source(callable)),
            reason: ArtifactRootReason::CompleteDefinition,
        });
    }
    for generated in &facts.resources.generated {
        let reason = match generated.callable {
            LirCallableId::Entry => ArtifactRootReason::Entry,
            LirCallableId::Coordinator(Coordinator::Initializer) => {
                ArtifactRootReason::StaticActivation
            }
            LirCallableId::Coordinator(Coordinator::Finalizer) => {
                ArtifactRootReason::StaticShutdown
            }
            _ => ArtifactRootReason::GeneratedFamily,
        };
        facts.resources.complete_roots.insert(ArtifactRootFact {
            artifact: ArtifactId::Callable(generated.callable),
            reason,
        });
        if matches!(
            generated.callable,
            LirCallableId::Entry | LirCallableId::Coordinator(_)
        ) {
            facts.resources.reachable_roots.insert(ArtifactRootFact {
                artifact: ArtifactId::Callable(generated.callable),
                reason,
            });
        }
    }
    for storage in &facts.resources.statics {
        let root = ArtifactRootFact {
            artifact: ArtifactId::Data(DataKey::Static(storage.field)),
            reason: ArtifactRootReason::StaticStorage(storage.field),
        };
        facts.resources.complete_roots.insert(root);
        if storage.disposition == StaticStorageDisposition::Active {
            facts.resources.reachable_roots.insert(root);
        }
    }
}

fn add_data(
    facts: &mut PlanFacts,
    key: DataKey,
    purpose: DataPurpose,
    size: usize,
    alignment: usize,
    initializers: Vec<DataInitializerFact>,
) -> Result<(), PlanError> {
    let layout = facts.add_layout(LayoutFact {
        size,
        alignment,
        disposition: LayoutDisposition::Addressable,
    })?;
    facts.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(key),
        signature: None,
        layout: Some(layout),
    });
    let dependencies = initializers
        .iter()
        .filter_map(|initializer| match initializer {
            DataInitializerFact::Address { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    facts.resources.data.push(DataFact {
        key,
        purpose,
        layout,
        initializers,
        dependencies,
    });
    Ok(())
}

fn declare_generated(
    facts: &mut PlanFacts,
    callable: LirCallableId,
    signature: SignatureFact,
    attribution: GeneratedAttribution,
    dependencies: BTreeSet<ArtifactId>,
) -> Result<LirCallableId, PlanError> {
    let signature = facts.add_signature(signature)?;
    facts.callables.push(CallableDeclaration {
        key: callable,
        signature,
        body: BodyDisposition::Required,
    });
    facts.resources.generated.push(GeneratedCallableFact {
        callable,
        attribution,
        dependencies,
    });
    Ok(callable)
}

fn declare_helper(
    facts: &mut PlanFacts,
    family: HelperFamily,
    layout: LayoutId,
    signature: SignatureFact,
    dependencies: BTreeSet<ArtifactId>,
) -> Result<LirCallableId, PlanError> {
    let signature = facts.add_signature(signature)?;
    let callable = LirCallableId::Helper(HelperKey {
        family,
        layout,
        signature,
    });
    facts.callables.push(CallableDeclaration {
        key: callable,
        signature,
        body: BodyDisposition::Required,
    });
    facts.resources.generated.push(GeneratedCallableFact {
        callable,
        attribution: GeneratedAttribution::InheritedSourceOperation,
        dependencies,
    });
    Ok(callable)
}

fn add_generated_dependency(facts: &mut PlanFacts, callable: LirCallableId, target: ArtifactId) {
    facts
        .resources
        .generated
        .iter_mut()
        .find(|fact| fact.callable == callable)
        .expect("newly declared generated callable")
        .dependencies
        .insert(target);
}

fn helper_for(
    facts: &PlanFacts,
    family: HelperFamily,
    layout: LayoutId,
) -> Result<LirCallableId, PlanError> {
    facts
        .resources
        .generated
        .iter()
        .find_map(|fact| match fact.callable {
            LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                Some(fact.callable)
            }
            _ => None,
        })
        .ok_or(PlanError::UnknownDeclaration)
}

fn helper_signature(inputs: &[ScalarType], returns: ReturnShape) -> SignatureFact {
    SignatureFact {
        convention: Convention::Language,
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(index, ty)| Component {
                ty: *ty,
                role: ComponentRole::Parameter(index),
            })
            .collect(),
        results: match returns {
            ReturnShape::Scalar(ty) => vec![Component {
                ty,
                role: ComponentRole::Result,
            }],
            _ => vec![],
        },
        returns,
    }
}

fn unit_signature() -> SignatureFact {
    helper_signature(&[], ReturnShape::Unit)
}

fn callable_word(target: Option<LirCallableId>) -> DataInitializerFact {
    target.map_or(DataInitializerFact::Zero(8), |target| {
        DataInitializerFact::Address {
            target: ArtifactId::Callable(target),
            category: ArtifactCategory::Callable,
            addend: 0,
        }
    })
}

fn address(target: ArtifactId) -> DataInitializerFact {
    DataInitializerFact::Address {
        target,
        category: target.category(),
        addend: 0,
    }
}

fn layout_id(layouts: &[(MirType, LayoutId)], ty: MirType) -> Result<LayoutId, PlanError> {
    layouts
        .iter()
        .find_map(|(candidate, layout)| (*candidate == ty).then_some(*layout))
        .ok_or(PlanError::InvalidLayout)
}

fn semantic_type(ty: MirType) -> SemanticType {
    match ty {
        MirType::I64 => SemanticType::I64,
        MirType::U64 => SemanticType::U64,
        MirType::U8 => SemanticType::U8,
        MirType::F64 => SemanticType::F64,
        MirType::Bool => SemanticType::Bool,
        MirType::Function(id) => SemanticType::Function(id),
        MirType::Array(id) => SemanticType::Array(id),
        MirType::Class(id) => SemanticType::Class(id),
        MirType::Interface(id) => SemanticType::Interface(id),
        MirType::Obj => SemanticType::Obj,
        MirType::Shared(target) => SemanticType::Shared(shared_target(target)),
        MirType::Optional(id) => SemanticType::Optional(id),
        MirType::Unit => SemanticType::Unit,
    }
}

fn shared_target(target: MirSharedTarget) -> SharedTarget {
    match target {
        MirSharedTarget::Obj => SharedTarget::Obj,
        MirSharedTarget::Class(id) => SharedTarget::Class(id),
        MirSharedTarget::Interface(id) => SharedTarget::Interface(id),
        MirSharedTarget::Array(id) => SharedTarget::Array(id),
        MirSharedTarget::OptionalBox(id) => SharedTarget::OptionalBox(id),
    }
}

fn static_cleanup(cleanup: &MirStaticValueCleanup) -> StaticCleanupFact {
    match cleanup {
        MirStaticValueCleanup::None => StaticCleanupFact::None,
        MirStaticValueCleanup::CompleteObject(cleanup) => StaticCleanupFact::Class(cleanup.target),
        MirStaticValueCleanup::OptionalClass(cleanup) => {
            StaticCleanupFact::Optional(cleanup.optional)
        }
        MirStaticValueCleanup::Shared(cleanup) => {
            StaticCleanupFact::Shared(shared_target(cleanup.target))
        }
        MirStaticValueCleanup::OptionalShared(cleanup) => {
            StaticCleanupFact::Optional(cleanup.optional)
        }
        MirStaticValueCleanup::AggregateOptional(cleanup) => {
            StaticCleanupFact::Optional(cleanup.optional)
        }
        MirStaticValueCleanup::Array(cleanup) => match cleanup {
            crate::mir::MirArrayInstruction::Release { array, .. } => {
                StaticCleanupFact::Array(*array)
            }
            _ => unreachable!("verified static array cleanup is a release"),
        },
    }
}
