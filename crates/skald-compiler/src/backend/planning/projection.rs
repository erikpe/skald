//! Final-MIR queries stop here. Downstream facts carry no frontend authority.

use super::{
    layouts::collect_types,
    signatures::{declaration_inventory, intern_source_signature, signature, unit_signature},
};
use super::{AdmissionError, AdmittedProgram};
use crate::backend::{plan::*, x86_64_sysv, BackendInput};
use crate::mir::*;
use std::collections::BTreeMap;

pub(in crate::backend) fn admit(
    input: BackendInput<'_>,
) -> Result<AdmittedProgram<'_>, AdmissionError> {
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
        semantic: SemanticFacts::default(),
        resources: ResourceFacts::default(),
    };
    let types = collect_types(program);
    let mut layouts = Vec::new();
    let (projected_layouts, semantic_projection) =
        x86_64_sysv::begin_semantic_projection(input, &types)?;
    for (ty, layout) in types.into_iter().zip(projected_layouts) {
        layouts.push((ty, facts.add_layout(layout)?));
    }
    // Reserve the complete canonical function-type pool before resolving any
    // code-address references (including higher-order signatures).
    let mut function_types = BTreeMap::new();
    for ty in program.function_types.iter() {
        function_types.insert(ty.id, facts.add_signature(unit_signature())?);
    }
    let mut interface_requirements = BTreeMap::new();
    for interface in program.interfaces.iter() {
        for requirement in &interface.requirements {
            let mut projected = signature(
                program,
                &requirement.parameters,
                requirement.return_type,
                Convention::Language,
                &layouts,
                &function_types,
            );
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
            interface_requirements.insert(requirement.id, facts.add_signature(projected)?);
        }
    }
    for ty in program.function_types.iter() {
        let id = function_types[&ty.id];
        facts.signatures[id.index()] = signature(
            program,
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
            program,
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
            program,
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
    let trace = x86_64_sysv::project_trace(input)?;
    // Reuse the target's existing runtime frame shape; freeze its checked ID.
    let trace_record_layout = trace
        .record_layout
        .map(|layout| facts.add_layout(layout))
        .transpose()?;
    facts.semantic = x86_64_sysv::finish_semantic_projection(
        semantic_projection,
        program,
        &layouts,
        &interface_requirements,
    )?;
    super::resources::project(input, &layouts, &trace, &mut facts)?;
    let plan = CheckedPlan::check(facts)?;
    Ok(AdmittedProgram {
        program,
        plan,
        layouts,
        function_types,
        trace,
        trace_record_layout,
    })
}

#[cfg(test)]
pub(super) fn project_semantic_catalog(
    input: BackendInput<'_>,
) -> Result<CheckedPlan, AdmissionError> {
    project_test_catalog(input, false)
}

#[cfg(test)]
pub(in crate::backend) fn project_resource_catalog(
    input: BackendInput<'_>,
) -> Result<CheckedPlan, AdmissionError> {
    project_test_catalog(input, true)
}

#[cfg(test)]
fn project_test_catalog(
    input: BackendInput<'_>,
    include_resources: bool,
) -> Result<CheckedPlan, AdmissionError> {
    let program = input.program();
    let types = collect_types(program);
    let (layouts, projection) = x86_64_sysv::begin_semantic_projection(input, &types)?;
    let executable_sources = program
        .executable_definitions()
        .map(|definition| definition.callable())
        .collect::<std::collections::BTreeSet<_>>();
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
        executable_sources: executable_sources.clone(),
        active_statics: input.active_static_fields().iter().copied().collect(),
        dispatch: vec![],
        semantic: SemanticFacts::default(),
        resources: ResourceFacts::default(),
    };
    let type_layouts = types
        .into_iter()
        .zip(layouts)
        .map(|(ty, layout)| facts.add_layout(layout).map(|id| (ty, id)))
        .collect::<Result<Vec<_>, _>>()?;
    let signature = facts.add_signature(unit_signature())?;
    facts.callables = executable_sources
        .into_iter()
        .map(|source| CallableDeclaration {
            key: LirCallableId::Source(source),
            signature,
            body: BodyDisposition::Required,
        })
        .collect();
    let requirement_signatures = program
        .interfaces
        .iter()
        .flat_map(|interface| interface.requirements.iter())
        .map(|requirement| (requirement.id, signature))
        .collect();
    facts.semantic = x86_64_sysv::finish_semantic_projection(
        projection,
        program,
        &type_layouts,
        &requirement_signatures,
    )?;
    if include_resources {
        let entry_signature = facts.add_signature(SignatureFact {
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
            signature: entry_signature,
            body: BodyDisposition::Required,
        });
        let trace = x86_64_sysv::project_trace(input)?;
        super::resources::project(input, &type_layouts, &trace, &mut facts)?;
    }
    CheckedPlan::check(facts).map_err(AdmissionError::from)
}
