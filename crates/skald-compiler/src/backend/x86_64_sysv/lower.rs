//! Instruction selection and ABI lowering into the target assembly model.

use crate::{
    backend::{BackendError, Target},
    identity::FunctionId,
    mir::{
        BlockId, MirBinaryOperation, MirCall, MirCallTarget, MirFunctionDeclaration,
        MirFunctionDefinition, MirFunctionLinkage, MirInstruction, MirProgram, MirRvalueKind,
        MirTerminator, MirType, MirUnaryOperation, StorageId, ValueId,
    },
};

use super::{
    abi::{ArgumentLocation, CallLayout},
    frame::FrameLayout,
    machine::{
        AssemblyFunction, AssemblyProgram, ByteRegister, FloatOperand, Instruction, Label, Operand,
        Register, XmmRegister,
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

    spill_parameters(declaration, function, &frame, &mut instructions)?;
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
    declaration: &MirFunctionDeclaration,
    function: &MirFunctionDefinition,
    frame: &FrameLayout,
    instructions: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let layout = CallLayout::classify(&declaration.parameter_types).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            Some(function.function),
            "incoming argument area exceeds the x86-64 ABI encoding limits",
        )
    })?;
    for (storage, location) in function.parameters.iter().zip(layout.locations()) {
        let incoming = location.incoming().ok_or_else(|| {
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
        match incoming {
            ArgumentLocation::IntegerRegister(register) => {
                if ty == MirType::U8 {
                    load_rax(register.into(), instructions);
                    store_canonical_rax(ty, destination, instructions);
                } else {
                    instructions.push(Instruction::Move {
                        source: register.into(),
                        destination,
                    });
                }
            }
            ArgumentLocation::SseRegister(register) => {
                instructions.push(Instruction::MoveFloat64 {
                    source: register.into(),
                    destination: float_operand(destination),
                })
            }
            ArgumentLocation::Stack(displacement) => {
                if ty == MirType::F64 {
                    load_float(
                        float_memory(Register::Rbp, displacement),
                        XmmRegister::Xmm14,
                        instructions,
                    );
                    store_float(XmmRegister::Xmm14, float_operand(destination), instructions);
                } else {
                    load_rax(memory(Register::Rbp, displacement), instructions);
                    store_canonical_rax(ty, destination, instructions);
                }
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
                MirRvalueKind::ConstantF64Bits(bits) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: *bits,
                        destination: Register::Rax,
                    });
                    output.push(Instruction::MoveBitsToFloat {
                        source: Register::Rax,
                        destination: XmmRegister::Xmm14,
                    });
                    store_float(XmmRegister::Xmm14, float_operand(destination), output);
                }
                MirRvalueKind::ConstantBool(value) => {
                    output.push(Instruction::MoveImmediate64 {
                        bits: u64::from(*value),
                        destination: Register::Rax,
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
                MirRvalueKind::Load(storage) => {
                    if assignment.rvalue.ty == MirType::F64 {
                        load_float(
                            float_operand(frame_storage(frame, *storage)),
                            XmmRegister::Xmm14,
                            output,
                        );
                        store_float(XmmRegister::Xmm14, float_operand(destination), output);
                    } else {
                        load_rax(frame_storage(frame, *storage), output);
                        store_canonical_rax(assignment.rvalue.ty, destination, output);
                    }
                }
                MirRvalueKind::Unary { operation, operand } => match operation {
                    MirUnaryOperation::NegateI64 => {
                        load_rax(frame_value(frame, *operand), output);
                        output.push(Instruction::Negate(Register::Rax));
                        store_canonical_rax(assignment.rvalue.ty, destination, output);
                    }
                    MirUnaryOperation::NegateF64 => {
                        load_float(
                            float_operand(frame_value(frame, *operand)),
                            XmmRegister::Xmm14,
                            output,
                        );
                        output.push(Instruction::MoveImmediate64 {
                            bits: 1_u64 << 63,
                            destination: Register::Rax,
                        });
                        output.push(Instruction::MoveBitsToFloat {
                            source: Register::Rax,
                            destination: XmmRegister::Xmm15,
                        });
                        output.push(Instruction::XorFloat128 {
                            source: XmmRegister::Xmm15,
                            destination: XmmRegister::Xmm14,
                        });
                        store_float(XmmRegister::Xmm14, float_operand(destination), output);
                    }
                },
                MirRvalueKind::Binary {
                    operation,
                    left,
                    right,
                } => {
                    if operation.operand_type() == MirType::F64 {
                        load_float(
                            float_operand(frame_value(frame, *left)),
                            XmmRegister::Xmm14,
                            output,
                        );
                        load_float(
                            float_operand(frame_value(frame, *right)),
                            XmmRegister::Xmm15,
                            output,
                        );
                        output.push(match operation {
                            MirBinaryOperation::AddF64 => Instruction::AddFloat64 {
                                source: XmmRegister::Xmm15,
                                destination: XmmRegister::Xmm14,
                            },
                            MirBinaryOperation::SubtractF64 => Instruction::SubtractFloat64 {
                                source: XmmRegister::Xmm15,
                                destination: XmmRegister::Xmm14,
                            },
                            MirBinaryOperation::MultiplyF64 => Instruction::MultiplyFloat64 {
                                source: XmmRegister::Xmm15,
                                destination: XmmRegister::Xmm14,
                            },
                            _ => unreachable!("f64 type implies an f64 operation"),
                        });
                        store_float(XmmRegister::Xmm14, float_operand(destination), output);
                        return Ok(());
                    }
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
                        MirBinaryOperation::AddF64
                        | MirBinaryOperation::SubtractF64
                        | MirBinaryOperation::MultiplyF64 => {
                            unreachable!("f64 operations are selected above")
                        }
                    });
                    store_canonical_rax(assignment.rvalue.ty, destination, output);
                }
            }
        }
        MirInstruction::Call(call) => {
            select_call(program, function, call, frame, output)?;
        }
        MirInstruction::Store(store) => {
            let ty = function
                .storage(store.storage)
                .expect("verified store target must exist")
                .ty;
            if ty == MirType::F64 {
                load_float(
                    float_operand(frame_value(frame, store.value)),
                    XmmRegister::Xmm14,
                    output,
                );
                store_float(
                    XmmRegister::Xmm14,
                    float_operand(frame_storage(frame, store.storage)),
                    output,
                );
            } else {
                load_rax(frame_value(frame, store.value), output);
                store_canonical_rax(ty, frame_storage(frame, store.storage), output);
            }
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
    let layout = CallLayout::classify(&target.parameter_types).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            Some(function.function),
            "outgoing argument area exceeds the x86-64 ABI encoding limits",
        )
    })?;
    if layout.stack_size() != 0 {
        output.push(Instruction::ReserveStack(layout.stack_size()));
    }

    for ((argument, ty), location) in arguments
        .iter()
        .zip(&target.parameter_types)
        .zip(layout.locations())
    {
        match location {
            ArgumentLocation::IntegerRegister(register) => output.push(Instruction::Move {
                source: frame_value(frame, *argument),
                destination: (*register).into(),
            }),
            ArgumentLocation::SseRegister(register) => {
                load_float(
                    float_operand(frame_value(frame, *argument)),
                    *register,
                    output,
                );
            }
            ArgumentLocation::Stack(displacement) if *ty == MirType::F64 => {
                load_float(
                    float_operand(frame_value(frame, *argument)),
                    XmmRegister::Xmm14,
                    output,
                );
                store_float(
                    XmmRegister::Xmm14,
                    float_memory(Register::Rsp, *displacement),
                    output,
                );
            }
            ArgumentLocation::Stack(displacement) => {
                load_rax(frame_value(frame, *argument), output);
                store_rax(memory(Register::Rsp, *displacement), output);
            }
        }
    }

    output.push(Instruction::Call(symbol_for(target)));
    if layout.stack_size() != 0 {
        output.push(Instruction::ReleaseStack(layout.stack_size()));
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
        if target.return_type == MirType::F64 {
            store_float(
                XmmRegister::Xmm0,
                float_operand(frame_value(frame, result)),
                output,
            );
        } else {
            store_canonical_rax(target.return_type, frame_value(frame, result), output);
        }
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
                if return_type == MirType::F64 {
                    load_float(
                        float_operand(frame_value(frame, *value)),
                        XmmRegister::Xmm0,
                        output,
                    );
                } else {
                    load_rax(frame_value(frame, *value), output);
                    canonicalize_rax(return_type, output);
                }
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

fn load_float(source: FloatOperand, destination: XmmRegister, output: &mut Vec<Instruction>) {
    output.push(Instruction::MoveFloat64 {
        source,
        destination: destination.into(),
    });
}

fn store_float(source: XmmRegister, destination: FloatOperand, output: &mut Vec<Instruction>) {
    output.push(Instruction::MoveFloat64 {
        source: source.into(),
        destination,
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

fn float_memory(base: Register, displacement: i32) -> FloatOperand {
    FloatOperand::Memory { base, displacement }
}

fn float_operand(operand: Operand) -> FloatOperand {
    match operand {
        Operand::Memory { base, displacement } => float_memory(base, displacement),
        Operand::Register(_) => unreachable!("floating values use XMM registers"),
    }
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

fn epilogue_label(function: FunctionId) -> Label {
    Label::new(format!(".Lska_fn_{}_epilogue", function.index()))
}
