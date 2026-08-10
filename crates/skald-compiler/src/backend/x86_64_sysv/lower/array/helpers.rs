//! Deterministic inline-array helpers specialized by canonical array ID.

use crate::{
    backend::{BackendError, Target},
    mir::{MirProgram, MirType},
};

use super::super::super::{
    layout::{DataLayout, ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET},
    machine::{AssemblyFunction, ByteRegister, Instruction, Label, Operand, Register},
    symbol,
};
use super::super::{call, value};

const RUNTIME_FREE: &str = "ska_rt_free";

pub(super) fn lower_all(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    program
        .array_types
        .iter()
        .flat_map(|array| {
            [
                lower_initializer(array.id, array.element, data_layout),
                lower_copier(program, array.id, array.element, data_layout),
                lower_clone(array.id, data_layout),
                lower_destroyer(program, array.id, array.element, data_layout),
                lower_release(array.id),
                lower_shared_finalizer(array.id, data_layout),
            ]
        })
        .collect()
}

fn lower_shared_finalizer(
    array: crate::identity::ArrayTypeId,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout
        .array(array)
        .ok_or_else(|| helper_error(format!("array {array} has no shared finalizer layout")))?;
    // Generic shared release passes the payload address (immediately after the
    // count and metadata words). The existing exact element destroyer accepts
    // an inline backing address, so derive an address that gives it the same
    // element position without duplicating lifecycle lowering.
    let adjustment = i64::try_from(layout.shared_element_offset())
        .ok()
        .and_then(|shared| {
            let payload = i64::try_from(super::super::super::layout::SHARED_HEADER_SIZE).ok()?;
            let inline = i64::try_from(layout.element_offset()).ok()?;
            shared.checked_sub(payload)?.checked_sub(inline)
        })
        .and_then(|offset| i32::try_from(offset).ok())
        .ok_or_else(|| helper_error("shared array finalizer offset cannot be encoded"))?;
    let stem = format!(".Lska_array_{}_finalize_shared", array.index());
    let header = Label::new(format!("{stem}_header"));
    let body = Label::new(format!("{stem}_body"));
    let complete = Label::new(format!("{stem}_complete"));
    let payload_home = value::memory(Register::Rbp, -8);
    let index_home = value::memory(Register::Rbp, -16);
    Ok(AssemblyFunction {
        symbol: symbol::shared_array_finalizer(array),
        exported: false,
        instructions: vec![
            Instruction::Push(Register::Rbp),
            Instruction::Move {
                source: Register::Rsp.into(),
                destination: Register::Rbp.into(),
            },
            Instruction::ReserveStack(16),
            Instruction::Move {
                source: Register::Rdi.into(),
                destination: payload_home,
            },
            Instruction::Move {
                source: value::memory(Register::Rdi, 0),
                destination: Register::Rax.into(),
            },
            Instruction::Move {
                source: Register::Rax.into(),
                destination: index_home,
            },
            Instruction::Label(header.clone()),
            Instruction::Move {
                source: index_home,
                destination: Register::Rax.into(),
            },
            Instruction::Test(Register::Rax),
            Instruction::JumpIfEqual(complete.clone()),
            Instruction::MoveImmediate64 {
                bits: 1,
                destination: Register::R11,
            },
            Instruction::Subtract {
                source: Register::R11,
                destination: Register::Rax,
            },
            Instruction::Move {
                source: Register::Rax.into(),
                destination: index_home,
            },
            Instruction::Jump(body.clone()),
            Instruction::Label(body),
            Instruction::Move {
                source: payload_home,
                destination: Register::Rax.into(),
            },
            Instruction::LoadEffectiveAddress {
                source: value::memory(Register::Rax, adjustment),
                destination: Register::Rdi,
            },
            Instruction::Move {
                source: index_home,
                destination: Register::Rsi.into(),
            },
            call::direct_instruction(
                symbol::array_destroy_element(array),
                call::TraceAttribution::InheritedSourceOperation,
            ),
            Instruction::Jump(header),
            Instruction::Label(complete),
            Instruction::Leave,
            Instruction::Return,
        ],
    })
}

