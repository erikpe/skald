//! Instruction selection and ABI lowering into the target assembly model.

use crate::{
    backend::BackendError,
    identity::FunctionId,
    mir::{
        BlockId, MirFunctionDeclaration, MirFunctionDefinition, MirFunctionLinkage, MirInstruction,
        MirProgram,
    },
};

use super::{
    frame::FrameLayout,
    machine::{AssemblyFunction, AssemblyProgram, Instruction, Label, Register},
};

mod assignment;
mod call;
mod terminator;
mod value;

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

    call::spill_parameters(declaration, function, &frame, &mut instructions)?;
    if function.body.blocks[0].id != function.body.entry {
        instructions.push(Instruction::Jump(block_label(function.body.entry)));
    }
    let epilogue = epilogue_label(function.function);
    for block in &function.body.blocks {
        instructions.push(Instruction::Label(block_label(block.id)));
        for instruction in &block.instructions {
            InstructionSelector::new(program, function, &frame, &mut instructions)
                .select(instruction)?;
        }
        terminator::select(
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

struct InstructionSelector<'program, 'output> {
    program: &'program MirProgram,
    function: &'program MirFunctionDefinition,
    frame: &'program FrameLayout,
    output: &'output mut Vec<Instruction>,
}

impl<'program, 'output> InstructionSelector<'program, 'output> {
    fn new(
        program: &'program MirProgram,
        function: &'program MirFunctionDefinition,
        frame: &'program FrameLayout,
        output: &'output mut Vec<Instruction>,
    ) -> Self {
        Self {
            program,
            function,
            frame,
            output,
        }
    }

    /// Exhaustive MIR instruction dispatch. Operation-specific selection lives
    /// in sibling modules so adding an instruction identifies one clear owner.
    fn select(&mut self, instruction: &MirInstruction) -> Result<(), BackendError> {
        match instruction {
            MirInstruction::Assign(assignment) => self.select_assignment(assignment),
            MirInstruction::Call(call) => self.select_call(call)?,
            MirInstruction::Store(store) => self.select_store(store),
        }
        Ok(())
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
