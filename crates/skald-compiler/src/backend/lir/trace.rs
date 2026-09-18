//! Explicit, owner-bound trace records and operation associations.

use super::{BlockHandle, BuildError, DraftBuilder, DraftChecks, ObjectHandle, ObjectRole};
use crate::backend::graph::{LoweredBlockId, LoweredObjectId};
use crate::backend::plan::ArtifactId;
use crate::backend::plan::{ArtifactCategory, DataKey, PlanError};
use crate::backend::RuntimeTracePolicy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct TracePlan<O = LoweredObjectId> {
    pub frame_eligible: bool,
    pub record: Option<O>,
    pub context: ArtifactId,
    pub initial_location: Option<ArtifactId>,
    pub locations: Vec<ArtifactId>,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::backend) enum TraceSite<B = LoweredBlockId> {
    Instruction { block: B, ordinal: usize },
    Terminator(B),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) enum TraceAction<O = LoweredObjectId, B = LoweredBlockId> {
    PushFrame {
        record: O,
    },
    ReplaceLocation {
        record: O,
        location: ArtifactId,
        site: TraceSite<B>,
    },
    PopFrame {
        record: O,
    },
}

impl<'p> DraftBuilder<'p> {
    pub(in crate::backend) fn declare_trace_plan(
        &mut self,
        plan: TracePlan<ObjectHandle<'p>>,
    ) -> Result<(), BuildError> {
        self.checks().trace_enabled()?;
        if self.draft.trace_plan.is_some()
            || self
                .draft
                .blocks
                .iter()
                .any(|(_, block)| !block.instructions.is_empty() || block.terminator.is_some())
        {
            return Err(BuildError::InvalidTrace);
        }
        let record = plan
            .record
            .map(|record| {
                self.draft.objects.get(record)?;
                Ok::<_, BuildError>(record.id())
            })
            .transpose()?;
        let plan = TracePlan {
            frame_eligible: plan.frame_eligible,
            record,
            context: plan.context,
            initial_location: plan.initial_location,
            locations: plan.locations,
        };
        self.checks().check_trace_plan(&plan)?;
        self.draft.trace_plan = Some(plan);
        Ok(())
    }
}

impl<'p> DraftChecks<'_, 'p> {
    pub(super) fn check_trace_plan(&self, plan: &TracePlan) -> Result<(), BuildError> {
        self.trace_enabled()?;
        if plan.frame_eligible != plan.record.is_some()
            || plan.frame_eligible != plan.initial_location.is_some()
            || plan
                .initial_location
                .is_some_and(|location| !plan.locations.contains(&location))
        {
            return Err(BuildError::InvalidTrace);
        }
        if let Some(record) = plan.record {
            let record = self.draft.objects.get_id(record)?;
            if record.role != ObjectRole::TraceRecord
                || record.layout.disposition != crate::backend::plan::LayoutDisposition::Addressable
            {
                return Err(BuildError::InvalidTrace);
            }
        }
        if !matches!(plan.context, ArtifactId::Data(DataKey::TraceContext(_))) {
            return Err(BuildError::InvalidTrace);
        }
        let view = self.draft.owner.context();
        view.artifact(view.artifact_id(plan.context)?, ArtifactCategory::Data)?;
        view.artifact(
            view.artifact_id(ArtifactId::TraceTls)?,
            ArtifactCategory::Tls,
        )?;
        let mut unique = std::collections::BTreeSet::new();
        for location in &plan.locations {
            if !matches!(location, ArtifactId::Data(DataKey::TraceLocation(_)))
                || !unique.insert(*location)
            {
                return Err(BuildError::InvalidTrace);
            }
            view.artifact(view.artifact_id(*location)?, ArtifactCategory::Data)?;
        }
        Ok(())
    }
    pub(super) fn trace_enabled(&self) -> Result<(), BuildError> {
        if self.draft.owner.context().runtime_trace() == RuntimeTracePolicy::Omitted {
            return Err(BuildError::Plan(PlanError::OmittedTrace));
        }
        Ok(())
    }
    pub(super) fn trace_location(&self, location: ArtifactId) -> Result<(), BuildError> {
        self.trace_enabled()?;
        if !self
            .draft
            .trace_plan
            .as_ref()
            .is_some_and(|plan| plan.locations.contains(&location))
        {
            return Err(BuildError::InvalidTrace);
        }
        Ok(())
    }
    pub(super) fn normalize_trace(
        &self,
        action: TraceAction<ObjectHandle<'p>, BlockHandle<'p>>,
    ) -> Result<TraceAction, BuildError> {
        self.trace_enabled()?;
        let plan = self
            .draft
            .trace_plan
            .as_ref()
            .ok_or(BuildError::InvalidTrace)?;
        let record = match &action {
            TraceAction::PushFrame { record }
            | TraceAction::PopFrame { record }
            | TraceAction::ReplaceLocation { record, .. } => *record,
        };
        self.draft.objects.get(record)?;
        if Some(record.id()) != plan.record || !plan.frame_eligible {
            return Err(BuildError::InvalidTrace);
        }
        Ok(match action {
            TraceAction::PushFrame { .. } => TraceAction::PushFrame {
                record: record.id(),
            },
            TraceAction::PopFrame { .. } => TraceAction::PopFrame {
                record: record.id(),
            },
            TraceAction::ReplaceLocation { location, site, .. } => {
                self.trace_location(location)?;
                let site = match site {
                    TraceSite::Instruction { block, ordinal } => {
                        self.draft.blocks.get(block)?;
                        ordinal.checked_add(1).ok_or(BuildError::SizeOverflow)?;
                        TraceSite::Instruction {
                            block: block.id(),
                            ordinal,
                        }
                    }
                    TraceSite::Terminator(block) => {
                        self.draft.blocks.get(block)?;
                        TraceSite::Terminator(block.id())
                    }
                };
                TraceAction::ReplaceLocation {
                    record: record.id(),
                    location,
                    site,
                }
            }
        })
    }
}
