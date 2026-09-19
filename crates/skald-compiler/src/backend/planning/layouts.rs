use super::signatures::declaration_inventory;
use crate::mir::*;

pub(super) fn collect_types(program: &MirProgram) -> Vec<MirType> {
    let mut types = vec![
        MirType::I64,
        MirType::U64,
        MirType::U8,
        MirType::Bool,
        MirType::F64,
        MirType::Unit,
        MirType::Obj,
        MirType::Shared(MirSharedTarget::Obj),
    ];
    types.extend(
        program
            .function_types
            .iter()
            .map(|t| MirType::Function(t.id)),
    );
    types.extend(program.classes.iter().map(|c| MirType::Class(c.id)));
    types.extend(program.interfaces.iter().map(|i| MirType::Interface(i.id)));
    types.extend(program.array_types.iter().map(|a| MirType::Array(a.id)));
    types.extend(
        program
            .optional_types
            .iter()
            .map(|o| MirType::Optional(o.id)),
    );
    for ty in program
        .classes
        .iter()
        .flat_map(|class| class.fields.iter().map(|field| field.ty))
        .chain(program.array_types.iter().map(|array| array.element))
        .chain(
            program
                .optional_types
                .iter()
                .map(|optional| optional.payload),
        )
    {
        if !types.contains(&ty) {
            types.push(ty);
        }
    }
    for ty in declaration_inventory(program)
        .into_iter()
        .flat_map(|(id, _)| {
            let signature = program.callable_signature(id).expect("declared signature");
            signature
                .parameters
                .iter()
                .map(|p| p.ty)
                .chain([signature.return_type])
        })
        .chain(
            program
                .function_types
                .iter()
                .flat_map(|t| t.parameters.iter().map(|p| p.ty).chain([t.result])),
        )
    {
        if !types.contains(&ty) {
            types.push(ty);
        }
    }
    types
}
