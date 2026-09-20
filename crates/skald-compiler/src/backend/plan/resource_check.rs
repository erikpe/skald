//! Independent structural checks for frozen program-resource facts.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ArtifactDeclaration, ArtifactId, ArtifactPolicy, BodyDisposition, CallableDeclaration,
    DataFact, DataInitializerFact, DataKey, DataPurpose, LayoutDisposition, LirCallableId,
    PlanError, PlanFacts, StaticActivationKind, StaticCleanupFact, StaticStorageDisposition,
};

pub(super) fn check(
    facts: &PlanFacts,
    callables: &BTreeMap<LirCallableId, CallableDeclaration>,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    if facts.resources.is_empty() {
        return Ok(());
    }

    check_statics(facts, callables, artifacts)?;
    check_data(facts, artifacts)?;
    check_literal_backings(facts, artifacts)?;
    check_generated(facts, callables, artifacts)?;
    check_roots(facts, callables, artifacts)?;
    Ok(())
}

fn check_literal_backings(
    facts: &PlanFacts,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    if !facts
        .resources
        .literal_backings
        .windows(2)
        .all(|pair| pair[0].literal < pair[1].literal)
    {
        return Err(PlanError::DuplicateDeclaration);
    }
    let canonical = facts
        .resources
        .literal_backings
        .iter()
        .filter_map(|fact| (fact.literal == fact.canonical).then_some(fact.literal))
        .collect::<BTreeSet<_>>();
    let supplied = facts
        .resources
        .data
        .iter()
        .filter_map(|data| match (data.key, data.purpose) {
            (DataKey::Literal(literal), DataPurpose::LiteralBacking) => Some(literal),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if canonical != supplied {
        return Err(PlanError::InvalidArtifact);
    }
    for backing in &facts.resources.literal_backings {
        let key = DataKey::Literal(backing.canonical);
        if !canonical.contains(&backing.canonical)
            || !artifacts.contains_key(&ArtifactId::Data(key))
        {
            return Err(PlanError::UnknownDeclaration);
        }
    }
    Ok(())
}

fn check_statics(
    facts: &PlanFacts,
    callables: &BTreeMap<LirCallableId, CallableDeclaration>,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    let mut previous = None;
    let mut active = BTreeSet::new();
    for storage in &facts.resources.statics {
        if previous >= Some(storage.field) {
            return Err(PlanError::DuplicateDeclaration);
        }
        previous = Some(storage.field);
        let binding = facts
            .semantic
            .layout(storage.ty)
            .ok_or(PlanError::InvalidDomain)?;
        if binding != storage.layout {
            return Err(PlanError::InvalidLayout);
        }
        let declaration = artifacts
            .get(&ArtifactId::Data(DataKey::Static(storage.field)))
            .ok_or(PlanError::UnknownDeclaration)?;
        if declaration.layout != Some(storage.layout) || declaration.signature.is_some() {
            return Err(PlanError::InvalidArtifact);
        }
        if storage.disposition == StaticStorageDisposition::Active {
            active.insert(storage.field);
        } else if facts.artifact_policy == ArtifactPolicy::Reachable {
            return Err(PlanError::InvalidDomain);
        }
    }
    if active != facts.active_statics {
        return Err(PlanError::InvalidDomain);
    }

    let mut activation_fields = BTreeSet::new();
    for activation in &facts.resources.activation {
        let storage = facts
            .resources
            .static_storage(activation.field)
            .ok_or(PlanError::UnknownDeclaration)?;
        if storage.disposition != StaticStorageDisposition::Active
            || !activation_fields.insert(activation.field)
        {
            return Err(PlanError::InvalidDomain);
        }
        if let StaticActivationKind::Explicit(callable) = activation.action {
            if !matches!(
                callable,
                LirCallableId::Source(crate::identity::CallableId::StaticInitializer(_))
            ) || callables.get(&callable).map(|item| item.body)
                != Some(BodyDisposition::Required)
            {
                return Err(PlanError::AbsentBody);
            }
        }
    }
    if activation_fields != active {
        return Err(PlanError::InvalidDomain);
    }

    let shutdown_fields = facts
        .resources
        .shutdown
        .iter()
        .map(|shutdown| shutdown.field)
        .collect::<BTreeSet<_>>();
    for shutdown in &facts.resources.shutdown {
        let known_cleanup = match shutdown.cleanup {
            StaticCleanupFact::None => true,
            StaticCleanupFact::Class(class) => facts.semantic.class(class).is_some(),
            StaticCleanupFact::Optional(optional) => facts.semantic.optional(optional).is_some(),
            StaticCleanupFact::Shared(target) => facts
                .semantic
                .layout(super::SemanticType::Shared(target))
                .is_some(),
            StaticCleanupFact::Array(array) => facts.semantic.array(array).is_some(),
        };
        if !known_cleanup {
            return Err(PlanError::InvalidDomain);
        }
    }
    if shutdown_fields != active
        || facts
            .resources
            .shutdown
            .iter()
            .map(|shutdown| shutdown.field)
            .ne(facts
                .resources
                .activation
                .iter()
                .rev()
                .map(|item| item.field))
    {
        return Err(PlanError::InvalidDomain);
    }
    Ok(())
}

fn check_data(
    facts: &PlanFacts,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    let declared = artifacts
        .keys()
        .filter_map(|artifact| match artifact {
            ArtifactId::Data(key) => Some(*key),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let supplied = facts
        .resources
        .data
        .iter()
        .map(|data| data.key)
        .collect::<BTreeSet<_>>();
    if declared != supplied || supplied.len() != facts.resources.data.len() {
        return Err(PlanError::InvalidArtifact);
    }
    if !facts
        .resources
        .data
        .windows(2)
        .all(|pair| pair[0].key < pair[1].key)
    {
        return Err(PlanError::DuplicateDeclaration);
    }
    for data in &facts.resources.data {
        check_data_fact(facts, artifacts, data)?;
    }

    match (artifacts.get(&ArtifactId::TraceTls), &facts.resources.tls) {
        (None, None) => {}
        (Some(declaration), Some(tls))
            if declaration.layout == Some(tls.layout)
                && declaration.signature.is_none()
                && initializer_width(facts, artifacts, &tls.initializers)?
                    == facts
                        .layouts
                        .get(tls.layout.index())
                        .ok_or(PlanError::UnknownDeclaration)?
                        .size => {}
        _ => return Err(PlanError::InvalidArtifact),
    }
    Ok(())
}

fn check_data_fact(
    facts: &PlanFacts,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
    data: &DataFact,
) -> Result<(), PlanError> {
    let declaration = artifacts
        .get(&ArtifactId::Data(data.key))
        .ok_or(PlanError::UnknownDeclaration)?;
    if declaration.layout != Some(data.layout) || declaration.signature.is_some() {
        return Err(PlanError::InvalidArtifact);
    }
    let layout = facts
        .layouts
        .get(data.layout.index())
        .ok_or(PlanError::UnknownDeclaration)?;
    if layout.disposition != LayoutDisposition::Addressable
        || initializer_width(facts, artifacts, &data.initializers)? != layout.size
    {
        return Err(PlanError::InvalidLayout);
    }
    let dependencies = data
        .initializers
        .iter()
        .filter_map(|initializer| match initializer {
            DataInitializerFact::Address { target, .. } => Some(*target),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if dependencies != data.dependencies || !purpose_matches(data) {
        return Err(PlanError::InvalidArtifact);
    }
    let known_nominal_owner = match data.purpose {
        DataPurpose::ClassDispatch(class) => facts.semantic.class(class).is_some(),
        DataPurpose::ArrayDescriptor(array) => facts.semantic.array(array).is_some(),
        DataPurpose::OptionalBoxDescriptor(optional_box) => {
            facts.semantic.optional_box(optional_box).is_some()
        }
        _ => true,
    };
    if !known_nominal_owner {
        return Err(PlanError::InvalidDomain);
    }
    if let DataPurpose::StaticStorage(disposition) = data.purpose {
        let DataKey::Static(field) = data.key else {
            return Err(PlanError::InvalidArtifact);
        };
        if facts
            .resources
            .static_storage(field)
            .map(|item| item.disposition)
            != Some(disposition)
            || data.initializers != [DataInitializerFact::Zero(layout.size)]
            || !data.dependencies.is_empty()
        {
            return Err(PlanError::InvalidDomain);
        }
    }
    if let DataKey::FailureMessage(message) = data.key {
        if data.initializers != [DataInitializerFact::Bytes(message.bytes().to_vec())] {
            return Err(PlanError::InvalidDomain);
        }
    }
    Ok(())
}

fn initializer_width(
    facts: &PlanFacts,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
    initializers: &[DataInitializerFact],
) -> Result<usize, PlanError> {
    initializers.iter().try_fold(0usize, |bytes, initializer| {
        let width = match initializer {
            DataInitializerFact::Bytes(value) => value.len(),
            DataInitializerFact::Zero(width) => *width,
            DataInitializerFact::Address {
                target,
                category,
                addend,
            } => {
                if target.category() != *category {
                    return Err(PlanError::ArtifactCategoryMismatch);
                }
                let declaration = artifacts.get(target).ok_or(PlanError::UnknownDeclaration)?;
                let valid_addend = match declaration.layout {
                    Some(layout) => usize::try_from(*addend).is_ok_and(|offset| {
                        facts
                            .layouts
                            .get(layout.index())
                            .is_some_and(|layout| offset <= layout.size)
                    }),
                    None => *addend == 0,
                };
                if !valid_addend {
                    return Err(PlanError::OutOfBounds);
                }
                facts.profile.data_layout.pointer_bytes
            }
        };
        bytes.checked_add(width).ok_or(PlanError::SizeOverflow)
    })
}

fn purpose_matches(data: &DataFact) -> bool {
    match (data.key, data.purpose) {
        (DataKey::FailureMessage(_), DataPurpose::FailureMessage)
        | (DataKey::Literal(_), DataPurpose::LiteralBacking)
        | (DataKey::Static(_), DataPurpose::StaticStorage(_))
        | (DataKey::TraceBytes(_), DataPurpose::TraceBytes)
        | (DataKey::TraceContext(_), DataPurpose::TraceContext)
        | (DataKey::TraceLocation(_), DataPurpose::TraceLocation) => true,
        (DataKey::ClassDispatch(key), DataPurpose::ClassDispatch(purpose)) => key == purpose,
        (DataKey::ArrayDescriptor(key), DataPurpose::ArrayDescriptor(purpose)) => key == purpose,
        (DataKey::OptionalBoxDescriptor(key), DataPurpose::OptionalBoxDescriptor(purpose)) => {
            key == purpose
        }
        _ => false,
    }
}

fn check_generated(
    facts: &PlanFacts,
    callables: &BTreeMap<LirCallableId, CallableDeclaration>,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    if !facts
        .resources
        .generated
        .windows(2)
        .all(|pair| pair[0].callable < pair[1].callable)
    {
        return Err(PlanError::DuplicateDeclaration);
    }
    let declared = callables
        .keys()
        .filter(|key| !matches!(key, LirCallableId::Source(_)))
        .copied()
        .collect::<BTreeSet<_>>();
    let supplied = facts
        .resources
        .generated
        .iter()
        .map(|fact| fact.callable)
        .collect::<BTreeSet<_>>();
    if declared != supplied || supplied.len() != facts.resources.generated.len() {
        return Err(PlanError::InvalidDomain);
    }
    for generated in &facts.resources.generated {
        let valid_attribution = matches!(
            (generated.callable, generated.attribution),
            (
                LirCallableId::Entry,
                super::GeneratedAttribution::EntryWrapper
            ) | (
                LirCallableId::Coordinator(_),
                super::GeneratedAttribution::ProgramLifecycle
            ) | (
                LirCallableId::Helper(_),
                super::GeneratedAttribution::InheritedSourceOperation
            )
        );
        if !valid_attribution {
            return Err(PlanError::InvalidDomain);
        }
        for dependency in &generated.dependencies {
            if !artifacts.contains_key(dependency) {
                return Err(PlanError::UnknownDeclaration);
            }
        }
    }
    Ok(())
}

fn check_roots(
    facts: &PlanFacts,
    callables: &BTreeMap<LirCallableId, CallableDeclaration>,
    artifacts: &BTreeMap<ArtifactId, ArtifactDeclaration>,
) -> Result<(), PlanError> {
    for roots in [
        &facts.resources.complete_roots,
        &facts.resources.reachable_roots,
    ] {
        for root in roots {
            if !artifacts.contains_key(&root.artifact) {
                return Err(PlanError::UnknownDeclaration);
            }
            let valid_reason = match (root.artifact, root.reason) {
                (ArtifactId::Callable(LirCallableId::Entry), super::ArtifactRootReason::Entry) => {
                    true
                }
                (
                    ArtifactId::Callable(LirCallableId::Source(_)),
                    super::ArtifactRootReason::CompleteDefinition,
                ) => true,
                (
                    ArtifactId::Callable(LirCallableId::Helper(_)),
                    super::ArtifactRootReason::GeneratedFamily,
                ) => true,
                (
                    ArtifactId::Callable(LirCallableId::Coordinator(
                        super::Coordinator::Initializer,
                    )),
                    super::ArtifactRootReason::StaticActivation,
                ) => true,
                (
                    ArtifactId::Callable(LirCallableId::Coordinator(super::Coordinator::Finalizer)),
                    super::ArtifactRootReason::StaticShutdown,
                ) => true,
                (
                    ArtifactId::Data(DataKey::Static(artifact)),
                    super::ArtifactRootReason::StaticStorage(reason),
                ) => artifact == reason,
                _ => false,
            };
            if !valid_reason {
                return Err(PlanError::InvalidDomain);
            }
        }
    }
    let complete = facts
        .resources
        .complete_roots
        .iter()
        .map(|root| root.artifact)
        .collect::<BTreeSet<_>>();
    let reachable = facts
        .resources
        .reachable_roots
        .iter()
        .map(|root| root.artifact)
        .collect::<BTreeSet<_>>();
    if !reachable.is_subset(&complete) {
        return Err(PlanError::InvalidDomain);
    }
    if !reachable.contains(&ArtifactId::Callable(LirCallableId::Entry))
        && callables.contains_key(&LirCallableId::Entry)
    {
        return Err(PlanError::InvalidDomain);
    }
    for storage in &facts.resources.statics {
        let artifact = ArtifactId::Data(DataKey::Static(storage.field));
        if !complete.contains(&artifact)
            || (storage.disposition == StaticStorageDisposition::Active
                && !reachable.contains(&artifact))
        {
            return Err(PlanError::InvalidDomain);
        }
    }
    for (key, declaration) in callables {
        if declaration.body == BodyDisposition::Required
            && !complete.contains(&ArtifactId::Callable(*key))
        {
            return Err(PlanError::AbsentBody);
        }
    }
    for root in &facts.resources.reachable_roots {
        if let ArtifactId::Data(DataKey::Static(field)) = root.artifact {
            if facts
                .resources
                .static_storage(field)
                .map(|item| item.disposition)
                != Some(StaticStorageDisposition::Active)
            {
                return Err(PlanError::InvalidDomain);
            }
        }
    }
    Ok(())
}
