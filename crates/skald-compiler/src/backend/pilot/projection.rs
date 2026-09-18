//! Final-MIR queries stop here. Downstream facts carry no frontend authority.

use super::{admission, AdmittedPilot, PilotError};
use super::{
    layouts::collect_types,
    signatures::{declaration_inventory, intern_source_signature, signature, unit_signature},
};
use crate::backend::{
    failure::FailureMessage, plan::*, x86_64_sysv, BackendInput, RuntimeTracePolicy,
};
use crate::mir::*;
use std::collections::BTreeMap;

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn admit(input: BackendInput<'_>) -> Result<AdmittedPilot<'_>, PilotError> {
    admission::check(input)?;
    let program = input.program();
    let mut facts = PlanFacts {
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
        runtime_trace: input.runtime_trace(),
        artifact_policy: if input.reachable_artifacts_only() {
            ArtifactPolicy::Reachable
        } else {
            ArtifactPolicy::Complete
        },
        layouts: vec![],
        signatures: vec![],
        callables: vec![],
        artifacts: vec![],
        executable_sources: program
            .executable_definitions()
            .map(|d| d.callable())
            .collect(),
        active_statics: input.active_static_fields().iter().copied().collect(),
        dispatch: vec![],
    };
    let types = collect_types(program);
    let mut layouts = Vec::new();
    let projected_layouts = x86_64_sysv::project_layouts(input, &types)?;
    for (ty, layout) in types.into_iter().zip(projected_layouts) {
        layouts.push((ty, facts.add_layout(layout)?));
    }
    // Reserve the complete canonical function-type pool before resolving any
    // code-address references (including higher-order signatures).
    let mut function_types = BTreeMap::new();
    for ty in program.function_types.iter() {
        function_types.insert(ty.id, facts.add_signature(unit_signature())?);
    }
    for ty in program.function_types.iter() {
        let id = function_types[&ty.id];
        facts.signatures[id.index()] = signature(
            &ty.parameters,
            ty.result,
            Convention::Language,
            &layouts,
            &function_types,
        );
    }
    for declaration in program.declarations.iter() {
        let convention = if matches!(declaration.linkage, MirFunctionLinkage::External { .. }) {
            Convention::ExternC
        } else {
            Convention::Language
        };
        let projected = signature(
            &declaration.parameters,
            declaration.return_type,
            convention,
            &layouts,
            &function_types,
        );
        let signature = intern_source_signature(
            &mut facts,
            projected,
            MirCallableSignature {
                parameters: &declaration.parameters,
                return_type: declaration.return_type,
            },
            false,
            program,
            &function_types,
        )?;
        let key = LirCallableId::Source(declaration.id.into());
        facts.callables.push(CallableDeclaration {
            key,
            signature,
            body: if program.has_executable_definition(declaration.id.into()) {
                BodyDisposition::Required
            } else {
                BodyDisposition::Absent
            },
        });
        if let MirFunctionLinkage::External { link } = declaration.linkage {
            if let Some(previous) = facts
                .artifacts
                .iter()
                .find(|a| a.key == ArtifactId::External(link))
            {
                if facts.signatures[previous.signature.expect("external signature").index()]
                    != facts.signatures[signature.index()]
                {
                    return Err(PlanError::InvalidSignature.into());
                }
            } else {
                facts.artifacts.push(ArtifactDeclaration {
                    key: ArtifactId::External(link),
                    signature: Some(signature),
                    layout: None,
                });
            }
        }
    }
    for (callable, receiver) in declaration_inventory(program)
        .into_iter()
        .filter(|(id, _)| !matches!(id, crate::identity::CallableId::Function(_)))
    {
        let declared = program
            .callable_signature(callable)
            .expect("declared member signature");
        let mut projected = signature(
            declared.parameters,
            declared.return_type,
            Convention::Language,
            &layouts,
            &function_types,
        );
        if receiver {
            projected.inputs.extend(
                [
                    ComponentRole::ReceiverStatic,
                    ComponentRole::ReceiverComplete,
                    ComponentRole::ReceiverMetadata,
                ]
                .map(|role| Component {
                    ty: ScalarType::DataAddress,
                    role,
                }),
            );
        }
        let signature = intern_source_signature(
            &mut facts,
            projected,
            declared,
            receiver,
            program,
            &function_types,
        )?;
        facts.callables.push(CallableDeclaration {
            key: LirCallableId::Source(callable),
            signature,
            body: if program.has_executable_definition(callable) {
                BodyDisposition::Required
            } else {
                BodyDisposition::Absent
            },
        });
    }
    let entry = facts.add_signature(SignatureFact {
        convention: Convention::ExternC,
        inputs: vec![],
        results: vec![Component {
            ty: ScalarType::I64,
            role: ComponentRole::Result,
        }],
        returns: ReturnShape::Scalar(ScalarType::I64),
    })?;
    facts.callables.push(CallableDeclaration {
        key: LirCallableId::Entry,
        signature: entry,
        body: BodyDisposition::Required,
    });
    for (service, inputs, returns) in [
        (RuntimeService::AbiMarker, &[][..], ReturnShape::Unit),
        (
            RuntimeService::Panic,
            &[ScalarType::DataAddress, ScalarType::U64][..],
            ReturnShape::Never,
        ),
    ] {
        let signature = facts.add_signature(SignatureFact {
            convention: Convention::Runtime,
            inputs: inputs
                .iter()
                .enumerate()
                .map(|(i, ty)| Component {
                    ty: *ty,
                    role: ComponentRole::RuntimeParameter(i),
                })
                .collect(),
            results: vec![],
            returns,
        })?;
        facts.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Runtime(service),
            signature: Some(signature),
            layout: None,
        });
    }
    // Freeze the complete pilot failure pool; retention later chooses used data.
    for message in [
        FailureMessage::ShiftCountOutOfRange,
        FailureMessage::IntegerDivisionByZero,
        FailureMessage::IntegerRemainderByZero,
        FailureMessage::PrimitiveCastOutOfRange,
    ] {
        data(
            &mut facts,
            ArtifactId::Data(DataKey::FailureMessage(message)),
            message.bytes().len(),
            1,
        )?;
    }
    let trace = x86_64_sysv::project_trace(input)?;
    for (i, bytes) in trace.strings.iter().enumerate() {
        data(
            &mut facts,
            ArtifactId::Data(DataKey::TraceBytes(i)),
            bytes.len(),
            1,
        )?;
    }
    for i in 0..trace.contexts.len() {
        data(
            &mut facts,
            ArtifactId::Data(DataKey::TraceContext(i)),
            32,
            8,
        )?;
    }
    for i in 0..trace.locations.len() {
        data(
            &mut facts,
            ArtifactId::Data(DataKey::TraceLocation(i)),
            24,
            8,
        )?;
    }
    if input.runtime_trace() == RuntimeTracePolicy::Enabled {
        data(&mut facts, ArtifactId::TraceTls, 8, 8)?;
    }
    let plan = CheckedPlan::check(facts)?;
    Ok(AdmittedPilot {
        program,
        plan,
        layouts,
        function_types,
        trace,
    })
}

fn data(
    facts: &mut PlanFacts,
    key: ArtifactId,
    size: usize,
    alignment: usize,
) -> Result<(), PlanError> {
    let layout = facts.add_layout(LayoutFact {
        size,
        alignment,
        disposition: LayoutDisposition::Addressable,
    })?;
    facts.artifacts.push(ArtifactDeclaration {
        key,
        signature: None,
        layout: Some(layout),
    });
    Ok(())
}
