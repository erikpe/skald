//! Instruction selection and ABI lowering into the target assembly model.

use crate::{
    backend::{BackendError, Target},
    mir::{
        BlockId, MirBinaryOperation, MirCall, MirCallTarget, MirFunctionDeclaration,
        MirFunctionDefinition, MirFunctionLinkage, MirInstruction, MirProgram, MirRvalueKind,
        MirTerminator, MirType, MirUnaryOperation, StorageId, ValueId,
    },
};

use super::{
    abi::{self, IncomingArgument},
    frame::FrameLayout,
    machine::{
        AssemblyFunction, AssemblyProgram, ByteRegister, Instruction, Label, Operand, Register,
    },
};

pub(super) fn lower(program: &MirProgram) -> Result<AssemblyProgram, BackendError> {
    let mut functions = program
        .definitions
        .iter()
        .map(|function| {
            let declaration = program
                .declarations
                .get(function.function)
                .expect("verified definition must have a declaration");
            lower_function(program, declaration, function)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let entry = program
        .declarations
        .get(program.entry_function)
        .expect("verified entry declaration must exist");
    functions.push(entry_wrapper(entry));
    Ok(AssemblyProgram { functions })
}

fn lower_function(
    program: &MirProgram,
    declaration: &MirFunctionDeclaration,
    function: &MirFunctionDefinition,
) -> Result<AssemblyFunction, BackendError> {
    let frame = FrameLayout::plan(function)?;
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
    ];
    if frame.size() != 0 {
        instructions.push(Instruction::ReserveStack(frame.size()));
    }

    spill_parameters(function, &frame, &mut instructions)?;
    if function.body.blocks[0].id != function.body.entry {
        instructions.push(Instruction::Jump(block_label(function.body.entry)));
    }
    let epilogue = epilogue_label(function.function);
    for block in &function.body.blocks {
        instructions.push(Instruction::Label(block_label(block.id)));
        for instruction in &block.instructions {
            select_instruction(program, function, instruction, &frame, &mut instructions)?;
        }
        select_terminator(
            block
                .terminator
                .as_ref()
                .expect("verified block is terminated"),
            &frame,
            declaration.return_type,
            &epilogue,
            &mut instructions,
        );
    }
    instructions.push(Instruction::Label(epilogue));
    instructions.push(Instruction::Leave);
    instructions.push(Instruction::Return);

    Ok(AssemblyFunction {
        symbol: symbol_for(declaration),
        exported: false,
        instructions,
    })
}

/// C-compatible process entry boundary. Returning the Skald `i64` in `%rax`
/// exposes its low 32 bits as C `main`'s `int`; Linux subsequently observes
/// the low eight bits as the process exit status.
fn entry_wrapper(entry: &MirFunctionDeclaration) -> AssemblyFunction {
    AssemblyFunction {
        symbol: "main".to_owned(),
        exported: true,
        instructions: vec![
            Instruction::Push(Register::Rbp),
            Instruction::Move {
                source: Register::Rsp.into(),
                destination: Register::Rbp.into(),
            },
            Instruction::Call(symbol_for(entry)),
            Instruction::Leave,
            Instruction::Return,
        ],
    }
}

fn spill_parameters(
    function: &MirFunctionDefinition,
    frame: &FrameLayout,
    instructions: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    for (index, storage) in function.parameters.iter().enumerate() {
        let incoming = abi::incoming_argument(index).ok_or_else(|| {
            BackendError::new(
                Target::X86_64SysV,
                Some(function.function),
                "incoming argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;
        let ty = function
            .storage(*storage)
            .expect("verified parameter storage must exist")
            .ty;
        let destination = frame_storage(frame, *storage);
        if ty == MirType::U8 {
            match incoming {
                IncomingArgument::Register(register) => load_rax(register.into(), instructions),
                IncomingArgument::Stack(displacement) => {
                    load_rax(memory(Register::Rbp, displacement), instructions)
                }
            }
            store_canonical_rax(ty, destination, instructions);
            continue;
        }
        match incoming {
            IncomingArgument::Register(register) => instructions.push(Instruction::Move {
                source: register.into(),
                destination,
            }),
            IncomingArgument::Stack(displacement) => {
                instructions.push(Instruction::Move {
                    source: memory(Register::Rbp, displacement),
                    destination: Register::Rax.into(),
                });
                instructions.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination,
                });
            }
        }
    }
    Ok(())
}

fn select_instruction(
    program: &MirProgram,
    function: &MirFunctionDefinition,
    instruction: &MirInstruction,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    match instruction {
        MirInstruction::Assign(assignment) => {
            let destination = frame_value(frame, assignment.result);
            match &assignment.rvalue.kind {
                MirRvalueKind::ConstantI64(value) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: *value as u64,
                        destination: Register::Rax,
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::ConstantU64(value) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: *value,
                        destination: Register::Rax,
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::ConstantU8(value) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: u64::from(*value),
                        destination: Register::Rax,
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::ConstantBool(value) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: u64::from(*value),
                        destination: Register::Rax,
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::Load(storage) => {
                    load_rax(frame_storage(frame, *storage), output);
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::Unary {
                    operation: MirUnaryOperation::NegateI64,
                    operand,
                } => {
                    load_rax(frame_value(frame, *operand), output);
                    output.push(Instruction::Negate(Register::Rax));
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::Binary {
                    operation,
                    left,
                    right,
                } => {
                    load_rax(frame_value(frame, *left), output);
                    output.push(Instruction::Move {
                        source: frame_value(frame, *right),
                        destination: Register::Rcx.into(),
                    });
                    output.push(match operation {
                        MirBinaryOperation::AddI64 | MirBinaryOperation::AddU64 => {
                            Instruction::Add {
                                source: Register::Rcx,
                                destination: Register::Rax,
                            }
                        }
                        MirBinaryOperation::SubtractI64 | MirBinaryOperation::SubtractU64 => {
                            Instruction::Subtract {
                                source: Register::Rcx,
                                destination: Register::Rax,
                            }
                        }
                        MirBinaryOperation::MultiplyI64 | MirBinaryOperation::MultiplyU64 => {
                            Instruction::Multiply {
                                source: Register::Rcx,
                                destination: Register::Rax,
                            }
                        }
                        MirBinaryOperation::AddU8 => Instruction::Add {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                        MirBinaryOperation::SubtractU8 => Instruction::Subtract {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                        MirBinaryOperation::MultiplyU8 => Instruction::Multiply {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
            }
        }
        MirInstruction::Call(call) => {
            select_call(program, function, call, frame, output)?;
        }
        MirInstruction::Store(store) => {
            load_rax(frame_value(frame, store.value), output);
            let ty = function
                .storage(store.storage)
                .expect("verified store target must exist")
                .ty;
            store_canonical_rax(ty, frame_storage(frame, store.storage), output);
        }
    }
    Ok(())
}

fn select_call(
    program: &MirProgram,
    function: &MirFunctionDefinition,
    call: &MirCall,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let MirCallTarget::Direct(target_id) = call.target;
    let target = program
        .declarations
        .get(target_id)
        .expect("verified call target must be declared");
    let arguments = &call.arguments;
    let stack_size = abi::outgoing_stack_size(arguments.len()).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            Some(function.function),
            "outgoing argument area exceeds the x86-64 ABI encoding limits",
        )
    })?;
    if stack_size != 0 {
        output.push(Instruction::ReserveStack(stack_size));
    }

    for (index, argument) in arguments
        .iter()
        .enumerate()
        .skip(abi::ARGUMENT_REGISTERS.len())
    {
        let displacement = abi::outgoing_argument_offset(index).expect("target legality checked");
        load_rax(frame_value(frame, *argument), output);
        store_rax(memory(Register::Rsp, displacement), output);
    }
    for (register, argument) in abi::ARGUMENT_REGISTERS.iter().zip(arguments) {
        output.push(Instruction::Move {
            source: frame_value(frame, *argument),
            destination: (*register).into(),
        });
    }

    output.push(Instruction::Call(symbol_for(target)));
    if stack_size != 0 {
        output.push(Instruction::ReleaseStack(stack_size));
    }
    if target.return_type == MirType::Bool
        && matches!(target.linkage, MirFunctionLinkage::External { .. })
    {
        output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
    }
    if let Some(result) = call.result {
        store_canonical_rax(target.return_type, frame_value(frame, result), output);
    }
    Ok(())
}

fn select_terminator(
    terminator: &MirTerminator,
    frame: &FrameLayout,
    return_type: MirType,
    epilogue: &Label,
    output: &mut Vec<Instruction>,
) {
    match terminator {
        MirTerminator::Return { value, .. } => {
            if let Some(value) = value {
                load_rax(frame_value(frame, *value), output);
                canonicalize_rax(return_type, output);
            }
            output.push(Instruction::Jump(epilogue.clone()));
        }
        MirTerminator::Goto { target, .. } => {
            output.push(Instruction::Jump(block_label(*target)));
        }
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            ..
        } => {
            load_rax(frame_value(frame, *condition), output);
            output.push(Instruction::Test(Register::Rax));
            output.push(Instruction::JumpIfNotZero(block_label(*true_target)));
            output.push(Instruction::Jump(block_label(*false_target)));
        }
    }
}

fn load_rax(source: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source,
        destination: Register::Rax.into(),
    });
}

/// Converts a MIR value in `%rax` to its canonical full-register form.
///
/// `u8` values use eight-byte homes in the initial backend, but only their low
/// eight bits belong to the language value. Every producer and ABI ingress
/// reaches this helper before the value is stored or returned.
fn canonicalize_rax(ty: MirType, output: &mut Vec<Instruction>) {
    if ty == MirType::U8 {
        output.push(Instruction::ZeroExtendByte {
            source: ByteRegister::Al,
            destination: Register::Rax,
        });
    }
}

fn store_canonical_rax(ty: MirType, destination: Operand, output: &mut Vec<Instruction>) {
    canonicalize_rax(ty, output);
    store_rax(destination, output);
}

fn store_rax(destination: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source: Register::Rax.into(),
        destination,
    });
}

fn frame_storage(frame: &FrameLayout, storage: StorageId) -> Operand {
    memory(Register::Rbp, frame.storage(storage))
}

fn frame_value(frame: &FrameLayout, value: ValueId) -> Operand {
    memory(Register::Rbp, frame.value(value))
}

fn memory(base: Register, displacement: i32) -> Operand {
    Operand::Memory { base, displacement }
}

fn symbol_for(function: &MirFunctionDeclaration) -> String {
    match &function.linkage {
        MirFunctionLinkage::Internal => format!(".Lska_fn_{}", function.id.index()),
        MirFunctionLinkage::External { symbol } => symbol.clone(),
    }
}

fn block_label(block: BlockId) -> Label {
    Label::new(format!(
        ".Lska_fn_{}_block_{}",
        block.function().index(),
        block.index()
    ))
}

fn epilogue_label(function: crate::resolve::FunctionId) -> Label {
    Label::new(format!(".Lska_fn_{}_epilogue", function.index()))
}
