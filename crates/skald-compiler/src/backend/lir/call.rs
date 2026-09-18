//! Logical calls and attribution; no physical ABI binding or executable MIR.

use super::{BuildError, DraftChecks, ValueHandle};
use crate::backend::effects::{Effect, Effects};
use crate::backend::graph::LoweredObjectId;
use crate::backend::graph::LoweredValueId;
use crate::backend::plan::{service_effects, ArtifactCategory, ReturnShape, ScalarType};
use crate::backend::plan::{ArtifactId, ComponentRole, LirCallableId, SignatureId};
use crate::source::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) enum CallTarget<V = LoweredValueId> {
    Direct(ArtifactId),
    Indirect(V),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct CallArgument<V = LoweredValueId> {
    pub role: ComponentRole,
    pub value: V,
}
/// Attribution never establishes purity or emits a trace update implicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum CallAttribution {
    SourceOperation {
        origin: Span,
        location: Option<ArtifactId>,
    },
    InheritedOperation {
        boundary: LirCallableId,
    },
    SourceBodyFromOmittedHelper {
        boundary: LirCallableId,
    },
    NonReporting,
    HardDefectOnly,
    ProcessBoundary,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct Call<V = LoweredValueId> {
    pub target: CallTarget<V>,
    pub signature: SignatureId,
    pub arguments: Vec<CallArgument<V>>,
    pub attribution: CallAttribution,
}

impl<'p> DraftChecks<'_, 'p> {
    pub(super) fn normalize_call(
        &self,
        call: Call<ValueHandle<'p>>,
        terminal: bool,
    ) -> Result<(Call, Vec<ScalarType>), BuildError> {
        let view = self.draft.owner.context();
        let signature = view.signature(view.signature_id(call.signature.index())?)?;
        if matches!(signature.returns, ReturnShape::Never) != terminal
            || signature.inputs.len() != call.arguments.len()
        {
            return Err(BuildError::InvalidCall);
        }
        let arguments = call
            .arguments
            .into_iter()
            .zip(&signature.inputs)
            .map(|(argument, component)| {
                let (value, ty) = self.value(argument.value)?;
                if argument.role != component.role || ty != component.ty {
                    return Err(BuildError::InvalidCall);
                }
                Ok(CallArgument {
                    role: argument.role,
                    value,
                })
            })
            .collect::<Result<Vec<_>, BuildError>>()?;
        let target = match call.target {
            CallTarget::Direct(target) => {
                if !matches!(
                    target.category(),
                    ArtifactCategory::Callable
                        | ArtifactCategory::Runtime
                        | ArtifactCategory::External
                ) {
                    return Err(BuildError::InvalidCall);
                }
                let declaration = view.artifact(view.artifact_id(target)?, target.category())?;
                if declaration.signature != Some(call.signature) {
                    return Err(BuildError::InvalidCall);
                }
                if let ArtifactId::Callable(key) = target {
                    view.callable(key)?;
                }
                CallTarget::Direct(target)
            }
            CallTarget::Indirect(value) => {
                self.check_type(ScalarType::CodeAddress(call.signature))?;
                let (value, ty) = self.value(value)?;
                if ty != ScalarType::CodeAddress(call.signature) {
                    return Err(BuildError::InvalidCall);
                }
                CallTarget::Indirect(value)
            }
        };
        let call = Call {
            target,
            signature: call.signature,
            arguments,
            attribution: call.attribution,
        };
        let required = self.call_effects(&call)?;
        match &call.attribution {
            CallAttribution::SourceOperation { location, .. } => {
                if let Some(location) = location {
                    self.trace_location(*location)?;
                } else if self
                    .draft
                    .trace_plan
                    .as_ref()
                    .is_some_and(|plan| plan.frame_eligible)
                {
                    return Err(BuildError::InvalidTrace);
                }
            }
            CallAttribution::InheritedOperation { boundary } => {
                view.callable(*boundary)?;
            }
            CallAttribution::SourceBodyFromOmittedHelper { boundary } => {
                view.callable(*boundary)?;
                if !matches!(boundary, LirCallableId::Helper(_))
                    || !matches!(
                        call.target,
                        CallTarget::Direct(ArtifactId::Callable(LirCallableId::Source(_)))
                    )
                {
                    return Err(BuildError::InvalidCall);
                }
            }
            CallAttribution::NonReporting | CallAttribution::HardDefectOnly
                if required.contains(Effect::Report) =>
            {
                return Err(BuildError::InvalidCall)
            }
            _ => {}
        }
        Ok((
            call,
            signature
                .results
                .iter()
                .map(|component| component.ty)
                .collect(),
        ))
    }
    pub(super) fn call_effects(&self, call: &Call) -> Result<Effects<LoweredObjectId>, BuildError> {
        Ok(match call.target {
            CallTarget::Direct(target) => {
                let view = self.draft.owner.context();
                view.artifact(view.artifact_id(target)?, target.category())?;
                match target {
                    ArtifactId::Runtime(service) => service_effects(service),
                    _ => Effects::conservative_call(),
                }
            }
            CallTarget::Indirect(value) => {
                self.draft.values.get_id(value)?;
                Effects::conservative_call()
            }
        })
    }
}
