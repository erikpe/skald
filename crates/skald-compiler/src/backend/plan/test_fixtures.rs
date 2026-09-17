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
