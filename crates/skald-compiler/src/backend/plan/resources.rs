//! Frozen data, generated work, static storage, and artifact-retention facts.

use std::collections::BTreeSet;

use crate::identity::{ArrayTypeId, ClassId, OptionalBoxTypeId, StaticFieldId};

use super::{ArtifactCategory, ArtifactId, DataKey, LayoutId, LirCallableId, SemanticType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum StaticStorageDisposition {
    Active,
    RetainedInactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct StaticStorageFact {
    pub field: StaticFieldId,
    pub ty: SemanticType,
    pub layout: LayoutId,
    pub disposition: StaticStorageDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum StaticActivationKind {
    ZeroDefault,
    Explicit(LirCallableId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct StaticActivationFact {
    pub field: StaticFieldId,
    pub action: StaticActivationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum StaticCleanupFact {
    None,
    Class(ClassId),
    Optional(crate::identity::OptionalTypeId),
    Shared(super::SharedTarget),
    Array(ArrayTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct StaticShutdownFact {
    pub field: StaticFieldId,
    pub cleanup: StaticCleanupFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DataInitializerFact {
    Bytes(Vec<u8>),
    Zero(usize),
    Address {
        target: ArtifactId,
        category: ArtifactCategory,
        addend: i64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DataPurpose {
    FailureMessage,
    LiteralBacking,
    StaticStorage(StaticStorageDisposition),
    ClassDispatch(ClassId),
    ArrayDescriptor(ArrayTypeId),
    OptionalBoxDescriptor(OptionalBoxTypeId),
    TraceBytes,
    TraceContext,
    TraceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct DataFact {
    pub key: DataKey,
    pub purpose: DataPurpose,
    pub layout: LayoutId,
    pub initializers: Vec<DataInitializerFact>,
    pub dependencies: BTreeSet<ArtifactId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TlsFact {
    pub layout: LayoutId,
    pub initializers: Vec<DataInitializerFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum GeneratedAttribution {
    InheritedSourceOperation,
    ProgramLifecycle,
    EntryWrapper,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct GeneratedCallableFact {
    pub callable: LirCallableId,
    pub attribution: GeneratedAttribution,
    pub dependencies: BTreeSet<ArtifactId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArtifactRootReason {
    Entry,
    CompleteDefinition,
    GeneratedFamily,
    StaticStorage(StaticFieldId),
    StaticActivation,
    StaticShutdown,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ArtifactRootFact {
    pub artifact: ArtifactId,
    pub reason: ArtifactRootReason,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ResourceFacts {
    pub statics: Vec<StaticStorageFact>,
    pub activation: Vec<StaticActivationFact>,
    pub shutdown: Vec<StaticShutdownFact>,
    pub data: Vec<DataFact>,
    pub tls: Option<TlsFact>,
    pub generated: Vec<GeneratedCallableFact>,
    pub complete_roots: BTreeSet<ArtifactRootFact>,
    pub reachable_roots: BTreeSet<ArtifactRootFact>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ResourceFacts {
    pub(in crate::backend) fn static_storage(
        &self,
        field: StaticFieldId,
    ) -> Option<StaticStorageFact> {
        self.statics
            .binary_search_by_key(&field, |fact| fact.field)
            .ok()
            .map(|index| self.statics[index])
    }

    pub(super) fn is_empty(&self) -> bool {
        self.statics.is_empty()
            && self.activation.is_empty()
            && self.shutdown.is_empty()
            && self.data.is_empty()
            && self.tls.is_none()
            && self.generated.is_empty()
            && self.complete_roots.is_empty()
            && self.reachable_roots.is_empty()
    }
}
