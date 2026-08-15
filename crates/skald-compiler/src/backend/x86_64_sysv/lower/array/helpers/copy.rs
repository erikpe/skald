//! Element-copy helpers specialized by array element category.

use crate::{
    backend::{
        x86_64_sysv::{
            layout::DataLayout,
            machine::{AssemblyFunction, ByteRegister, Instruction, Label, Operand, Register},
            symbol,
        },
        BackendError, Target,
    },
    mir::{MirProgram, MirType},
};

use super::{helper_error, materialize_helper_element_addresses, offset_operand};
use crate::backend::x86_64_sysv::lower::{call, value};

pub(super) fn lower_copier(
    program: &MirProgram,
    array: crate::identity::ArrayTypeId,
    element: MirType,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout.array(array).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} has no helper layout"),
        )
    })?;
    let displacement = i32::try_from(layout.element_offset()).map_err(|_| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} element offset cannot be encoded"),
        )
    })?;
    let (source, destination, mut address_setup) = if matches!(layout.stride(), 1 | 2 | 4 | 8) {
        let scale = u8::try_from(layout.stride()).expect("encodable array stride");
        (
            value::indexed_memory(Register::Rsi, Register::Rcx, scale, displacement),
            value::indexed_memory(Register::Rdi, Register::Rdx, scale, displacement),
            Vec::new(),
        )
    } else {
        (
            value::memory(Register::Rsi, 0),
            value::memory(Register::Rdi, 0),
            materialize_helper_element_addresses(layout.stride(), displacement),
        )
    };
    let mut instructions = match element {
        MirType::U8 | MirType::Bool => vec![
            Instruction::LoadZeroExtendByte {
                source,
                destination: Register::Rax,
            },
            Instruction::MoveByte {
                source: ByteRegister::Al,
                destination,
            },
        ],
        MirType::I64 | MirType::U64 | MirType::F64 => {
            let mut instructions = Vec::new();
            value::load_rax(source, &mut instructions);
            value::store_rax(destination, &mut instructions);
            instructions
        }
        MirType::Optional(optional) => {
            let metadata = program
                .optional_type(optional)
                .expect("verified optional array element metadata exists");
            if let Some(class) = metadata.inline_class() {
                lower_optional_class_copier(
                    program,
                    array,
                    class,
                    source,
                    destination,
                    data_layout,
                )?
            } else if metadata.shared_owner().is_some() {
                lower_shared_copier(array, source, destination, true)
            } else if let Some(payload) = metadata.primitive() {
                let optional_layout = data_layout.optional_type(optional)?;
                let payload_offset = i32::try_from(optional_layout.payload_offset())
                    .map_err(|_| helper_error("optional payload offset exceeds x86-64"))?;
                let stem = format!(".Lska_array_{}_copy_optional", array.index());
                let present = Label::new(format!("{stem}_present"));
                let complete = Label::new(format!("{stem}_complete"));
                let mut instructions = vec![
                    Instruction::Move {
                        source,
                        destination: Register::Rax.into(),
                    },
                    Instruction::Test(Register::Rax),
                    Instruction::JumpIfNotZero(present.clone()),
                    Instruction::Move {
                        source: Register::Rax.into(),
                        destination,
                    },
                    Instruction::Jump(complete.clone()),
                    Instruction::Label(present),
                ];
                let source_payload = offset_operand(source, payload_offset)?;
                let destination_payload = offset_operand(destination, payload_offset)?;
                if matches!(
                    payload,
                    crate::mir::MirPrimitiveType::U8 | crate::mir::MirPrimitiveType::Bool
                ) {
                    instructions.push(Instruction::LoadZeroExtendByte {
                        source: source_payload,
                        destination: Register::Rax,
                    });
                    instructions.push(Instruction::MoveByte {
                        source: ByteRegister::Al,
                        destination: destination_payload,
                    });
                } else {
                    value::load_rax(source_payload, &mut instructions);
                    value::store_rax(destination_payload, &mut instructions);
                }
                instructions.extend([
                    Instruction::MoveImmediate64 {
                        bits: 1,
                        destination: Register::Rax,
                    },
                    Instruction::Move {
                        source: Register::Rax.into(),
                        destination,
                    },
                    Instruction::Label(complete),
                ]);
                instructions
            } else if matches!(
                metadata.storage,
                crate::mir::MirOptionalStorage::Nested(_)
                    | crate::mir::MirOptionalStorage::InlineArray(_)
            ) {
                lower_aggregate_optional_copier(
                    program,
                    array,
                    optional,
                    source,
                    destination,
                    data_layout,
                )?
            } else {
                return Err(helper_error(format!(
                    "array {array} has an unsupported optional copy element {element}"
                )));
            }
        }
        MirType::Class(class) => vec![
            Instruction::LoadEffectiveAddress {
                source: destination,
                destination: Register::Rdi,
            },
            Instruction::LoadEffectiveAddress {
                source,
                destination: Register::Rsi,
            },
            call::direct_instruction(
                symbol::class_copy_helper(program, class),
                call::TraceAttribution::InheritedSourceOperation,
            ),
        ],
        MirType::Array(inner) => lower_nested_array_copier(array, inner, source, destination),
        MirType::Shared(_) => lower_shared_copier(array, source, destination, false),
        MirType::Function(_) | MirType::Interface(_) | MirType::Obj | MirType::Unit => {
            return Err(helper_error(format!(
                "array {array} has an unsupported copy element {element}"
            )));
        }
    };
    let calls = instructions.iter().any(call::is_call_instruction);
    if calls {
        instructions.insert(0, Instruction::Push(Register::Rbp));
        instructions.insert(
            1,
            Instruction::Move {
                source: Register::Rsp.into(),
                destination: Register::Rbp.into(),
            },
        );
        instructions.extend([Instruction::Leave, Instruction::Return]);
    } else {
        instructions.push(Instruction::Return);
    }
    address_setup.extend(instructions);
    Ok(AssemblyFunction {
        symbol: symbol::array_copy_element(array),
        exported: false,
        instructions: address_setup,
    })
}

