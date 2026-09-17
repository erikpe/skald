//! Private invariant defects; public backend presentation is preserved.
use crate::backend::graph::{GraphFailure, GraphIdentity, GraphLocation};
use crate::backend::plan::LirCallableId;
use crate::backend::{BackendError, Target};
use crate::source::Span;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum VerificationReason {
    Graph,
    UnsupportedCapability,
    SizeOverflow,
    Schema,
    GuardProtection,
    InvalidObject,
    InvalidProvenance,
    InvalidMemory,
    NarrowedEffects,
    InvalidTrace,
    InvalidReference,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VerificationFailure {
    pub identity: Box<GraphIdentity>,
    pub location: GraphLocation,
    pub reason: VerificationReason,
    pub origin: Option<Span>,
    pub graph: Option<Box<GraphFailure>>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl VerificationFailure {
    pub(super) fn new(
        identity: GraphIdentity,
        location: GraphLocation,
        reason: VerificationReason,
    ) -> Self {
        Self {
            identity: Box::new(identity),
            location,
            reason,
            origin: None,
            graph: None,
        }
    }
    pub(super) fn from_graph(f: GraphFailure) -> Self {
        Self {
            identity: f.identity.clone(),
            location: f.location,
            reason: VerificationReason::Graph,
            origin: f.origin,
            graph: Some(Box::new(f)),
        }
    }
    pub(in crate::backend) fn backend_error(&self, target: Target) -> BackendError {
        let callable = match self.identity.callable {
            LirCallableId::Source(c) => Some(c),
            _ => None,
        };
        let category = match self.reason {
            VerificationReason::UnsupportedCapability => "target capability rejection",
            VerificationReason::SizeOverflow => "size overflow",
            _ => "verification defect",
        };
        BackendError::new(
            target,
            callable,
            format!(
                "{:?} {:?} {category} in {:?} at {:?}: {:?}",
                self.identity.stage,
                self.identity.target,
                self.identity.callable,
                self.location,
                self.graph
                    .as_ref()
                    .map(|f| format!("{:?}", f.reason))
                    .unwrap_or_else(|| format!("{:?}", self.reason))
            ),
        )
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn origin(
    draft: &super::super::CallableDraft<'_>,
    location: GraphLocation,
) -> Option<Span> {
    match location {
        GraphLocation::Value(index) => draft.values().nth(index)?.1.origin,
        GraphLocation::Object(index) => draft.objects().nth(index)?.1.origin,
        GraphLocation::Instruction { block, ordinal, .. } => {
            let instruction = draft.blocks().nth(block)?.1.instructions.get(ordinal)?;
            if let super::super::Operation::Call(super::super::Call {
                attribution: super::super::CallAttribution::SourceOperation { origin, .. },
                ..
            }) = &instruction.operation
            {
                return Some(*origin);
            }
            draft
                .values
                .get_id(*instruction.results.first()?)
                .ok()?
                .origin
        }
        _ => None,
    }
}
