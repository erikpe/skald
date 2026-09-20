use crate::backend::plan::*;
use crate::{identity::FunctionTypeId, mir::*};
use std::collections::BTreeMap;

pub(super) fn unit_signature() -> SignatureFact {
    SignatureFact {
        convention: Convention::Language,
        inputs: vec![],
        results: vec![],
        returns: ReturnShape::Unit,
    }
}

fn scalar(
    program: &MirProgram,
    ty: MirType,
    functions: &BTreeMap<FunctionTypeId, SignatureId>,
) -> Option<ScalarType> {
    Some(match ty {
        MirType::I64 => ScalarType::I64,
        MirType::U64 => ScalarType::U64,
        MirType::U8 => ScalarType::U8,
        MirType::Bool => ScalarType::Bool,
        MirType::F64 => ScalarType::F64,
        MirType::Function(id) => ScalarType::CodeAddress(functions[&id]),
        MirType::Shared(_) => ScalarType::DataAddress,
        MirType::Optional(id)
            if program
                .optional_type(id)
                .is_some_and(|optional| optional.shared_owner().is_some()) =>
        {
            ScalarType::DataAddress
        }
        _ => return None,
    })
}
pub(super) fn signature(
    program: &MirProgram,
    parameters: &[MirParameter],
    result: MirType,
    convention: Convention,
    layouts: &[(MirType, LayoutId)],
    functions: &BTreeMap<FunctionTypeId, SignatureId>,
) -> SignatureFact {
    let layout = |ty| {
        layouts
            .iter()
            .find(|(t, _)| *t == ty)
            .expect("complete type layout pool")
            .1
    };
    let returns = if result == MirType::Unit {
        ReturnShape::Unit
    } else if let Some(ty) = scalar(program, result, functions) {
        ReturnShape::Scalar(ty)
    } else {
        ReturnShape::Aggregate(layout(result))
    };
    let mut inputs = Vec::new();
    if let ReturnShape::Aggregate(layout) = returns {
        inputs.push(Component {
            ty: ScalarType::DataAddress,
            role: ComponentRole::ResultDestination(layout),
        });
    }
    for (index, parameter) in parameters.iter().enumerate() {
        if parameter.mode != MirParameterMode::Value {
            inputs.push(Component {
                ty: ScalarType::DataAddress,
                role: ComponentRole::AliasAddress(index),
            });
            if matches!(
                parameter.ty,
                MirType::Class(_) | MirType::Interface(_) | MirType::Obj
            ) {
                inputs.extend(
                    [
                        ComponentRole::AliasComplete(index),
                        ComponentRole::AliasMetadata(index),
                    ]
                    .map(|role| Component {
                        ty: ScalarType::DataAddress,
                        role,
                    }),
                );
            }
        } else if let Some(ty) = scalar(program, parameter.ty, functions) {
            inputs.push(Component {
                ty,
                role: ComponentRole::Parameter(index),
            });
        } else if parameter.ty != MirType::Unit {
            inputs.push(Component {
                ty: ScalarType::DataAddress,
                role: ComponentRole::AggregateAddress {
                    parameter: index,
                    layout: layout(parameter.ty),
                },
            });
        }
    }
    let results = if let ReturnShape::Scalar(ty) = returns {
        vec![Component {
            ty,
            role: ComponentRole::Result,
        }]
    } else {
        vec![]
    };
    SignatureFact {
        convention,
        inputs,
        results,
        returns,
    }
}
pub(super) fn declaration_inventory(
    program: &MirProgram,
) -> Vec<(crate::identity::CallableId, bool)> {
    let mut declarations = program
        .declarations
        .iter()
        .map(|d| (d.id.into(), false))
        .collect::<Vec<_>>();
    for class in program.classes.iter() {
        declarations.extend(class.initializers.iter().map(|d| (d.id.into(), true)));
        declarations.extend(
            class
                .copy_constructor_declaration
                .iter()
                .map(|d| (d.id.into(), true)),
        );
        declarations.extend(
            class
                .copy_assignment_declaration
                .iter()
                .map(|d| (d.id.into(), true)),
        );
        declarations.extend(
            class
                .destruction
                .destructor
                .iter()
                .map(|d| (d.id.into(), true)),
        );
        declarations.extend(
            class
                .methods
                .iter()
                .map(|d| (d.id.into(), d.kind != MirMethodKind::Static)),
        );
    }
    if let Some(lifecycle) = &program.static_lifecycle {
        declarations.extend(
            lifecycle
                .initializers()
                .iter()
                .map(|body| (body.id.into(), false)),
        );
    }
    declarations
}

pub(super) fn intern_source_signature(
    facts: &mut PlanFacts,
    projected: SignatureFact,
    declared: MirCallableSignature<'_>,
    receiver: bool,
    program: &MirProgram,
    functions: &BTreeMap<FunctionTypeId, SignatureId>,
) -> Result<SignatureId, PlanError> {
    // Equal physical cells are insufficient: erased alias mutability or shared
    // targets can differ in the exact canonical source signature.
    if !receiver && projected.convention == Convention::Language {
        if let Some(signature) = program
            .function_types
            .iter()
            .find(|ty| ty.parameters == declared.parameters && ty.result == declared.return_type)
        {
            return Ok(functions[&signature.id]);
        }
    }
    facts.add_signature(projected)
}
