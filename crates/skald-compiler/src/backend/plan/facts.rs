//! Supplied execution facts, with no registers, instructions or frame offsets.

use std::collections::BTreeSet;

use crate::identity::{CallableId, StaticFieldId, VirtualFamilyId};

use super::{ArtifactId, LayoutId, LirCallableId, PlanError, SignatureId};
use crate::backend::RuntimeTracePolicy;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Architecture {
    X86_64,
    Aarch64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Abi {
    SysV,
    Aapcs64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Endianness {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct DataLayout {
    pub pointer_bytes: usize,
    pub pointer_alignment: usize,
    pub endianness: Endianness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Capabilities {
    pub binary64: bool,
    pub indirect_calls: bool,
    pub runtime_trace: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TargetProfile {
    pub architecture: Architecture,
    pub abi: Abi,
    pub data_layout: DataLayout,
    pub capabilities: Capabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArtifactPolicy {
    Complete,
    Reachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ScalarType {
    I64,
    U64,
    U8,
    Bool,
    F64,
    DataAddress,
    CodeAddress(SignatureId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum LayoutDisposition {
    Addressable,
    ElidedUnit,
    ElidedMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct LayoutFact {
    pub size: usize,
    pub alignment: usize,
    pub disposition: LayoutDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Convention {
    Language,
    Runtime,
    ExternC,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ComponentRole {
    Parameter(usize),
    AggregateAddress { parameter: usize, layout: LayoutId },
    ResultDestination(LayoutId),
    ReceiverStatic,
    ReceiverComplete,
    ReceiverMetadata,
    AliasAddress(usize),
    AliasComplete(usize),
    AliasMetadata(usize),
    RuntimeParameter(usize),
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Component {
    pub ty: ScalarType,
    pub role: ComponentRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ReturnShape {
    Unit,
    Scalar(ScalarType),
    Aggregate(LayoutId),
    Never,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SignatureFact {
    pub convention: Convention,
    pub inputs: Vec<Component>,
    pub results: Vec<Component>,
    pub returns: ReturnShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum BodyDisposition {
    Required,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct CallableDeclaration {
    pub key: LirCallableId,
    pub signature: SignatureId,
    pub body: BodyDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ArtifactDeclaration {
    pub key: ArtifactId,
    pub signature: Option<SignatureId>,
    pub layout: Option<LayoutId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct DispatchSlot {
    pub family: VirtualFamilyId,
    pub index: usize,
    /// None represents an upstream-verified unused slot; not inferred here.
    pub target: Option<LirCallableId>,
}

/// Declaration indices name entries in these supplied pools. They acquire
/// lookup authority only through a checked, borrowed plan view.
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct PlanFacts {
    pub profile: TargetProfile,
    pub runtime_trace: RuntimeTracePolicy,
    pub artifact_policy: ArtifactPolicy,
    pub layouts: Vec<LayoutFact>,
    pub signatures: Vec<SignatureFact>,
    pub callables: Vec<CallableDeclaration>,
    pub artifacts: Vec<ArtifactDeclaration>,
    pub executable_sources: BTreeSet<CallableId>,
    pub active_statics: BTreeSet<StaticFieldId>,
    pub dispatch: Vec<DispatchSlot>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl PlanFacts {
    /// Allocate in explicit declaration order; checking happens at freeze.
    pub(in crate::backend) fn add_layout(
        &mut self,
        fact: LayoutFact,
    ) -> Result<LayoutId, PlanError> {
        append_declaration(&mut self.layouts, fact, LayoutId::new)
    }

    pub(in crate::backend) fn add_signature(
        &mut self,
        fact: SignatureFact,
    ) -> Result<SignatureId, PlanError> {
        append_declaration(&mut self.signatures, fact, SignatureId::new)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn append_declaration<T, I>(
    entries: &mut Vec<T>,
    fact: T,
    id: impl FnOnce(usize) -> I,
) -> Result<I, PlanError> {
    let index = entries.len();
    index.checked_add(1).ok_or(PlanError::SizeOverflow)?;
    entries.push(fact);
    Ok(id(index))
}
