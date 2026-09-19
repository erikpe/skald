//! Compact declaration keys; authority comes from their borrowed context.

use crate::identity::{
    ArrayTypeId, CallableId, ClassId, ExternalLinkId, LiteralDataId, OptionalBoxTypeId,
    StaticFieldId,
};

macro_rules! declaration_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(not(test), allow(dead_code))]
        pub(in crate::backend) struct $name(usize);

        #[cfg_attr(not(test), allow(dead_code))]
        impl $name {
            pub(super) const fn new(index: usize) -> Self {
                Self(index)
            }
            pub(in crate::backend) const fn index(self) -> usize {
                self.0
            }
        }
    };
}

declaration_id!(LayoutId);
declaration_id!(SignatureId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum HelperFamily {
    ArrayElementInitializer,
    ArrayElementCopier,
    ArrayClone,
    ArrayElementDestroyer,
    ArrayRelease,
    ArraySharedFinalizer,
    RawClassCopy,
    Retain,
    Release,
    ClassFinalizer,
    OptionalBoxFinalizer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct HelperKey {
    pub family: HelperFamily,
    pub layout: LayoutId,
    pub signature: SignatureId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Coordinator {
    Initializer,
    Finalizer,
}

/// Opaque target-owned specialization, not a shared enum of target recipes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TargetThunkKey {
    pub family: usize,
    pub specialization: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum LirCallableId {
    Source(CallableId),
    Helper(HelperKey),
    Coordinator(Coordinator),
    Entry,
    TargetThunk(TargetThunkKey),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum RuntimeService {
    Allocate,
    Free,
    Panic,
    IoStandardHandle,
    IoOpen,
    IoRead,
    IoWrite,
    IoClose,
    AbiMarker,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DataKey {
    FailureMessage(crate::backend::failure::FailureMessage),
    Table(usize),
    Literal(LiteralDataId),
    ClassDispatch(ClassId),
    ArrayDescriptor(ArrayTypeId),
    OptionalBoxDescriptor(OptionalBoxTypeId),
    Static(StaticFieldId),
    TraceBytes(usize),
    TraceContext(usize),
    TraceLocation(usize),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArtifactId {
    Callable(LirCallableId),
    Data(DataKey),
    Runtime(RuntimeService),
    External(ExternalLinkId),
    TraceTls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArtifactCategory {
    Callable,
    Data,
    Runtime,
    External,
    Tls,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ArtifactId {
    pub(in crate::backend) const fn category(self) -> ArtifactCategory {
        match self {
            Self::Callable(_) => ArtifactCategory::Callable,
            Self::Data(_) => ArtifactCategory::Data,
            Self::Runtime(_) => ArtifactCategory::Runtime,
            Self::External(_) => ArtifactCategory::External,
            Self::TraceTls => ArtifactCategory::Tls,
        }
    }

    pub(super) const fn is_trace(self) -> bool {
        matches!(
            self,
            Self::TraceTls
                | Self::Data(
                    DataKey::TraceBytes(_) | DataKey::TraceContext(_) | DataKey::TraceLocation(_)
                )
        )
    }
}
