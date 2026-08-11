//! Semantic runtime-trace names derived only from final MIR declarations.

use std::collections::BTreeSet;

use crate::{
    backend::{BackendError, Target},
    identity::{ArrayTypeId, CallableId, ClassId, InterfaceId, ModuleId},
    mir::{MirParameter, MirParameterMode, MirProgram, MirSharedTarget, MirType},
};

pub(super) fn callable(program: &MirProgram, id: CallableId) -> Result<String, BackendError> {
    match id {
        CallableId::Function(function) => {
            let declaration = program.declarations.get(function).ok_or_else(|| {
                error(
                    Some(id),
                    "runtime trace callable has no function declaration",
                )
            })?;
            Ok(format!(
                "{}::{}",
                module_name(program, declaration.module, id)?,
                declaration.name
            ))
        }
        CallableId::StaticInitializer(initializer) => {
            let class = class(program, initializer.class(), id)?;
            let field = program
                .static_field(initializer.field())
                .ok_or_else(|| error(Some(id), "runtime trace static initializer has no field"))?;
            Ok(format!(
                "{}::{}.{}::<static-init>",
                module_name(program, class.module, id)?,
                class.name,
                field.name
            ))
        }
        CallableId::Initializer(initializer) => {
            let class = class(program, initializer.class(), id)?;
            let declaration = program
                .initializer(initializer)
                .ok_or_else(|| error(Some(id), "runtime trace initializer has no declaration"))?;
            Ok(format!(
                "{}::{}.init({})",
                module_name(program, class.module, id)?,
                class.name,
                parameter_list(program, &declaration.parameters, id)?
            ))
        }
        CallableId::CopyConstructor(copy) => {
            member_lifecycle_name(program, copy.class(), id, "copy")
        }
        CallableId::CopyAssignment(assignment) => {
            member_lifecycle_name(program, assignment.class(), id, "assign")
        }
        CallableId::Destructor(destructor) => {
            member_lifecycle_name(program, destructor.class(), id, "destroy")
        }
        CallableId::Method(method) => {
            let class = class(program, method.class(), id)?;
            let declaration = program
                .method(method)
                .ok_or_else(|| error(Some(id), "runtime trace method has no declaration"))?;
            Ok(format!(
                "{}::{}.{}",
                module_name(program, class.module, id)?,
                class.name,
                declaration.name
            ))
        }
    }
}

pub(super) fn module_for_callable(
    program: &MirProgram,
    id: CallableId,
) -> Result<ModuleId, BackendError> {
    match id {
        CallableId::Function(function) => program
            .declarations
            .get(function)
            .map(|declaration| declaration.module)
            .ok_or_else(|| {
                error(
                    Some(id),
                    "runtime trace callable has no function declaration",
                )
            }),
        _ => {
            let class_id = id
                .class()
                .ok_or_else(|| error(Some(id), "runtime trace callable has no module owner"))?;
            Ok(class(program, class_id, id)?.module)
        }
    }
}

fn member_lifecycle_name(
    program: &MirProgram,
    class_id: ClassId,
    callable: CallableId,
    operation: &str,
) -> Result<String, BackendError> {
    let class = class(program, class_id, callable)?;
    Ok(format!(
        "{}::{}.{}",
        module_name(program, class.module, callable)?,
        class.name,
        operation
    ))
}

fn parameter_list(
    program: &MirProgram,
    parameters: &[MirParameter],
    callable: CallableId,
) -> Result<String, BackendError> {
    parameters
        .iter()
        .map(|parameter| parameter_name(program, *parameter, callable))
        .collect::<Result<Vec<_>, _>>()
        .map(|names| names.join(", "))
}

fn parameter_name(
    program: &MirProgram,
    parameter: MirParameter,
    callable: CallableId,
) -> Result<String, BackendError> {
    let mode = match parameter.mode {
        MirParameterMode::Value => "",
        MirParameterMode::ReadOnlyAlias => "ref ",
        MirParameterMode::MutableAlias => "mut ref ",
    };
    let mut active_arrays = BTreeSet::new();
    Ok(format!(
        "{mode}{}",
        type_name(program, parameter.ty, callable, &mut active_arrays)?
    ))
}

