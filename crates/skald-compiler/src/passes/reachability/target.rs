//! Deterministic closed-world call and callable-address target resolution.

use crate::{
    identity::{CallableId, FunctionTypeId},
    mir::{MirCallTarget, MirExecutionNode, MirFunctionLinkage, MirMethodCallTarget, MirProgram},
};

use super::{
    MirDependencyEdgeKind, MirDependencyExtractionError, MirDependencyTarget, MirRuntimeEntity,
};

pub(super) enum MirResolvedCallTarget {
    Dependencies(Vec<(MirDependencyTarget, MirDependencyEdgeKind)>),
    Indirect(FunctionTypeId),
}

pub(super) struct MirTargetResolver<'mir> {
    program: &'mir MirProgram,
}

impl<'mir> MirTargetResolver<'mir> {
    pub(super) const fn new(program: &'mir MirProgram) -> Self {
        Self { program }
    }

    pub(super) fn resolve_call(
        &self,
        target: MirCallTarget,
    ) -> Result<MirResolvedCallTarget, MirDependencyExtractionError> {
        Ok(match target {
            MirCallTarget::Direct(function) => {
                let declaration = self
                    .program
                    .declarations
                    .get(function)
                    .ok_or(MirDependencyExtractionError::UnknownFunction(function))?;
                let target = match declaration.linkage {
                    MirFunctionLinkage::Internal => {
                        MirDependencyTarget::Execution(MirExecutionNode::callable(function.into()))
                    }
                    MirFunctionLinkage::External { link } => MirDependencyTarget::External(link),
                    MirFunctionLinkage::Intrinsic { intrinsic } => {
                        MirDependencyTarget::Intrinsic(intrinsic)
                    }
                };
                MirResolvedCallTarget::Dependencies(vec![(
                    target,
                    MirDependencyEdgeKind::DirectCall,
                )])
            }
            MirCallTarget::Static(method) => {
                self.require_method(method)?;
                MirResolvedCallTarget::Dependencies(vec![(
                    MirDependencyTarget::Execution(MirExecutionNode::callable(method.into())),
                    MirDependencyEdgeKind::StaticCall,
                )])
            }
            MirCallTarget::Method(MirMethodCallTarget::Direct(method)) => {
                self.require_method(method)?;
                MirResolvedCallTarget::Dependencies(vec![(
                    MirDependencyTarget::Execution(MirExecutionNode::callable(method.into())),
                    MirDependencyEdgeKind::DirectMethodCall,
                )])
            }
            MirCallTarget::Method(MirMethodCallTarget::Virtual { family, .. }) => {
                let family = self
                    .program
                    .virtual_family(family)
                    .ok_or(MirDependencyExtractionError::UnknownVirtualFamily(family))?;
                let mut targets = family
                    .members
                    .iter()
                    .copied()
                    .map(|method| {
                        self.require_method(method)?;
                        Ok((
                            MirDependencyTarget::Execution(MirExecutionNode::callable(
                                method.into(),
                            )),
                            MirDependencyEdgeKind::VirtualDispatch,
                        ))
                    })
                    .collect::<Result<Vec<_>, MirDependencyExtractionError>>()?;
                targets.sort_unstable();
                targets.dedup();
                targets.push((
                    MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::VirtualFamily(family.id)),
                    MirDependencyEdgeKind::RuntimeEntityReference,
                ));
                MirResolvedCallTarget::Dependencies(targets)
            }
            MirCallTarget::Interface(target) => {
                if target.requirement.interface() != target.interface
                    || self
                        .program
                        .interface_requirement(target.requirement)
                        .is_none()
                {
                    return Err(MirDependencyExtractionError::UnknownInterfaceRequirement(
                        target.requirement,
                    ));
                }
                let mut targets = Vec::new();
                for class in self.program.classes.iter() {
                    let Some(conformance) = self.program.conformance(class.id, target.interface)
                    else {
                        continue;
                    };
                    for implementation in &conformance.implementations {
                        if implementation.requirement == target.requirement {
                            self.require_method(implementation.method)?;
                            targets.push((
                                MirDependencyTarget::Execution(MirExecutionNode::callable(
                                    implementation.method.into(),
                                )),
                                MirDependencyEdgeKind::InterfaceDispatch,
                            ));
                        }
                    }
                }
                targets.sort_unstable();
                targets.dedup();
                targets.push((
                    MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::InterfaceRequirement(
                        target.requirement,
                    )),
                    MirDependencyEdgeKind::RuntimeEntityReference,
                ));
                MirResolvedCallTarget::Dependencies(targets)
            }
            MirCallTarget::Indirect(target) => {
                self.require_function_type(target.function_type)?;
                MirResolvedCallTarget::Indirect(target.function_type)
            }
        })
    }

    pub(super) fn validate_callable_address(
        &self,
        callable: CallableId,
        function_type: FunctionTypeId,
    ) -> Result<(), MirDependencyExtractionError> {
        let function_type = self.require_function_type(function_type)?;
        let signature = self
            .program
            .callable_signature(callable)
            .ok_or(MirDependencyExtractionError::UnknownCallable(callable))?;
        if signature.parameters != function_type.parameters.as_slice()
            || signature.return_type != function_type.result
        {
            return Err(MirDependencyExtractionError::CallableFunctionTypeMismatch {
                callable,
                function_type: function_type.id,
            });
        }
        if let CallableId::Function(function) = callable {
            let linkage = self
                .program
                .declarations
                .get(function)
                .ok_or(MirDependencyExtractionError::UnknownFunction(function))?
                .linkage;
            if linkage != MirFunctionLinkage::Internal {
                return Err(MirDependencyExtractionError::NonInternalCallableAddress(
                    callable,
                ));
            }
        }
        Ok(())
    }

    fn require_method(
        &self,
        method: crate::identity::MethodId,
    ) -> Result<(), MirDependencyExtractionError> {
        self.program
            .method(method)
            .map(|_| ())
            .ok_or(MirDependencyExtractionError::UnknownMethod(method))
    }

    fn require_function_type(
        &self,
        function_type: FunctionTypeId,
    ) -> Result<&'mir crate::mir::MirFunctionType, MirDependencyExtractionError> {
        self.program.function_type(function_type).ok_or(
            MirDependencyExtractionError::UnknownFunctionType(function_type),
        )
    }
}
