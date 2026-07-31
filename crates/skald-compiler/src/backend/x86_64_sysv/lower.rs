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
    literal_data::LiteralPool,
    machine::{AssemblyFunction, AssemblyProgram, Instruction, Label, Register},
    symbol,
};

mod array;
mod assignment;
mod call;
mod cleanup;
mod copy;
mod finalize;
mod integer_division;
mod object_abi;
mod optional;
mod ownership;
mod primitive_cast;
mod shift;
mod strings;
mod terminator;
mod type_operations;
mod value;

pub(super) fn lower(
    program: &MirProgram,
    data_layout: &DataLayout,
    dispatch: &DispatchMetadata,
) -> Result<AssemblyProgram, BackendError> {
    let literal_pool = LiteralPool::build(program);
    let context = LoweringContext {
        program,
        data_layout,
        dispatch,
        literal_pool: &literal_pool,
    };
    let mut functions = program
        .executable_definitions()
        .map(|definition| {
            let signature = program
                .callable_signature(definition.callable())
                .expect("verified definition must have a declaration");
            lower_definition(&context, signature, definition)
        })
        .collect::<Result<Vec<_>, _>>()?;
    functions.extend(array::lower_helpers(program, data_layout)?);
    functions.extend(ownership::lower_helpers(program, dispatch));
    functions.extend(finalize::lower_all(program, data_layout, dispatch)?);
    let entry = program
        .declarations
        .get(program.entry_function)
        .expect("verified entry declaration must exist");
    functions.push(entry_wrapper(program, entry.id.into()));
    let panic_messages = terminator::PanicMessagePool::build(&functions);
    Ok(AssemblyProgram {
        functions,
        dispatch_tables: dispatch.assembly_tables(program),
        literal_backings: literal_pool.into_backings(),
        panic_messages: panic_messages.into_assembly(),
    })
}

struct LoweringContext<'program> {
    program: &'program MirProgram,
    data_layout: &'program DataLayout,
    dispatch: &'program DispatchMetadata,
    literal_pool: &'program LiteralPool,
}

fn lower_definition(
    context: &LoweringContext<'_>,
    signature: MirCallableSignature<'_>,
    function: MirDefinitionRef<'_>,
) -> Result<AssemblyFunction, BackendError> {
    let frame = FrameLayout::plan(function, context.data_layout)?;
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
        instructions.push(Instruction::Jump(block_label(
            context.program,
            function.body().entry,
        )));
    }
    let epilogue = epilogue_label(context.program, function.callable());
    for block in &function.body().blocks {
        instructions.push(Instruction::Label(block_label(context.program, block.id)));
        let mut selector =
            InstructionSelector::new(context, function, block.id, &frame, &mut instructions);
        for instruction in &block.instructions {
            selector.select(instruction)?;
        }
        let block_terminator = block
            .terminator
            .as_ref()
            .expect("verified block is terminated");
        if !selector.select_termination(block_terminator)?
            && !selector.select_shift_terminator(block_terminator)
            && !selector.select_integer_division_terminator(block_terminator)
            && !selector.select_array_terminator(block_terminator)?
            && !selector.select_optional_terminator(block_terminator)?
            && !selector.select_type_operation_terminator(block_terminator, block.id)?
        {
            terminator::select(
                context.program,
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
        symbol: symbol::callable(context.program, function.callable()),
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
    literal_pool: &'program LiteralPool,
    function: MirDefinitionRef<'program>,
    frame: &'program FrameLayout,
    block: BlockId,
    optional_sequence: usize,
    array_sequence: usize,
    integer_division_sequence: usize,
    primitive_cast_sequence: usize,
    output: &'output mut Vec<Instruction>,
}

impl<'program, 'output> InstructionSelector<'program, 'output> {
    fn new(
        context: &LoweringContext<'program>,
        function: MirDefinitionRef<'program>,
        block: BlockId,
        frame: &'program FrameLayout,
        output: &'output mut Vec<Instruction>,
    ) -> Self {
        Self {
            program: context.program,
            data_layout: context.data_layout,
            dispatch: context.dispatch,
            literal_pool: context.literal_pool,
            function,
            frame,
            block,
            optional_sequence: 0,
            array_sequence: 0,
            integer_division_sequence: 0,
            primitive_cast_sequence: 0,
            output,
        }
    }

    /// Exhaustive MIR instruction dispatch. Operation-specific selection lives
    /// in sibling modules so adding an instruction identifies one clear owner.
    fn select(&mut self, instruction: &MirInstruction) -> Result<(), BackendError> {
        match instruction {
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
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
            MirInstruction::SharedAllocate(allocation) => {
                self.select_shared_allocate(allocation)?
            }
            MirInstruction::SharedInitialize(initialize) => {
                self.select_shared_initialize(initialize)?
            }
            MirInstruction::SharedPublish(publish) => self.select_shared_publish(publish)?,
            MirInstruction::SharedStatic(static_owner) => self.select_shared_static(static_owner),
            MirInstruction::SharedAdopt(adopt) => self.select_shared_adopt(adopt),
            MirInstruction::SharedCopy(copy) => self.select_shared_copy(copy),
            MirInstruction::SharedMove(transfer) => self.select_shared_move(transfer),
            MirInstruction::SharedRelease(release) => self.select_shared_release(release),
            MirInstruction::SharedFieldCopy(copy) => self.select_shared_field_copy(copy)?,
            MirInstruction::SharedCast(cast) => self.select_shared_cast(cast)?,
            MirInstruction::SharedFieldInitialize(initialize) => {
                self.select_shared_field_initialize(initialize)?
            }
            MirInstruction::SharedFieldReplace(replace) => {
                self.select_shared_field_replace(replace)?
            }
            MirInstruction::StringInitialize(initialize) => {
                self.select_string_initialize(initialize)?
            }
            MirInstruction::OptionalInitialize(initialize) => {
                self.select_optional_initialize(initialize)?
            }
            MirInstruction::OptionalAssign(assignment) => {
                self.select_optional_assign(assignment)?
            }
            MirInstruction::OptionalSharedInitialize(initialize) => {
                self.select_optional_shared_initialize(initialize)?
            }
            MirInstruction::OptionalSharedAssign(assignment) => {
                self.select_optional_shared_assign(assignment)?
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                self.select_optional_shared_cleanup(cleanup)?
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                self.select_class_optional_initialize(initialize)?
            }
            MirInstruction::ClassOptionalAssign(assignment) => {
                self.select_class_optional_assign(assignment)?
            }
            MirInstruction::ClassOptionalPublish(publish) => {
                self.select_class_optional_publish(publish)?
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                self.select_class_optional_cleanup(cleanup)?
            }
            MirInstruction::EndOptionalView(end) => self.select_optional_view_end(end)?,
            MirInstruction::Array(array) => self.select_array_instruction(array)?,
        }
        Ok(())
    }
}

fn block_label(program: &MirProgram, block: BlockId) -> Label {
    Label::new(format!(
        ".Lska.{}.block_{}",
        symbol::local_label_stem(program, block.callable()),
        block.index()
    ))
}

fn epilogue_label(program: &MirProgram, callable: CallableId) -> Label {
    Label::new(format!(
        ".Lska.{}.epilogue",
        symbol::local_label_stem(program, callable)
    ))
}
