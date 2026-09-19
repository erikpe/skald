use std::collections::BTreeMap;

use crate::backend::{
    plan::{CheckedPlan, LayoutId, PlanError, SignatureId},
    BackendError,
};
use crate::{identity::FunctionTypeId, mir::MirType};

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct UnsupportedProgram {
    pub callable: Option<crate::identity::CallableId>,
    pub reason: String,
}

#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum AdmissionError {
    Unsupported(UnsupportedProgram),
    Backend(BackendError),
    Plan(PlanError),
}

impl From<BackendError> for AdmissionError {
    fn from(error: BackendError) -> Self {
        Self::Backend(error)
    }
}
impl From<PlanError> for AdmissionError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

/// Immutable maps retain semantic identity even when physical shapes coincide.
/// Only the admission/projection owner can construct this authority.
pub(in crate::backend) struct AdmittedProgram<'input> {
    pub(super) program: &'input crate::mir::MirProgram,
    pub(super) plan: CheckedPlan,
    pub(super) layouts: Vec<(MirType, LayoutId)>,
    pub(super) trace: TraceFacts,
    pub(super) trace_record_layout: Option<LayoutId>,
    pub(super) function_types: BTreeMap<FunctionTypeId, SignatureId>,
}

impl AdmittedProgram<'_> {
    pub(in crate::backend) fn program(&self) -> &crate::mir::MirProgram {
        self.program
    }
    pub(in crate::backend) fn trace(&self) -> &TraceFacts {
        &self.trace
    }
    pub(in crate::backend) fn plan(&self) -> &CheckedPlan {
        &self.plan
    }
    pub(in crate::backend) fn layout(&self, ty: MirType) -> Option<LayoutId> {
        self.layouts
            .iter()
            .find_map(|(key, id)| (*key == ty).then_some(*id))
    }
    pub(in crate::backend) fn function_type(&self, id: FunctionTypeId) -> Option<SignatureId> {
        self.function_types.get(&id).copied()
    }
    pub(in crate::backend) fn trace_record_layout(&self) -> Option<LayoutId> {
        self.trace_record_layout
    }
}

#[derive(Default)]
pub(in crate::backend) struct TraceFacts {
    pub record_layout: Option<crate::backend::plan::LayoutFact>,
    pub strings: Vec<Vec<u8>>,
    pub contexts: Vec<TraceContext>,
    pub locations: Vec<TraceLocation>,
    pub requests: Vec<TraceRequest>,
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(
                f,
                "unsupported low-level input in {:?}: {}",
                reason.callable, reason.reason
            ),
            Self::Backend(error) => error.fmt(f),
            Self::Plan(error) => write!(f, "low-level planning invariant: {error:?}"),
        }
    }
}
impl std::error::Error for AdmissionError {}

pub(in crate::backend) struct TraceContext {
    pub name: crate::backend::plan::DataKey,
    pub path: crate::backend::plan::DataKey,
}
pub(in crate::backend) struct TraceLocation {
    pub context: crate::backend::plan::DataKey,
    pub line: u64,
    pub column: u64,
}
pub(in crate::backend) struct TraceRequest {
    pub callable: crate::identity::CallableId,
    pub span: crate::source::Span,
    pub location: crate::backend::plan::DataKey,
}
