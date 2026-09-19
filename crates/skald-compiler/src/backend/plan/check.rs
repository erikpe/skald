//! Structural validation of immutable declarations, not semantic reanalysis.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Abi, Architecture, ArtifactDeclaration, ArtifactId, ArtifactPolicy, BodyDisposition,
    CallableDeclaration, ComponentRole, Convention, DataKey, DispatchSlot, Endianness,
    LayoutDisposition, LayoutFact, LayoutId, LirCallableId, PlanFacts, PlanView, ReturnShape,
    ScalarType, SemanticFacts, SignatureFact, SignatureId, TargetProfile,
};
use crate::backend::RuntimeTracePolicy;
use crate::identity::StaticFieldId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum PlanError {
    InvalidProfile,
    UnsupportedCapability,
    InvalidLayout,
    SizeOverflow,
    OutOfBounds,
    InvalidSignature,
    DuplicateDeclaration,
    UnknownDeclaration,
    ArtifactCategoryMismatch,
    InvalidArtifact,
    OmittedTrace,
    AbsentBody,
    InvalidDomain,
    InvalidDispatch,
    WrongContext,
    WrongTarget,
    WrongOwner,
}

/// The fields are private; consumers receive immutable narrow views.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct CheckedPlan {
    pub(super) profile: TargetProfile,
    pub(super) runtime_trace: RuntimeTracePolicy,
    pub(super) artifact_policy: ArtifactPolicy,
    pub(super) layouts: Vec<LayoutFact>,
    pub(super) signatures: Vec<SignatureFact>,
    pub(super) active_statics: BTreeSet<StaticFieldId>,
    pub(super) dispatch: Vec<DispatchSlot>,
    pub(super) callables: BTreeMap<LirCallableId, CallableDeclaration>,
    pub(super) artifacts: BTreeMap<ArtifactId, ArtifactDeclaration>,
    pub(super) semantic: SemanticFacts,
    pub(super) resources: super::ResourceFacts,
}

#[cfg_attr(not(test), allow(dead_code))]
impl CheckedPlan {
    pub(in crate::backend) fn check(mut facts: PlanFacts) -> Result<Self, PlanError> {
        check_profile(&facts)?;
        for layout in &facts.layouts {
            check_layout(*layout)?;
        }
        for signature in &facts.signatures {
            check_signature(&facts, signature)?;
        }
        let mut callables = BTreeMap::new();
        let mut artifacts = BTreeMap::new();
        for declaration in &facts.callables {
            signature(&facts, declaration.signature)?;
            match declaration.key {
                LirCallableId::Source(source) => {
                    if facts.executable_sources.contains(&source)
                        != (declaration.body == BodyDisposition::Required)
                    {
                        return Err(PlanError::InvalidDomain);
                    }
                }
                LirCallableId::Helper(key) => {
                    addressable_layout(&facts, key.layout)?;
                    if key.signature != declaration.signature
                        || declaration.body != BodyDisposition::Required
                    {
                        return Err(PlanError::InvalidDomain);
                    }
                }
                LirCallableId::TargetThunk(_) => return Err(PlanError::InvalidDomain),
                _ if declaration.body != BodyDisposition::Required => {
                    return Err(PlanError::InvalidDomain)
                }
                _ => {}
            }
            if callables.insert(declaration.key, *declaration).is_some() {
                return Err(PlanError::DuplicateDeclaration);
            }
            artifacts.insert(
                ArtifactId::Callable(declaration.key),
                ArtifactDeclaration {
                    key: ArtifactId::Callable(declaration.key),
                    signature: Some(declaration.signature),
                    layout: None,
                },
            );
        }
        for source in &facts.executable_sources {
            if !callables.contains_key(&LirCallableId::Source(*source)) {
                return Err(PlanError::InvalidDomain);
            }
        }
        for declaration in &facts.artifacts {
            check_artifact(&facts, declaration)?;
            if artifacts.insert(declaration.key, *declaration).is_some() {
                return Err(PlanError::DuplicateDeclaration);
            }
        }
        for field in &facts.active_statics {
            if !artifacts.contains_key(&ArtifactId::Data(DataKey::Static(*field))) {
                return Err(PlanError::InvalidDomain);
            }
        }
        super::semantic_check::check(&facts)?;
        super::resource_check::check(&facts, &callables, &artifacts)?;
        facts.dispatch.sort_by_key(|slot| (slot.family, slot.index));
        let mut previous = None;
        for slot in &facts.dispatch {
            if previous == Some((slot.family, slot.index)) {
                return Err(PlanError::DuplicateDeclaration);
            }
            let expected = previous
                .filter(|(family, _)| *family == slot.family)
                .map(|(_, index): (_, usize)| index.checked_add(1).ok_or(PlanError::SizeOverflow))
                .transpose()?
                .unwrap_or(0);
            if slot.index != expected {
                return Err(PlanError::InvalidDispatch);
            }
            if let Some(target) = slot.target {
                if callables.get(&target).map(|d| d.body) != Some(BodyDisposition::Required) {
                    return Err(PlanError::AbsentBody);
                }
            }
            previous = Some((slot.family, slot.index));
        }
        // Dense declaration pools retain supplied index order; keyed catalogs
        // have one canonical iteration order regardless of request arrival.
        Ok(Self {
            profile: facts.profile,
            runtime_trace: facts.runtime_trace,
            artifact_policy: facts.artifact_policy,
            layouts: facts.layouts,
            signatures: facts.signatures,
            active_statics: facts.active_statics,
            dispatch: facts.dispatch,
            callables,
            artifacts,
            semantic: facts.semantic,
            resources: facts.resources,
        })
    }

