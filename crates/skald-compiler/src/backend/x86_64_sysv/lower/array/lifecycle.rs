//! Raw-address lifecycle helpers used by generated array copy loops.
//!
//! Ordinary MIR lowering works with semantic places. Generated array clone
//! helpers instead receive element addresses directly, so synthesized class
//! copying needs one private address-based wrapper per copyable exact class.
//! Calls between wrappers deliberately break recursive class/array graphs.

use crate::{
    backend::{BackendError, Target},
    identity::ClassId,
    mir::{MirCopyCapability, MirProgram, MirSynthesizedFieldCopy, MirType},
};

use super::super::super::{
    layout::DataLayout,
    machine::{AssemblyFunction, ByteRegister, Instruction, Label, Operand, Register},
    symbol,
};
use super::super::{ownership::emit_retain_loaded_handle, value};

const DESTINATION_HOME: i32 = -8;
const SOURCE_HOME: i32 = -16;
const FRAME_SIZE: u32 = 16;

pub(super) fn lower_class_copy_helpers(
    program: &MirProgram,
    data_layout: &DataLayout,
) -> Result<Vec<AssemblyFunction>, BackendError> {
    let required = required_class_copy_helpers(program);
    program
        .classes
        .iter()
        .filter(|class| required[class.id.index()])
        .map(|class| lower_class_copy(program, data_layout, class.id))
        .collect()
}

fn required_class_copy_helpers(program: &MirProgram) -> Vec<bool> {
    let mut required = vec![false; program.classes.len()];
    let mut pending = Vec::new();
    for array in program.array_types.iter() {
        if let MirType::Class(class) | MirType::OptionalClass(class) = array.element {
            pending.push(class);
        }
    }

    while let Some(class) = pending.pop() {
        if std::mem::replace(&mut required[class.index()], true) {
            continue;
        }
        let declaration = program
            .class(class)
            .expect("verified class copy target is declared");
        match &declaration.copy_constructor {
            MirCopyCapability::User(copy) => {
                if let Some(base) = copy.base {
                    pending.push(base.base);
                }
            }
            MirCopyCapability::Synthesized(copy) => {
                if let Some(base) = copy.base {
                    pending.push(base.base);
                }
                for field in &copy.fields {
                    match program
                        .field(field.field())
                        .expect("verified synthesized field is declared")
                        .ty
                    {
                        MirType::Class(class) | MirType::OptionalClass(class) => {
                            pending.push(class);
                        }
                        _ => {}
                    }
                }
            }
            MirCopyCapability::Unavailable => {
                unreachable!("array element capabilities exclude unavailable class copying")
            }
        }
    }
    required
}

fn lower_class_copy(
    program: &MirProgram,
    data_layout: &DataLayout,
    class: ClassId,
) -> Result<AssemblyFunction, BackendError> {
    let mut output = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::ReserveStack(FRAME_SIZE),
        Instruction::Move {
            source: Register::Rdi.into(),
            destination: memory(Register::Rbp, DESTINATION_HOME),
        },
        Instruction::Move {
            source: Register::Rsi.into(),
            destination: memory(Register::Rbp, SOURCE_HOME),
        },
    ];
    let declaration = program
        .class(class)
        .ok_or_else(|| lifecycle_error(format!("unknown class copy helper target {class}")))?;

    match &declaration.copy_constructor {
        MirCopyCapability::User(copy) => {
            if let Some(base) = copy.base {
                emit_class_helper_call(data_layout, class, base.base, &mut output)?;
            }
            load_home_address(DESTINATION_HOME, 0, Register::Rdi, &mut output);
            load_home_address(DESTINATION_HOME, 0, Register::Rsi, &mut output);
            output.push(Instruction::LoadSymbolAddress {
                symbol: symbol::dispatch_table(class),
                destination: Register::Rdx,
            });
            load_home_address(SOURCE_HOME, 0, Register::Rcx, &mut output);
            load_home_address(SOURCE_HOME, 0, Register::R8, &mut output);
            output.push(Instruction::LoadSymbolAddress {
                symbol: symbol::dispatch_table(class),
                destination: Register::R9,
            });
            output.push(Instruction::Call(symbol::callable(
                program,
                copy.operation.into(),
            )));
        }
        MirCopyCapability::Synthesized(copy) => {
            if let Some(base) = copy.base {
                emit_class_helper_call(data_layout, class, base.base, &mut output)?;
            }
            for field in &copy.fields {
                emit_field_copy(program, data_layout, class, *field, &mut output)?;
            }
        }
        MirCopyCapability::Unavailable => {
            unreachable!("unavailable class copy helpers are not emitted")
        }
    }

    output.extend([Instruction::Leave, Instruction::Return]);
    Ok(AssemblyFunction {
        symbol: symbol::class_copy_helper(class),
        exported: false,
        instructions: output,
    })
}

