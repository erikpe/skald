//! Logical calls use declared signatures; ABI binding belongs to target selection.
use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Call, CallArgument, CallTarget, Operation},
        plan::{ArtifactId, LirCallableId, PlanError},
    },
    mir::{BlockId, MirArgument, MirCall, MirCallTarget, MirFunctionLinkage},
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn call(&mut self, block: BlockId, source: &MirCall) -> Result<(), LowerError> {
        let (target, signature) = match source.target {
            MirCallTarget::Direct(id) => {
                let declaration = self
                    .admitted
                    .program()
                    .declarations
                    .get(id)
                    .ok_or(PlanError::UnknownDeclaration)?;
                let target = match declaration.linkage {
                    MirFunctionLinkage::External { link } => ArtifactId::External(link),
                    _ => ArtifactId::Callable(LirCallableId::Source(id.into())),
                };
                let signature = self
                    .plan()
                    .artifact(self.plan().artifact_id(target)?, target.category())?
                    .signature
                    .ok_or(PlanError::InvalidSignature)?;
                (CallTarget::Direct(target), signature)
            }
            MirCallTarget::Static(id) => {
                let target = ArtifactId::Callable(LirCallableId::Source(id.into()));
                let signature = self
                    .plan()
                    .artifact(self.plan().artifact_id(target)?, target.category())?
                    .signature
                    .ok_or(PlanError::InvalidSignature)?;
                (CallTarget::Direct(target), signature)
            }
            MirCallTarget::Indirect(crate::mir::MirIndirectCallTarget {
                callee,
                function_type,
            }) => (
                CallTarget::Indirect(self.values[callee.index()]),
                self.admitted
                    .function_type(function_type)
                    .ok_or(PlanError::UnknownDeclaration)?,
            ),
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        let components = &self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs;
        if source.arguments.len() != components.len() {
            return Err(PlanError::InvalidSignature.into());
        }
        let arguments = source
            .arguments
            .iter()
            .zip(components)
            .map(|(argument, component)| {
                let MirArgument::Value(value) = argument else {
                    return Err(PlanError::InvalidDomain);
                };
                Ok(CallArgument {
                    role: component.role,
                    value: self.values[value.index()],
                })
            })
            .collect::<Result<Vec<_>, PlanError>>()?;
        let attribution = self.attribution(block, source.span, false)?;
        let results = source
            .result
            .iter()
            .map(|value| self.values[value.index()])
            .collect::<Vec<_>>();
        self.builder.append_into(
            self.blocks[block.index()],
            Operation::Call(Call {
                target,
                signature,
                arguments,
                attribution,
            }),
            &results,
        )?;
        Ok(())
    }
}