    pub(in crate::backend) fn view(&self) -> PlanView<'_> {
        PlanView::new(self)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn check_profile(facts: &PlanFacts) -> Result<(), PlanError> {
    let p = facts.profile;
    if !matches!(
        (p.architecture, p.abi),
        (Architecture::X86_64, Abi::SysV) | (Architecture::Aarch64, Abi::Aapcs64)
    ) || p.data_layout.pointer_bytes != 8
        || p.data_layout.pointer_alignment != 8
        || (p.architecture == Architecture::X86_64
            && p.data_layout.endianness != Endianness::Little)
    {
        return Err(PlanError::InvalidProfile);
    }
    if facts.runtime_trace == RuntimeTracePolicy::Enabled && !p.capabilities.runtime_trace {
        return Err(PlanError::UnsupportedCapability);
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn check_layout(layout: LayoutFact) -> Result<(), PlanError> {
    if !layout.alignment.is_power_of_two()
        || (layout.disposition != LayoutDisposition::Addressable && layout.size != 0)
    {
        return Err(PlanError::InvalidLayout);
    }
    layout
        .size
        .checked_add(layout.alignment - 1)
        .ok_or(PlanError::SizeOverflow)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn signature(facts: &PlanFacts, id: SignatureId) -> Result<&SignatureFact, PlanError> {
    facts
        .signatures
        .get(id.index())
        .ok_or(PlanError::UnknownDeclaration)
}

#[cfg_attr(not(test), allow(dead_code))]
fn addressable_layout(facts: &PlanFacts, id: LayoutId) -> Result<&LayoutFact, PlanError> {
    let layout = facts
        .layouts
        .get(id.index())
        .ok_or(PlanError::UnknownDeclaration)?;
    if layout.disposition != LayoutDisposition::Addressable {
        return Err(PlanError::InvalidLayout);
    }
    Ok(layout)
}

#[cfg_attr(not(test), allow(dead_code))]
fn scalar(facts: &PlanFacts, ty: ScalarType) -> Result<(), PlanError> {
    match ty {
        ScalarType::F64 if !facts.profile.capabilities.binary64 => {
            return Err(PlanError::UnsupportedCapability)
        }
        ScalarType::CodeAddress(id) => {
            if !facts.profile.capabilities.indirect_calls {
                return Err(PlanError::UnsupportedCapability);
            }
            signature(facts, id)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn check_signature(facts: &PlanFacts, s: &SignatureFact) -> Result<(), PlanError> {
    let mut roles = BTreeSet::new();
    let mut parameters = BTreeSet::new();
    let mut alias_parts = BTreeMap::<usize, (bool, bool, bool)>::new();
    let mut receivers = (false, false, false);
    let mut destination = None;
    for component in &s.inputs {
        scalar(facts, component.ty)?;
        if !roles.insert(component.role) {
            return Err(PlanError::InvalidSignature);
        }
        let address = match component.role {
            ComponentRole::Parameter(index) => {
                if !parameters.insert(index) {
                    return Err(PlanError::InvalidSignature);
                }
                false
            }
            ComponentRole::AggregateAddress { parameter, layout } => {
                addressable_layout(facts, layout)?;
                if !parameters.insert(parameter) {
                    return Err(PlanError::InvalidSignature);
                }
                true
            }
            ComponentRole::ResultDestination(layout) => {
                addressable_layout(facts, layout)?;
                if destination.replace(layout).is_some() {
                    return Err(PlanError::InvalidSignature);
                }
                true
            }
            ComponentRole::ReceiverStatic => {
                receivers.0 = true;
                true
            }
            ComponentRole::ReceiverComplete => {
                receivers.1 = true;
                true
            }
            ComponentRole::ReceiverMetadata => {
                receivers.2 = true;
                true
            }
            ComponentRole::AliasAddress(index) => {
                if !parameters.insert(index) {
                    return Err(PlanError::InvalidSignature);
                }
                alias_parts.entry(index).or_default().0 = true;
                true
            }
            ComponentRole::AliasComplete(index) => {
                alias_parts.entry(index).or_default().1 = true;
                true
            }
            ComponentRole::AliasMetadata(index) => {
                alias_parts.entry(index).or_default().2 = true;
                true
            }
            ComponentRole::RuntimeParameter(_) => {
                if s.convention != Convention::Runtime {
                    return Err(PlanError::InvalidSignature);
                }
                false
            }
            ComponentRole::Result => return Err(PlanError::InvalidSignature),
        };
        if address && component.ty != ScalarType::DataAddress {
            return Err(PlanError::InvalidSignature);
        }
    }
    if receivers != (false, false, false) && receivers != (true, true, true) {
        return Err(PlanError::InvalidSignature);
    }
    if alias_parts
        .values()
        .any(|parts| !parts.0 || parts.1 != parts.2)
    {
        return Err(PlanError::InvalidSignature);
    }
    for component in &s.results {
        scalar(facts, component.ty)?;
        if component.role != ComponentRole::Result {
            return Err(PlanError::InvalidSignature);
        }
    }
    let valid = match s.returns {
        ReturnShape::Unit | ReturnShape::Never => s.results.is_empty() && destination.is_none(),
        ReturnShape::Scalar(ty) => {
            scalar(facts, ty)?;
            destination.is_none() && s.results.len() == 1 && s.results[0].ty == ty
        }
        ReturnShape::Aggregate(layout) => s.results.is_empty() && destination == Some(layout),
    };
    if !valid {
        return Err(PlanError::InvalidSignature);
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn check_artifact(facts: &PlanFacts, d: &ArtifactDeclaration) -> Result<(), PlanError> {
    if d.key.is_trace() && facts.runtime_trace == RuntimeTracePolicy::Omitted {
        return Err(PlanError::OmittedTrace);
    }
    match d.key {
        ArtifactId::Runtime(_) | ArtifactId::External(_) => {
            let s = signature(facts, d.signature.ok_or(PlanError::InvalidArtifact)?)?;
            let expected = if matches!(d.key, ArtifactId::Runtime(_)) {
                Convention::Runtime
            } else {
                Convention::ExternC
            };
            if let ArtifactId::Runtime(service) = d.key {
                super::services::check_service(service, s)?;
            }
            if s.convention != expected {
                return Err(PlanError::InvalidArtifact);
            }
            if d.layout.is_some() {
                return Err(PlanError::InvalidArtifact);
            }
        }
        ArtifactId::Data(_) | ArtifactId::TraceTls => {
            let layout = addressable_layout(facts, d.layout.ok_or(PlanError::InvalidArtifact)?)?;
            if let ArtifactId::Data(DataKey::FailureMessage(message)) = d.key {
                if layout.size != message.bytes().len() || layout.alignment != 1 {
                    return Err(PlanError::InvalidArtifact);
                }
            }
            if d.signature.is_some() {
                return Err(PlanError::InvalidArtifact);
            }
            if let ArtifactId::Data(DataKey::Static(field)) = d.key {
                if facts.artifact_policy == ArtifactPolicy::Reachable
                    && !facts.active_statics.contains(&field)
                {
                    return Err(PlanError::InvalidDomain);
                }
            }
        }
        ArtifactId::Callable(_) => return Err(PlanError::InvalidArtifact),
    }
    Ok(())
}