fn lower_initializer(
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
    let (destination, mut address_setup) = if matches!(layout.stride(), 1 | 2 | 4 | 8) {
        (
            value::indexed_memory(
                Register::Rdi,
                Register::Rsi,
                u8::try_from(layout.stride()).expect("encodable array stride"),
                displacement,
            ),
            Vec::new(),
        )
    } else {
        (
            value::memory(Register::Rdi, 0),
            materialize_destroy_element_address(layout.stride(), displacement),
        )
    };
    let mut instructions = vec![Instruction::MoveImmediate64 {
        bits: 0,
        destination: Register::Rax,
    }];
    if matches!(element, MirType::U8 | MirType::Bool) {
        instructions.push(Instruction::MoveByte {
            source: ByteRegister::Al,
            destination,
        });
    } else {
        value::store_rax(destination, &mut instructions);
    }
    instructions.push(Instruction::Return);
    address_setup.extend(instructions);
    Ok(AssemblyFunction {
        symbol: symbol::array_initialize_element(array),
        exported: false,
        instructions: address_setup,
    })
}

fn lower_clone(
    array: crate::identity::ArrayTypeId,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout.array(array).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
            format!("array {array} has no clone layout"),
        )
    })?;
    let stem = format!(".Lska_array_{}_clone", array.index());
    let empty = Label::new(format!("{stem}_empty"));
    let header = Label::new(format!("{stem}_header"));
    let body = Label::new(format!("{stem}_body"));
    let complete = Label::new(format!("{stem}_complete"));
    let source_home = value::memory(Register::Rbp, -8);
    let length_home = value::memory(Register::Rbp, -16);
    let destination_home = value::memory(Register::Rbp, -24);
    let index_home = value::memory(Register::Rbp, -32);
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::ReserveStack(32),
        Instruction::Test(Register::Rdi),
        Instruction::JumpIfEqual(empty.clone()),
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: source_home,
        },
        Instruction::Move {
            source: value::memory(Register::Rdi, ARRAY_LENGTH_OFFSET),
            destination: Register::Rax.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: length_home,
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.element_offset()).expect("array offset fits u64"),
            destination: Register::R11,
        },
        Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        },
        call::direct_instruction(
            "ska_rt_alloc",
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: Register::Rax.into(),
            destination: destination_home,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        },
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdx, ARRAY_OWNER_COUNT_OFFSET),
        },
        Instruction::Move {
            source: length_home,
            destination: Register::Rax.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdx, ARRAY_LENGTH_OFFSET),
        },
        Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rcx,
        },
        Instruction::Label(header.clone()),
        Instruction::Move {
            source: length_home,
            destination: Register::R11.into(),
        },
        Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        },
        Instruction::JumpIfBelow(body.clone()),
        Instruction::Jump(complete.clone()),
        Instruction::Label(body),
        Instruction::Move {
            source: source_home,
            destination: Register::Rsi.into(),
        },
        Instruction::Move {
            source: destination_home,
            destination: Register::Rdi.into(),
        },
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: Register::Rdx.into(),
        },
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: index_home,
        },
        call::direct_instruction(
            symbol::array_copy_element(array),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Move {
            source: index_home,
            destination: Register::Rcx.into(),
        },
    ];
    instructions.extend([
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        },
        Instruction::Add {
            source: Register::R11,
            destination: Register::Rcx,
        },
        Instruction::Jump(header),
        Instruction::Label(complete),
        Instruction::Move {
            source: destination_home,
            destination: Register::Rax.into(),
        },
        Instruction::Leave,
        Instruction::Return,
        Instruction::Label(empty),
        Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        },
        Instruction::Leave,
        Instruction::Return,
    ]);
    Ok(AssemblyFunction {
        symbol: symbol::array_clone(array),
        exported: false,
        instructions,
    })
}