fn lower_optional_class_copier(
    program: &MirProgram,
    array: crate::identity::ArrayTypeId,
    class: crate::identity::ClassId,
    source: Operand,
    destination: Operand,
    data_layout: &DataLayout,
) -> Result<Vec<Instruction>, BackendError> {
    let optional = program
        .optional_for_payload(MirType::Class(class))
        .expect("verified optional-class array metadata exists");
    let payload_offset = i32::try_from(data_layout.optional_type(optional)?.payload_offset())
        .map_err(|_| helper_error("optional class payload offset exceeds x86-64"))?;
    let source_payload = offset_operand(source, payload_offset)?;
    let destination_payload = offset_operand(destination, payload_offset)?;
    let stem = format!(".Lska_array_{}_copy_optional_class", array.index());
    let present = Label::new(format!("{stem}_present"));
    let complete = Label::new(format!("{stem}_complete"));
    Ok(vec![
        Instruction::Move {
            source,
            destination: Register::Rax.into(),
        },
        Instruction::Test(Register::Rax),
        Instruction::JumpIfNotZero(present.clone()),
        Instruction::Move {
            source: Register::Rax.into(),
            destination,
        },
        Instruction::Jump(complete.clone()),
        Instruction::Label(present),
        Instruction::ReserveStack(16),
        Instruction::LoadEffectiveAddress {
            source: destination,
            destination: Register::R11,
        },
        Instruction::Move {
            source: Register::R11.into(),
            destination: value::memory(Register::Rsp, 0),
        },
        Instruction::LoadEffectiveAddress {
            source: destination_payload,
            destination: Register::Rdi,
        },
        Instruction::LoadEffectiveAddress {
            source: source_payload,
            destination: Register::Rsi,
        },
        call::direct_instruction(
            symbol::class_copy_helper(program, class),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::R11.into(),
        },
        Instruction::ReleaseStack(16),
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::R11, 0),
        },
        Instruction::Label(complete),
    ])
}

