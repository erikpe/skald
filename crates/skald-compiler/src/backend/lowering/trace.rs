//! Shadow-frame ownership and immediately adjacent source-operation updates.
use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{
            CallAttribution, LifetimeDisposition, Object, ObjectRole, Operation, TraceAction,
            TracePlan, TraceSite,
        },
        plan::{ArtifactId, DataKey, PlanError},
        RuntimeTracePolicy,
    },
    mir::BlockId,
    source::Span,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn location(&self, origin: Span) -> Result<ArtifactId, LowerError> {
        self.admitted
            .trace()
            .requests
            .iter()
            .find(|request| {
                request.callable == self.definition.callable() && request.span == origin
            })
            .map(|request| ArtifactId::Data(request.location))
            .ok_or(PlanError::UnknownDeclaration.into())
    }
    pub(super) fn initialize_trace(&mut self) -> Result<(), LowerError> {
        if self.plan().runtime_trace() == RuntimeTracePolicy::Omitted {
            return Ok(());
        }
        let initial_location = self.location(self.definition.span())?;
        let ArtifactId::Data(DataKey::TraceLocation(index)) = initial_location else {
            return Err(PlanError::InvalidDomain.into());
        };
        let context = ArtifactId::Data(self.admitted.trace().locations[index].context);
        let layout = self
            .admitted
            .trace_record_layout()
            .ok_or(PlanError::UnknownDeclaration)?;
        let record = self.builder.declare_object(Object {
            layout: *self.plan().layout(self.plan().layout_id(layout.index())?)?,
            role: ObjectRole::TraceRecord,
            lifetime: LifetimeDisposition::WholeCallable,
            origin: Some(self.definition.span()),
        })?;
        let locations = self
            .admitted
            .trace()
            .requests
            .iter()
            .filter(|r| r.callable == self.definition.callable())
            .map(|r| ArtifactId::Data(r.location))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        self.builder.declare_trace_plan(TracePlan {
            frame_eligible: true,
            record: Some(record),
            context,
            initial_location: Some(initial_location),
            locations,
        })?;
        self.builder.append(
            self.blocks[self.definition.body().entry.index()],
            Operation::Trace(TraceAction::PushFrame { record }),
        )?;
        self.trace_record = Some(record);
        Ok(())
    }
    pub(super) fn attribution(
        &mut self,
        block: BlockId,
        origin: Span,
        terminal: bool,
    ) -> Result<CallAttribution, LowerError> {
        let location = if let Some(record) = self.trace_record {
            let location = self.location(origin)?;
            let block = self.blocks[block.index()];
            let site = if terminal {
                TraceSite::Terminator(block)
            } else {
                TraceSite::Instruction {
                    block,
                    ordinal: self.builder.instruction_count(block)? + 1,
                }
            };
            self.builder.append(
                block,
                Operation::Trace(TraceAction::ReplaceLocation {
                    record,
                    location,
                    site,
                }),
            )?;
            Some(location)
        } else {
            None
        };
        Ok(CallAttribution::SourceOperation { origin, location })
    }
    pub(super) fn pop_trace(&mut self, block: BlockId) -> Result<(), LowerError> {
        if let Some(record) = self.trace_record {
            self.builder.append(
                self.blocks[block.index()],
                Operation::Trace(TraceAction::PopFrame { record }),
            )?;
        }
        Ok(())
    }
}
