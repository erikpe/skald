//! Instruction selection and ABI lowering into the target assembly model.

use crate::{
    backend::{BackendError, RUNTIME_ABI_MARKER_SYMBOL},
    identity::CallableId,
    mir::{BlockId, MirCallableSignature, MirDefinitionRef, MirInstruction, MirProgram},
};

use super::{
    dispatch::DispatchMetadata,
    frame::FrameLayout,
    layout::DataLayout,
    machine::{AssemblyFunction, AssemblyProgram, Instruction, Label, Register},
    symbol,
};

mod assignment;
mod call;
mod cleanup;
mod copy;
mod object_abi;
mod terminator;
mod type_operations;
mod value;

pub(super) fn lower(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
) -> Result<AssemblyProgram, BackendError> {
    let mut functions = program
        .executable_definitions()
        .map(|definition| {
            let signature = program
                .callable_signature(definition.callable())
                .expect("verified definition must have a declaration");
            lower_definition(program, data_layout, dispatch, signature, definition)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let entry = program
        .declarations
        .get(program.entry_function)
        .expect("verified entry declaration must exist");
    functions.push(entry_wrapper(program, entry.id.into()));
    Ok(AssemblyProgram {
        functions,
        dispatch_tables: dispatch.assembly_tables(program),
    })
}

fn lower_definition(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
    signature: MirCallableSignature<'_>,
    function: MirDefinitionRef<'_>,
) -> Result<AssemblyFunction, BackendError> {
    let frame = FrameLayout::plan(function, data_layout)?;
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

    call::spill_parameters(signature, function, &frame, &mut instructions)?;
    if function.body().blocks[0].id != function.body().entry {
        instructions.push(Instruction::Jump(block_label(function.body().entry)));
    }
    let epilogue = epilogue_label(function.callable());
    for block in &function.body().blocks {
        instructions.push(Instruction::Label(block_label(block.id)));
        let mut selector = InstructionSelector::new(
            program,
            data_layout,
            dispatch,
            function,
            &frame,
            &mut instructions,
        );
        for instruction in &block.instructions {
            selector.select(instruction)?;
        }
        let block_terminator = block
            .terminator
            .as_ref()
            .expect("verified block is terminated");
        if !selector.select_type_operation_terminator(block_terminator, block.id)? {
            terminator::select(
                block_terminator,
                &frame,
                signature.return_type,
                &epilogue,
                &mut instructions,
            );
        }
    }
    instructions.push(Instruction::Label(epilogue));
    instructions.push(Instruction::Leave);
    instructions.push(Instruction::Return);

    Ok(AssemblyFunction {
        symbol: symbol::callable(program, function.callable()),
        exported: false,
        instructions,
    })
}

/// C-compatible process entry boundary. Returning the Skald `i64` in `rax`
/// exposes its low 32 bits as C `main`'s `int`; Linux subsequently observes
/// the low eight bits as the process exit status.
fn entry_wrapper(program: &MirProgram, entry: CallableId) -> AssemblyFunction {
    AssemblyFunction {
        symbol: "main".to_owned(),
        exported: true,
        instructions: vec![
            Instruction::Push(Register::Rbp),
            Instruction::Move {
                source: Register::Rsp.into(),
                destination: Register::Rbp.into(),
            },
            Instruction::Call(RUNTIME_ABI_MARKER_SYMBOL.to_owned()),
            Instruction::Call(symbol::callable(program, entry)),
            Instruction::Leave,
            Instruction::Return,
        ],
    }
}

struct InstructionSelector<'program, 'output> {
    program: &'program MirProgram,
    data_layout: &'program DataLayout,
    dispatch: &'program DispatchMetadata,
    function: MirDefinitionRef<'program>,
    frame: &'program FrameLayout,
    output: &'output mut Vec<Instruction>,
}

impl<'program, 'output> InstructionSelector<'program, 'output> {
    fn new(
        program: &'program MirProgram,
        data_layout: &'program DataLayout,
        dispatch: &'program DispatchMetadata,
        function: MirDefinitionRef<'program>,
        frame: &'program FrameLayout,
        output: &'output mut Vec<Instruction>,
    ) -> Self {
        Self {
            program,
            data_layout,
            dispatch,
            function,
            frame,
            output,
        }
    }

    /// Exhaustive MIR instruction dispatch. Operation-specific selection lives
    /// in sibling modules so adding an instruction identifies one clear owner.
    fn select(&mut self, instruction: &MirInstruction) -> Result<(), BackendError> {
        match instruction {
            MirInstruction::Assign(assignment) => self.select_assignment(assignment)?,
            MirInstruction::Call(call) => self.select_call(call)?,
            MirInstruction::Cleanup(cleanup) => self.select_cleanup(cleanup)?,
            MirInstruction::Initialize(initialize) => self.select_initialize(initialize)?,
            MirInstruction::Store(store) => self.select_store(store)?,
            MirInstruction::CopyConstruct(copy) => self.select_copy_construction(copy)?,
            MirInstruction::CopyAssign(copy) => self.select_copy_assignment(copy)?,
            MirInstruction::EndFullExpression(end) => self.select_end_full_expression(end)?,
            MirInstruction::BindCheckedView(binding) => {
                self.select_checked_view_binding(binding)?
            }
            MirInstruction::EndCheckedView(_) => {}
        }
        Ok(())
    }
}

fn block_label(block: BlockId) -> Label {
    Label::new(format!(
        ".Lska_{}_block_{}",
        symbol::local_label_stem(block.callable()),
        block.index()
    ))
}

fn epilogue_label(callable: CallableId) -> Label {
    Label::new(format!(
        ".Lska_{}_epilogue",
        symbol::local_label_stem(callable)
    ))
}
