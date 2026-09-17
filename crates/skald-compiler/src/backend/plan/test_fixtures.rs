//! Shared, checked declaration fixtures for private low-level owners.

use std::collections::BTreeSet;

use super::*;
use crate::{backend::RuntimeTracePolicy, identity::FunctionId};

pub(in crate::backend) fn source(index: usize) -> LirCallableId {
    LirCallableId::Source(FunctionId::new(index).into())
}

pub(in crate::backend) fn facts() -> PlanFacts {
    PlanFacts {
        profile: TargetProfile {
            architecture: Architecture::X86_64,
            abi: Abi::SysV,
            data_layout: DataLayout {
                pointer_bytes: 8,
                pointer_alignment: 8,
                endianness: Endianness::Little,
            },
            capabilities: Capabilities {
                binary64: true,
                indirect_calls: true,
                runtime_trace: true,
            },
        },
        runtime_trace: RuntimeTracePolicy::Omitted,
        artifact_policy: ArtifactPolicy::Complete,
        layouts: vec![LayoutFact {
            size: 8,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        }],
        signatures: vec![SignatureFact {
            convention: Convention::Language,
            inputs: vec![],
            results: vec![],
            returns: ReturnShape::Unit,
        }],
        callables: (0..2)
            .map(|index| CallableDeclaration {
                key: source(index),
                signature: SignatureId::new(0),
                body: BodyDisposition::Required,
            })
            .collect(),
        artifacts: vec![],
        executable_sources: (0..2).map(|index| FunctionId::new(index).into()).collect(),
        active_statics: BTreeSet::new(),
        dispatch: vec![],
    }
}

/// Runtime header shapes as an independent fixture table; checking and effect
/// classification stay in the production service-contract owner.
pub(in crate::backend) fn runtime_declarations(
    facts: &mut PlanFacts,
) -> std::collections::BTreeMap<RuntimeService, SignatureId> {
    use ScalarType::*;
    let cases: [(RuntimeService, &[ScalarType], ReturnShape); 9] = [
        (
            RuntimeService::Allocate,
            &[U64],
            ReturnShape::Scalar(DataAddress),
        ),
        (RuntimeService::Free, &[DataAddress], ReturnShape::Unit),
        (
            RuntimeService::Panic,
            &[DataAddress, U64],
            ReturnShape::Never,
        ),
        (
            RuntimeService::IoStandardHandle,
            &[U8],
            ReturnShape::Scalar(I64),
        ),
        (
            RuntimeService::IoOpen,
            &[DataAddress, U64, U8],
            ReturnShape::Scalar(I64),
        ),
        (
            RuntimeService::IoRead,
            &[I64, DataAddress, U64],
            ReturnShape::Scalar(I64),
        ),
        (
            RuntimeService::IoWrite,
            &[I64, DataAddress, U64],
            ReturnShape::Scalar(I64),
        ),
        (RuntimeService::IoClose, &[I64], ReturnShape::Scalar(I64)),
        (RuntimeService::AbiMarker, &[], ReturnShape::Unit),
    ];
    cases
        .into_iter()
        .map(|(service, inputs, returns)| {
            let signature = facts
                .add_signature(SignatureFact {
                    convention: Convention::Runtime,
                    inputs: inputs
                        .iter()
                        .enumerate()
                        .map(|(index, ty)| Component {
                            ty: *ty,
                            role: ComponentRole::RuntimeParameter(index),
                        })
                        .collect(),
                    results: match returns {
                        ReturnShape::Scalar(ty) => vec![Component {
                            ty,
                            role: ComponentRole::Result,
                        }],
                        _ => vec![],
                    },
                    returns,
                })
                .unwrap();
            facts.artifacts.push(ArtifactDeclaration {
                key: ArtifactId::Runtime(service),
                signature: Some(signature),
                layout: None,
            });
            (service, signature)
        })
        .collect()
}