fn emit_field_copy<I: Copy>(
    program: &MirProgram,
    data_layout: &DataLayout,
    owner: ClassId,
    field: MirSynthesizedFieldCopy<I>,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let field_id = field.field();
    let offset = field_offset(data_layout, field_id)?;
    let ty = program
        .field(field_id)
        .ok_or_else(|| lifecycle_error(format!("unknown synthesized field {field_id}")))?
        .ty;
    match field {
        MirSynthesizedFieldCopy::Primitive { .. } => {
            emit_scalar_copy(ty, offset, output);
        }
        MirSynthesizedFieldCopy::OptionalPrimitive { payload, .. } => {
            let layout = data_layout.optional(payload)?;
            emit_optional_copy(
                owner,
                field_id,
                offset,
                i32::try_from(layout.payload_offset())
                    .map_err(|_| lifecycle_error("optional payload offset exceeds x86-64"))?,
                payload.payload_type(),
                output,
            );
        }
        MirSynthesizedFieldCopy::Class { .. } => {
            let MirType::Class(class) = ty else {
                unreachable!("verified synthesized class field has class type")
            };
            emit_helper_call_at(class, offset, offset, output);
        }
        MirSynthesizedFieldCopy::OptionalClass { class, .. } => {
            let payload = i32::try_from(data_layout.optional_class(class)?.payload_offset())
                .map_err(|_| lifecycle_error("optional class payload offset exceeds x86-64"))?;
            emit_optional_class_copy(owner, field_id, class, offset, payload, output);
        }
        MirSynthesizedFieldCopy::Array { array, .. } => {
            load_home_address(SOURCE_HOME, offset, Register::R11, output);
            output.push(Instruction::Move {
                source: memory(Register::R11, 0),
                destination: Register::Rdi.into(),
            });
            output.push(Instruction::Call(symbol::array_clone(array)));
            load_home_address(DESTINATION_HOME, offset, Register::R11, output);
            value::store_rax(memory(Register::R11, 0), output);
        }
        MirSynthesizedFieldCopy::Shared { .. } => {
            emit_shared_copy(owner, field_id, offset, false, output);
        }
        MirSynthesizedFieldCopy::OptionalShared { .. } => {
            emit_shared_copy(owner, field_id, offset, true, output);
        }
    }
    Ok(())
}

fn emit_class_helper_call(
    data_layout: &DataLayout,
    owner: ClassId,
    class: ClassId,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let offset = data_layout
        .class(owner)
        .and_then(|layout| layout.base())
        .filter(|base| base.class == class)
        .ok_or_else(|| lifecycle_error(format!("class {owner} has no direct base {class}")))?
        .offset;
    let offset =
        i32::try_from(offset).map_err(|_| lifecycle_error("base copy offset exceeds x86-64"))?;
    emit_helper_call_at(class, offset, offset, output);
    Ok(())
}

fn emit_helper_call_at(
    class: ClassId,
    destination_offset: i32,
    source_offset: i32,
    output: &mut Vec<Instruction>,
) {
    load_home_address(DESTINATION_HOME, destination_offset, Register::Rdi, output);
    load_home_address(SOURCE_HOME, source_offset, Register::Rsi, output);
    output.push(Instruction::Call(symbol::class_copy_helper(class)));
}

fn emit_optional_copy(
    owner: ClassId,
    field: crate::identity::FieldId,
    offset: i32,
    payload_offset: i32,
    payload: MirType,
    output: &mut Vec<Instruction>,
) {
    let stem = format!(
        ".Lska_class_{}_field_{}_optional_copy",
        owner.index(),
        field.index()
    );
    let present = Label::new(format!("{stem}_present"));
    let complete = Label::new(format!("{stem}_complete"));
    load_home_address(SOURCE_HOME, offset, Register::R11, output);
    output.push(Instruction::Move {
        source: memory(Register::R11, 0),
        destination: Register::Rax.into(),
    });
    output.push(Instruction::Test(Register::Rax));
    output.push(Instruction::JumpIfNotZero(present.clone()));
    load_home_address(DESTINATION_HOME, offset, Register::R11, output);
    value::store_rax(memory(Register::R11, 0), output);
    output.push(Instruction::Jump(complete.clone()));
    output.push(Instruction::Label(present));
    emit_scalar_copy(
        payload,
        offset
            .checked_add(payload_offset)
            .expect("verified optional payload offset fits"),
        output,
    );
    load_home_address(DESTINATION_HOME, offset, Register::R11, output);
    output.push(Instruction::MoveImmediate64 {
        bits: 1,
        destination: Register::Rax,
    });
    value::store_rax(memory(Register::R11, 0), output);
    output.push(Instruction::Label(complete));
}