fn lower_copier(
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
            } else {
                return Err(helper_error(format!(
                    "array {array} has a gated optional copy element {element}"
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
        MirType::Interface(_) | MirType::Obj | MirType::Unit => {
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

fn lower_destroyer(
    program: &MirProgram,
    array: crate::identity::ArrayTypeId,
    element: MirType,
    data_layout: &DataLayout,
) -> Result<AssemblyFunction, BackendError> {
    let layout = data_layout
        .array(array)
        .ok_or_else(|| helper_error(format!("array {array} has no destroy layout")))?;
    let displacement = i32::try_from(layout.element_offset())
        .map_err(|_| helper_error(format!("array {array} element offset cannot be encoded")))?;
    let (element_address, mut address_setup) = if matches!(layout.stride(), 1 | 2 | 4 | 8) {
        (
            value::indexed_memory(
                Register::Rdi,
                Register::Rsi,
                u8::try_from(layout.stride()).expect("encodable array stride"),
                displacement,
            ),
            Vec::new(),
        )
    } else {
        (
            value::memory(Register::Rdi, 0),
            materialize_destroy_element_address(layout.stride(), displacement),
        )
    };
    let mut instructions = match element {
        MirType::I64 | MirType::U64 | MirType::U8 | MirType::F64 | MirType::Bool => Vec::new(),
        MirType::Class(class) => vec![
            Instruction::LoadEffectiveAddress {
                source: element_address,
                destination: Register::Rdi,
            },
            call::direct_instruction(
                symbol::complete_finalizer(program, class),
                call::TraceAttribution::InheritedSourceOperation,
            ),
        ],
        MirType::Optional(optional) => {
            let metadata = program
                .optional_type(optional)
                .expect("verified optional array element metadata exists");
            if metadata.primitive().is_some() {
                Vec::new()
            } else if let Some(class) = metadata.inline_class() {
                let payload_offset = i32::try_from(
                    data_layout.optional_type(optional)?.payload_offset(),
                )
                .map_err(|_| helper_error("optional class payload offset exceeds x86-64"))?;
                let present = Label::new(format!(
                    ".Lska_array_{}_destroy_optional_present",
                    array.index()
                ));
                let complete = Label::new(format!(
                    ".Lska_array_{}_destroy_optional_complete",
                    array.index()
                ));
                vec![
                    Instruction::Move {
                        source: element_address,
                        destination: Register::Rax.into(),
                    },
                    Instruction::Test(Register::Rax),
                    Instruction::JumpIfNotZero(present.clone()),
                    Instruction::Jump(complete.clone()),
                    Instruction::Label(present),
                    Instruction::LoadEffectiveAddress {
                        source: offset_operand(element_address, payload_offset)?,
                        destination: Register::Rdi,
                    },
                    call::direct_instruction(
                        symbol::complete_finalizer(program, class),
                        call::TraceAttribution::InheritedSourceOperation,
                    ),
                    Instruction::Label(complete),
                ]
            } else if metadata.shared_owner().is_some() {
                let complete = Label::new(format!(
                    ".Lska_array_{}_destroy_optional_shared_complete",
                    array.index()
                ));
                vec![
                    Instruction::Move {
                        source: element_address,
                        destination: Register::Rdi.into(),
                    },
                    Instruction::Test(Register::Rdi),
                    Instruction::JumpIfEqual(complete.clone()),
                    call::direct_instruction(
                        symbol::shared_handle_release(),
                        call::TraceAttribution::InheritedSourceOperation,
                    ),
                    Instruction::Label(complete),
                ]
            } else {
                return Err(helper_error(format!(
                    "array {array} has a gated optional destruction element {element}"
                )));
            }
        }
        MirType::Array(inner) => vec![
            Instruction::Move {
                source: element_address,
                destination: Register::Rdi.into(),
            },
            call::direct_instruction(
                symbol::array_release(inner),
                call::TraceAttribution::InheritedSourceOperation,
            ),
        ],
        MirType::Shared(_) => vec![
            Instruction::Move {
                source: element_address,
                destination: Register::Rdi.into(),
            },
            call::direct_instruction(
                symbol::shared_handle_release(),
                call::TraceAttribution::InheritedSourceOperation,
            ),
        ],
        MirType::Interface(_) | MirType::Obj | MirType::Unit => {
            return Err(helper_error(format!(
                "array {array} has unsupported destruction element {element}"
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
        symbol: symbol::array_destroy_element(array),
        exported: false,
        instructions: address_setup,
    })
}

fn lower_release(array: crate::identity::ArrayTypeId) -> Result<AssemblyFunction, BackendError> {
    let stem = format!(".Lska_array_{}_release", array.index());
    let destroy_header = Label::new(format!("{stem}_destroy_header"));
    let destroy_body = Label::new(format!("{stem}_destroy_body"));
    let free = Label::new(format!("{stem}_free"));
    let complete = Label::new(format!("{stem}_complete"));
    let backing_home = value::memory(Register::Rbp, -8);
    let index_home = value::memory(Register::Rbp, -16);
    let instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::ReserveStack(16),
        Instruction::Test(Register::Rdi),
        Instruction::JumpIfEqual(complete.clone()),
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: backing_home,
        },
        Instruction::Move {
            source: value::memory(Register::Rdi, ARRAY_OWNER_COUNT_OFFSET),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        },
        Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: value::memory(Register::Rdi, ARRAY_OWNER_COUNT_OFFSET),
        },
        Instruction::Test(Register::Rax),
        Instruction::JumpIfNotZero(complete.clone()),
        Instruction::Move {
            source: value::memory(Register::Rdi, ARRAY_LENGTH_OFFSET),
            destination: Register::Rax.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: index_home,
        },
        Instruction::Label(destroy_header.clone()),
        Instruction::Move {
            source: index_home,
            destination: Register::Rax.into(),
        },
        Instruction::Test(Register::Rax),
        Instruction::JumpIfNotZero(destroy_body.clone()),
        Instruction::Jump(free.clone()),
        Instruction::Label(destroy_body),
        Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        },
        Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: index_home,
        },
        Instruction::Move {
            source: backing_home,
            destination: Register::Rdi.into(),
        },
        Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rsi.into(),
        },
        call::direct_instruction(
            symbol::array_destroy_element(array),
            call::TraceAttribution::InheritedSourceOperation,
        ),
        Instruction::Jump(destroy_header),
        Instruction::Label(free),
        Instruction::Move {
            source: backing_home,
            destination: Register::Rdi.into(),
        },
        call::direct_instruction(RUNTIME_FREE, call::TraceAttribution::HardDefectOnly),
        Instruction::Label(complete),
        Instruction::Leave,
        Instruction::Return,
    ];
    Ok(AssemblyFunction {
        symbol: symbol::array_release(array),
        exported: false,
        instructions,
    })
}

fn materialize_helper_element_addresses(stride: usize, displacement: i32) -> Vec<Instruction> {
    let stride = u64::try_from(stride).expect("array stride fits u64");
    vec![
        Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: stride,
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rdi, displacement),
            destination: Register::Rdi,
        },
        Instruction::Move {
            source: Register::Rcx.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: stride,
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rsi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rsi, displacement),
            destination: Register::Rsi,
        },
    ]
}

fn materialize_destroy_element_address(stride: usize, displacement: i32) -> Vec<Instruction> {
    vec![
        Instruction::Move {
            source: Register::Rsi.into(),
            destination: Register::Rax.into(),
        },
        Instruction::MoveImmediate64 {
            bits: u64::try_from(stride).expect("array stride fits u64"),
            destination: Register::R11,
        },
        Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        },
        Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        },
        Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rdi, displacement),
            destination: Register::Rdi,
        },
    ]
}

fn offset_operand(operand: Operand, offset: i32) -> Result<Operand, BackendError> {
    match operand {
        Operand::Memory { base, displacement } => Ok(Operand::Memory {
            base,
            displacement: displacement
                .checked_add(offset)
                .ok_or_else(|| helper_error("array helper displacement exceeds x86-64"))?,
        }),
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => Ok(Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement: displacement
                .checked_add(offset)
                .ok_or_else(|| helper_error("array helper displacement exceeds x86-64"))?,
        }),
        Operand::Register(_) => Err(helper_error(
            "array helper cannot offset a register operand",
        )),
    }
}

fn helper_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}
