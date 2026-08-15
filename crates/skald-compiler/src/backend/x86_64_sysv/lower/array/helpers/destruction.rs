//! Element destruction, array release, and shared finalization helpers.

use crate::{
    backend::{
        x86_64_sysv::{
            layout::{
                DataLayout, ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET, SHARED_HEADER_SIZE,
            },
            machine::{AssemblyFunction, Instruction, Label, Operand, Register},
            symbol,
        },
        BackendError,
    },
    mir::{MirProgram, MirType},
};

use super::{helper_error, materialize_destroy_element_address, offset_operand, RUNTIME_FREE};
use crate::backend::x86_64_sysv::lower::{call, value};

pub(super) fn lower_shared_finalizer(
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
            let payload = i64::try_from(SHARED_HEADER_SIZE).ok()?;
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
fn lower_aggregate_optional_destroyer(
    program: &MirProgram,
    array: crate::identity::ArrayTypeId,
    optional: crate::identity::OptionalTypeId,
    address: Operand,
    data_layout: &DataLayout,
) -> Result<Vec<Instruction>, BackendError> {
    let metadata = program
        .optional_type(optional)
        .expect("verified optional metadata");
    match metadata.storage {
        crate::mir::MirOptionalStorage::Scalar => Ok(Vec::new()),
        crate::mir::MirOptionalStorage::SharedOwner(_) => {
            let complete = Label::new(format!(
                ".Lska_array_{}_destroy_o{}_complete",
                array.index(),
                optional.index()
            ));
            Ok(vec![
                Instruction::Move {
                    source: address,
                    destination: Register::Rdi.into(),
                },
                Instruction::Test(Register::Rdi),
                Instruction::JumpIfEqual(complete.clone()),
                call::direct_instruction(
                    symbol::shared_handle_release(),
                    call::TraceAttribution::InheritedSourceOperation,
                ),
                Instruction::Label(complete),
            ])
        }
        crate::mir::MirOptionalStorage::InlineClass(class) => {
            let payload_offset =
                i32::try_from(data_layout.optional_type(optional)?.payload_offset())
                    .map_err(|_| helper_error("optional payload offset exceeds x86-64"))?;
            let complete = Label::new(format!(
                ".Lska_array_{}_destroy_o{}_complete",
                array.index(),
                optional.index()
            ));
            Ok(vec![
                Instruction::Move {
                    source: address,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfEqual(complete.clone()),
                Instruction::LoadEffectiveAddress {
                    source: offset_operand(address, payload_offset)?,
                    destination: Register::Rdi,
                },
                call::direct_instruction(
                    symbol::complete_finalizer(program, class),
                    call::TraceAttribution::InheritedSourceOperation,
                ),
                Instruction::Label(complete),
            ])
        }
        crate::mir::MirOptionalStorage::Nested(nested) => {
            let payload_offset =
                i32::try_from(data_layout.optional_type(optional)?.payload_offset())
                    .map_err(|_| helper_error("nested optional payload offset exceeds x86-64"))?;
            let complete = Label::new(format!(
                ".Lska_array_{}_destroy_o{}_complete",
                array.index(),
                optional.index()
            ));
            let mut output = vec![
                Instruction::Move {
                    source: address,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfEqual(complete.clone()),
            ];
            output.extend(lower_aggregate_optional_destroyer(
                program,
                array,
                nested,
                offset_operand(address, payload_offset)?,
                data_layout,
            )?);
            output.push(Instruction::Label(complete));
            Ok(output)
        }
        crate::mir::MirOptionalStorage::InlineArray(inner) => {
            let payload_offset =
                i32::try_from(data_layout.optional_type(optional)?.payload_offset())
                    .map_err(|_| helper_error("optional array payload offset exceeds x86-64"))?;
            let complete = Label::new(format!(
                ".Lska_array_{}_destroy_o{}_complete",
                array.index(),
                optional.index()
            ));
            Ok(vec![
                Instruction::Move {
                    source: address,
                    destination: Register::Rax.into(),
                },
                Instruction::Test(Register::Rax),
                Instruction::JumpIfEqual(complete.clone()),
                Instruction::Move {
                    source: offset_operand(address, payload_offset)?,
                    destination: Register::Rdi.into(),
                },
                call::direct_instruction(
                    symbol::array_release(inner),
                    call::TraceAttribution::InheritedSourceOperation,
                ),
                Instruction::Label(complete),
            ])
        }
    }
}

pub(super) fn lower_destroyer(
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
            } else if matches!(
                metadata.storage,
                crate::mir::MirOptionalStorage::Nested(_)
                    | crate::mir::MirOptionalStorage::InlineArray(_)
            ) {
                lower_aggregate_optional_destroyer(
                    program,
                    array,
                    optional,
                    element_address,
                    data_layout,
                )?
            } else {
                return Err(helper_error(format!(
                    "array {array} has an unsupported optional destruction element {element}"
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
        MirType::Function(_) | MirType::Interface(_) | MirType::Obj | MirType::Unit => {
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

pub(super) fn lower_release(
    array: crate::identity::ArrayTypeId,
) -> Result<AssemblyFunction, BackendError> {
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