fn emit_optional_class_copy(
    owner: ClassId,
    field: crate::identity::FieldId,
    class: ClassId,
    offset: i32,
    payload_offset: i32,
    output: &mut Vec<Instruction>,
) {
    let stem = format!(
        ".Lska_class_{}_field_{}_optional_copy",
        owner.index(),
        field.index()
    );
    let present = Label::new(format!("{stem}_present"));
    let complete = Label::new(format!("{stem}_complete"));
    load_home_address(SOURCE_HOME, offset, Register::R11, output);
    output.push(Instruction::Move {
        source: memory(Register::R11, 0),
        destination: Register::Rax.into(),
    });
    output.push(Instruction::Test(Register::Rax));
    output.push(Instruction::JumpIfNotZero(present.clone()));
    load_home_address(DESTINATION_HOME, offset, Register::R11, output);
    value::store_rax(memory(Register::R11, 0), output);
    output.push(Instruction::Jump(complete.clone()));
    output.push(Instruction::Label(present));
    emit_helper_call_at(
        class,
        offset
            .checked_add(payload_offset)
            .expect("verified optional payload offset fits"),
        offset
            .checked_add(payload_offset)
            .expect("verified optional payload offset fits"),
        output,
    );
    load_home_address(DESTINATION_HOME, offset, Register::R11, output);
    output.push(Instruction::MoveImmediate64 {
        bits: 1,
        destination: Register::Rax,
    });
    value::store_rax(memory(Register::R11, 0), output);
    output.push(Instruction::Label(complete));
}

fn emit_shared_copy(
    owner: ClassId,
    field: crate::identity::FieldId,
    offset: i32,
    optional: bool,
    output: &mut Vec<Instruction>,
) {
    let stem = format!(
        ".Lska_class_{}_field_{}_shared_copy",
        owner.index(),
        field.index()
    );
    let absent = Label::new(format!("{stem}_absent"));
    let invalid = Label::new(format!("{stem}_invalid"));
    let overflow = Label::new(format!("{stem}_overflow"));
    let complete = Label::new(format!("{stem}_complete"));
    load_home_address(SOURCE_HOME, offset, Register::R11, output);
    value::load_rax(memory(Register::R11, 0), output);
    if optional {
        output.push(Instruction::Test(Register::Rax));
        output.push(Instruction::JumpIfEqual(absent.clone()));
    }
    emit_retain_loaded_handle(invalid.clone(), overflow.clone(), output);
    load_home_address(DESTINATION_HOME, offset, Register::R11, output);
    value::store_rax(memory(Register::R11, 0), output);
    output.push(Instruction::Jump(complete.clone()));
    if optional {
        output.push(Instruction::Label(absent));
        load_home_address(DESTINATION_HOME, offset, Register::R11, output);
        value::store_rax(memory(Register::R11, 0), output);
        output.push(Instruction::Jump(complete.clone()));
    }
    output.push(Instruction::Label(overflow));
    super::super::terminator::emit_ownership_overflow(output);
    output.push(Instruction::Label(invalid));
    // Generated field lifecycle code receives only verified live handles.
    output.push(Instruction::Trap);
    output.push(Instruction::Label(complete));
}

fn emit_scalar_copy(ty: MirType, offset: i32, output: &mut Vec<Instruction>) {
    load_home_address(SOURCE_HOME, offset, Register::R11, output);
    match ty {
        MirType::U8 | MirType::Bool => {
            output.push(Instruction::LoadZeroExtendByte {
                source: memory(Register::R11, 0),
                destination: Register::Rax,
            });
            load_home_address(DESTINATION_HOME, offset, Register::R11, output);
            output.push(Instruction::MoveByte {
                source: ByteRegister::Al,
                destination: memory(Register::R11, 0),
            });
        }
        MirType::I64 | MirType::U64 | MirType::F64 => {
            value::load_rax(memory(Register::R11, 0), output);
            load_home_address(DESTINATION_HOME, offset, Register::R11, output);
            value::store_rax(memory(Register::R11, 0), output);
        }
        _ => unreachable!("verified scalar copy has a primitive payload"),
    }
}

fn field_offset(
    data_layout: &DataLayout,
    field: crate::identity::FieldId,
) -> Result<i32, BackendError> {
    i32::try_from(
        data_layout
            .field(field)
            .ok_or_else(|| lifecycle_error(format!("field {field} has no target layout")))?
            .offset,
    )
    .map_err(|_| lifecycle_error(format!("field {field} offset exceeds x86-64")))
}

fn load_home_address(home: i32, offset: i32, register: Register, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source: memory(Register::Rbp, home),
        destination: register.into(),
    });
    if offset != 0 {
        output.push(Instruction::LoadEffectiveAddress {
            source: memory(register, offset),
            destination: register,
        });
    }
}

const fn memory(base: Register, displacement: i32) -> Operand {
    Operand::Memory { base, displacement }
}

fn lifecycle_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}