fn type_name(
    program: &MirProgram,
    ty: MirType,
    callable: CallableId,
    active_arrays: &mut BTreeSet<ArrayTypeId>,
) -> Result<String, BackendError> {
    match ty {
        MirType::I64 => Ok("i64".into()),
        MirType::U64 => Ok("u64".into()),
        MirType::U8 => Ok("u8".into()),
        MirType::F64 => Ok("f64".into()),
        MirType::Bool => Ok("bool".into()),
        MirType::Unit => Ok("unit".into()),
        MirType::Obj => Ok("Obj".into()),
        MirType::Class(class) => class_name(program, class, callable),
        MirType::Interface(interface) => interface_name(program, interface, callable),
        MirType::Array(array) => {
            if !active_arrays.insert(array) {
                return Err(error(
                    Some(callable),
                    "runtime trace initializer signature contains a recursive array type",
                ));
            }
            let element = program.array_type(array).ok_or_else(|| {
                error(
                    Some(callable),
                    "runtime trace initializer signature has an unknown array type",
                )
            })?;
            let name = format!(
                "{}[]",
                type_name(program, element.element, callable, active_arrays)?
            );
            active_arrays.remove(&array);
            Ok(name)
        }
        MirType::Shared(target) => Ok(format!(
            "shared {}",
            shared_target_name(program, target, callable, active_arrays)?
        )),
        MirType::Optional(optional) => {
            let metadata = program.optional_type(optional).ok_or_else(|| {
                error(
                    Some(callable),
                    "runtime trace signature has an unknown optional type",
                )
            })?;
            if let Some(target) = metadata.shared_owner() {
                Ok(format!(
                    "shared? {}",
                    shared_target_name(program, target, callable, active_arrays)?
                ))
            } else {
                Ok(format!(
                    "{}?",
                    type_name(program, metadata.payload, callable, active_arrays)?
                ))
            }
        }
    }
}

fn shared_target_name(
    program: &MirProgram,
    target: MirSharedTarget,
    callable: CallableId,
    active_arrays: &mut BTreeSet<ArrayTypeId>,
) -> Result<String, BackendError> {
    match target {
        MirSharedTarget::Obj => Ok("Obj".into()),
        MirSharedTarget::Class(class) => class_name(program, class, callable),
        MirSharedTarget::Interface(interface) => interface_name(program, interface, callable),
        MirSharedTarget::Array(array) => {
            type_name(program, MirType::Array(array), callable, active_arrays)
        }
        MirSharedTarget::OptionalBox(target) => Ok(format!("optional-box {target}")),
    }
}

fn class_name(
    program: &MirProgram,
    class_id: ClassId,
    callable: CallableId,
) -> Result<String, BackendError> {
    let class = class(program, class_id, callable)?;
    Ok(format!(
        "{}::{}",
        module_name(program, class.module, callable)?,
        class.name
    ))
}

fn interface_name(
    program: &MirProgram,
    interface: InterfaceId,
    callable: CallableId,
) -> Result<String, BackendError> {
    let declaration = program.interface(interface).ok_or_else(|| {
        error(
            Some(callable),
            "runtime trace initializer signature has an unknown interface type",
        )
    })?;
    Ok(format!(
        "{}::{}",
        module_name(program, declaration.module, callable)?,
        declaration.name
    ))
}

fn class(
    program: &MirProgram,
    class: ClassId,
    callable: CallableId,
) -> Result<&crate::mir::MirClassDeclaration, BackendError> {
    program.class(class).ok_or_else(|| {
        error(
            Some(callable),
            "runtime trace callable has no class declaration",
        )
    })
}

fn module_name(
    program: &MirProgram,
    module: ModuleId,
    callable: CallableId,
) -> Result<String, BackendError> {
    program
        .modules
        .get(module)
        .map(|provenance| provenance.module_path().to_string())
        .ok_or_else(|| {
            error(
                Some(callable),
                "runtime trace callable has no module provenance",
            )
        })
}

fn error(callable: Option<CallableId>, message: &str) -> BackendError {
    BackendError::new(Target::X86_64SysV, callable, message)
}
