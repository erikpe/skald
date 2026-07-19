//! Instruction selection and ABI lowering into the target assembly model.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirBinaryOperation, MirFunction, MirInstruction, MirProgram, MirRvalueKind, MirTerminator,
        MirUnaryOperation, StorageId, ValueId,
    },
    resolve::FunctionId,
};

use super::{
    abi::{self, IncomingArgument},
    frame::FrameLayout,
    machine::{AssemblyFunction, AssemblyProgram, Instruction, Operand, Register},
};

pub(super) fn lower(program: &MirProgram) -> Result<AssemblyProgram, BackendError> {
    let mut functions = program
        .functions
        .iter()
        .map(lower_function)
        .collect::<Result<Vec<_>, _>>()?;
    functions.push(entry_wrapper(program.entry_function));
    Ok(AssemblyProgram { functions })
}

fn lower_function(function: &MirFunction) -> Result<AssemblyFunction, BackendError> {
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
    let block = &function.body.blocks[0];
    for instruction in &block.instructions {
        select_instruction(instruction, &frame, &mut instructions)?;
    }
    select_terminator(
        block.terminator.as_ref().expect("target legality checked"),
        &frame,
        &mut instructions,
    );

    Ok(AssemblyFunction {
        symbol: symbol_for(function.id),
        exported: false,
        instructions,
    })
}

/// C-compatible process entry boundary. Returning the Skald `i64` in `%rax`
/// exposes its low 32 bits as C `main`'s `int`; Linux subsequently observes
/// the low eight bits as the process exit status.
fn entry_wrapper(entry_function: FunctionId) -> AssemblyFunction {
    AssemblyFunction {
        symbol: "main".to_owned(),
        exported: true,
        instructions: vec![
            Instruction::Push(Register::Rbp),
            Instruction::Move {
                source: Register::Rsp.into(),
                destination: Register::Rbp.into(),
            },
            Instruction::Call(symbol_for(entry_function)),
            Instruction::Leave,
            Instruction::Return,
        ],
    }
}

fn spill_parameters(
    function: &MirFunction,
    frame: &FrameLayout,
    instructions: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    for (index, storage) in function.parameters.iter().enumerate() {
        let incoming = abi::incoming_argument(index).ok_or_else(|| {
            BackendError::new(
                Target::X86_64SysV,
                Some(function.id),
                "incoming argument area exceeds the x86-64 ABI encoding limits",
            )
        })?;
        let destination = frame_storage(frame, *storage);
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
                        value: *value,
                        destination: Register::Rax,
                    });
                    store_rax(destination, output);
                }
                MirRvalueKind::Load(storage) => {
                    load_rax(frame_storage(frame, *storage), output);
                    store_rax(destination, output);
                }
                MirRvalueKind::Unary {
                    operation: MirUnaryOperation::NegateI64,
                    operand,
                } => {
                    load_rax(frame_value(frame, *operand), output);
                    output.push(Instruction::Negate(Register::Rax));
                    store_rax(destination, output);
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
                        MirBinaryOperation::AddI64 => Instruction::Add {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                        MirBinaryOperation::SubtractI64 => Instruction::Subtract {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                        MirBinaryOperation::MultiplyI64 => Instruction::Multiply {
                            source: Register::Rcx,
                            destination: Register::Rax,
                        },
                    });
                    store_rax(destination, output);
                }
                MirRvalueKind::DirectCall {
                    function,
                    arguments,
                } => select_call(*function, arguments, destination, frame, output)?,
            }
        }
        MirInstruction::Store(store) => {
            load_rax(frame_value(frame, store.value), output);
            store_rax(frame_storage(frame, store.storage), output);
        }
    }
    Ok(())
}

fn select_call(
    function: FunctionId,
    arguments: &[ValueId],
    destination: Operand,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) -> Result<(), BackendError> {
    let stack_size = abi::outgoing_stack_size(arguments.len()).ok_or_else(|| {
        BackendError::new(
            Target::X86_64SysV,
            None,
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

    output.push(Instruction::Call(symbol_for(function)));
    if stack_size != 0 {
        output.push(Instruction::ReleaseStack(stack_size));
    }
    store_rax(destination, output);
    Ok(())
}

fn select_terminator(
    terminator: &MirTerminator,
    frame: &FrameLayout,
    output: &mut Vec<Instruction>,
) {
    match terminator {
        MirTerminator::Return { value, .. } => {
            load_rax(frame_value(frame, *value), output);
            output.push(Instruction::Leave);
            output.push(Instruction::Return);
        }
    }
}

fn load_rax(source: Operand, output: &mut Vec<Instruction>) {
    output.push(Instruction::Move {
        source,
        destination: Register::Rax.into(),
    });
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

fn symbol_for(function: FunctionId) -> String {
    format!("ska_fn_{}", function.index())
}