fn lower_nested_array_copier(
    _array: crate::identity::ArrayTypeId,
    inner: crate::identity::ArrayTypeId,
    source: Operand,
    destination: Operand,
) -> Vec<Instruction> {
    vec![
        Instruction::ReserveStack(16),
        Instruction::LoadEffectiveAddress {
            source: destination,
            destination: Register::R11,
        },
        Instruction::Move {
            source: Register::R11.into(),
            destination: value::memory(Register::Rsp, 0),
        },
        Instruction::Move {
            source,
            destination: Register::Rdi.into(),
        },
        call::direct_instruction(
            symbol::array_clone(inner),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::R11.into(),
        },
        Instruction::ReleaseStack(16),
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::R11, 0),
        },
    ]
}

fn lower_shared_copier(
    array: crate::identity::ArrayTypeId,
    source: Operand,
    destination: Operand,
    optional: bool,
) -> Vec<Instruction> {
    let absent = Label::new(format!(".Lska_array_{}_copy_shared_absent", array.index()));
    let complete = Label::new(format!(
        ".Lska_array_{}_copy_shared_complete",
        array.index()
    ));
    let mut instructions = vec![
        Instruction::ReserveStack(16),
        Instruction::LoadEffectiveAddress {
            source: destination,
            destination: Register::R11,
        },
        Instruction::Move {
            source: Register::R11.into(),
            destination: value::memory(Register::Rsp, 0),
        },
        Instruction::Move {
            source,
            destination: Register::Rdi.into(),
        },
    ];
    if optional {
        instructions.extend([
            Instruction::Test(Register::Rdi),
            Instruction::JumpIfEqual(absent.clone()),
        ]);
    }
    instructions.extend([
        call::direct_instruction(
            symbol::shared_handle_retain(),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::R11.into(),
        },
        Instruction::ReleaseStack(16),
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::R11, 0),
        },
    ]);
    if optional {
        instructions.extend([
            Instruction::Jump(complete.clone()),
            Instruction::Label(absent),
            Instruction::Move {
                source: value::memory(Register::Rsp, 0),
                destination: Register::R11.into(),
            },
            Instruction::ReleaseStack(16),
            Instruction::MoveImmediate64 {
                bits: 0,
                destination: Register::Rax,
            },
            Instruction::Move {
                source: Register::Rax.into(),
                destination: value::memory(Register::R11, 0),
            },
            Instruction::Label(complete),
        ]);
    }
    instructions
}

fn lower_aggregate_optional_copier(
    program: &MirProgram,
    array: crate::identity::ArrayTypeId,
    optional: crate::identity::OptionalTypeId,
    source: Operand,
    destination: Operand,
    data_layout: &DataLayout,
) -> Result<Vec<Instruction>, BackendError> {
    let metadata = program
        .optional_type(optional)
        .expect("verified optional metadata");
    match metadata.storage {
        crate::mir::MirOptionalStorage::Scalar => {
            let payload = metadata.primitive().expect("scalar optional payload");
            let layout = data_layout.optional_type(optional)?;
            let payload_offset = i32::try_from(layout.payload_offset())
                .map_err(|_| helper_error("optional payload offset exceeds x86-64"))?;
            let present = Label::new(format!(
                ".Lska_array_{}_copy_o{}_present",
                array.index(),
                optional.index()
            ));
            let complete = Label::new(format!(
                ".Lska_array_{}_copy_o{}_complete",
                array.index(),
                optional.index()
            ));
            let mut output = vec![
                Instruction::Move {
                    source,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfNotZero(present.clone()),
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination,
                },
                Instruction::Jump(complete.clone()),
                Instruction::Label(present),
            ];
            let source_payload = offset_operand(source, payload_offset)?;
            let destination_payload = offset_operand(destination, payload_offset)?;
            if matches!(
                payload,
                crate::mir::MirPrimitiveType::U8 | crate::mir::MirPrimitiveType::Bool
            ) {
                output.push(Instruction::LoadZeroExtendByte {
                    source: source_payload,
                    destination: Register::Rax,
                });
                output.push(Instruction::MoveByte {
                    source: ByteRegister::Al,
                    destination: destination_payload,
                });
            } else {
                value::load_rax(source_payload, &mut output);
                value::store_rax(destination_payload, &mut output);
            }
            output.extend([
                Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rax,
                },
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination,
                },
                Instruction::Label(complete),
            ]);
            Ok(output)
        }
        crate::mir::MirOptionalStorage::InlineClass(class) => {
            lower_optional_class_copier(program, array, class, source, destination, data_layout)
        }
        crate::mir::MirOptionalStorage::SharedOwner(_) => {
            Ok(lower_shared_copier(array, source, destination, true))
        }
        crate::mir::MirOptionalStorage::Nested(nested) => {
            let payload_offset =
                i32::try_from(data_layout.optional_type(optional)?.payload_offset())
                    .map_err(|_| helper_error("nested optional payload offset exceeds x86-64"))?;
            let present = Label::new(format!(
                ".Lska_array_{}_copy_o{}_present",
                array.index(),
                optional.index()
            ));
            let complete = Label::new(format!(
                ".Lska_array_{}_copy_o{}_complete",
                array.index(),
                optional.index()
            ));
            let mut output = vec![
                Instruction::Move {
                    source,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfNotZero(present.clone()),
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination,
                },
                Instruction::Jump(complete.clone()),
                Instruction::Label(present),
                Instruction::ReserveStack(16),
                Instruction::LoadEffectiveAddress {
                    source: destination,
                    destination: Register::R11,
                },
                Instruction::Move {
                    source: Register::R11.into(),
                    destination: value::memory(Register::Rsp, 0),
                },
            ];
            output.extend(lower_aggregate_optional_copier(
                program,
                array,
                nested,
                offset_operand(source, payload_offset)?,
                offset_operand(destination, payload_offset)?,
                data_layout,
            )?);
            output.extend([
                Instruction::Move {
                    source: value::memory(Register::Rsp, 0),
                    destination: Register::R11.into(),
                },
                Instruction::ReleaseStack(16),
                Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rax,
                },
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination: value::memory(Register::R11, 0),
                },
                Instruction::Label(complete),
            ]);
            Ok(output)
        }
        crate::mir::MirOptionalStorage::InlineArray(inner) => {
            let payload_offset =
                i32::try_from(data_layout.optional_type(optional)?.payload_offset())
                    .map_err(|_| helper_error("optional array payload offset exceeds x86-64"))?;
            let present = Label::new(format!(
                ".Lska_array_{}_copy_o{}_present",
                array.index(),
                optional.index()
            ));
            let complete = Label::new(format!(
                ".Lska_array_{}_copy_o{}_complete",
                array.index(),
                optional.index()
            ));
            let mut output = vec![
                Instruction::Move {
                    source,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfNotZero(present.clone()),
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination,
                },
                Instruction::Jump(complete.clone()),
                Instruction::Label(present),
                Instruction::ReserveStack(16),
                Instruction::LoadEffectiveAddress {
                    source: destination,
                    destination: Register::R11,
                },
                Instruction::Move {
                    source: Register::R11.into(),
                    destination: value::memory(Register::Rsp, 0),
                },
            ];
            output.extend(lower_nested_array_copier(
                array,
                inner,
                offset_operand(source, payload_offset)?,
                offset_operand(destination, payload_offset)?,
            ));
            output.extend([
                Instruction::Move {
                    source: value::memory(Register::Rsp, 0),
                    destination: Register::R11.into(),
                },
                Instruction::ReleaseStack(16),
                Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rax,
                },
                Instruction::Move {
                    source: Register::Rax.into(),
                    destination: value::memory(Register::R11, 0),
                },
                Instruction::Label(complete),
            ]);
            Ok(output)
        }
    }
}
