use super::*;
use crate::{
    identity::{CallableId, FunctionId},
    intrinsic::Intrinsic,
};

fn io_program() -> MirProgram {
    fixture_io_program()
}

fn io_instructions(program: &MirProgram) -> Vec<&MirIoInstruction> {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Io(io) => Some(io),
            _ => None,
        })
        .collect()
}

fn io_function(
    program: &MirProgram,
    matches_operation: impl Fn(&MirIoOperation) -> bool,
) -> FunctionId {
    program
        .definitions
        .iter()
        .find(|definition| {
            definition.body.blocks.iter().any(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| match instruction {
                        MirInstruction::Io(io) => matches_operation(&io.operation),
                        _ => false,
                    })
            })
        })
        .map(|definition| definition.function)
        .expect("fixture contains requested I/O operation")
}

#[test]
fn io_intrinsics_lower_to_verified_semantic_mir() {
    let program = io_program();
    verify_mir(&program).unwrap();
    let operations = io_instructions(&program);
    assert_eq!(operations.len(), 5);
    assert!(matches!(
        operations[0].operation,
        MirIoOperation::StandardHandle { .. }
    ));
    assert!(matches!(
        operations[1].operation,
        MirIoOperation::Open { .. }
    ));
    assert!(matches!(
        operations[2].operation,
        MirIoOperation::Read { .. }
    ));
    assert!(matches!(
        operations[3].operation,
        MirIoOperation::Write { .. }
    ));
    assert!(matches!(
        operations[4].operation,
        MirIoOperation::Close { .. }
    ));
    assert!(operations.iter().all(|io| {
        match io.result.callable() {
            CallableId::Function(function) => program.definitions.get(function),
            _ => None,
        }
        .and_then(|definition| definition.value(io.result))
        .is_some_and(|value| value.ty == MirType::I64)
    }));

    for io in &operations[1..4] {
        let buffer = match &io.operation {
            MirIoOperation::Open { path, .. } => path,
            MirIoOperation::Read { destination, .. } => destination,
            MirIoOperation::Write { source, .. } => source,
            _ => unreachable!(),
        };
        assert_eq!(
            program.array_type(buffer.array).unwrap().element,
            MirType::U8
        );
    }
    assert_eq!(
        match &operations[2].operation {
            MirIoOperation::Read { destination, .. } => destination.access,
            _ => unreachable!(),
        },
        MirAliasAccess::Mutable
    );
    assert_eq!(
        match &operations[3].operation {
            MirIoOperation::Write { source, .. } => source.access,
            _ => unreachable!(),
        },
        MirAliasAccess::ReadOnly
    );
}

#[test]
fn io_dump_exposes_exact_semantic_inputs_and_checked_offsets() {
    let dump = dump_mir(&io_program());
    let relevant = dump
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.contains(" = io ")
                || line.starts_with("array-range-offset ")
                || line.starts_with("array-position-check ") && line.contains("RangeOffset")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        relevant,
        vec![
            "f7:v1 = io standard-handle stream f7:v0 @407..434",
            "f8:v1 = io open path indirect(f8:s0) : a0 readonly anchor f8:s2 mode f8:v0 @495..515",
            "array-range-offset f9:s5 = f9:v1 in indirect(f9:s1) : a0 @635..641",
            "array-position-check f9:s5 RangeOffset -> f9:b1 else f9:b2 @635..641",
            "f9:v3 = io read handle f9:v2 destination indirect(f9:s1) : a0 mutable anchor f9:s4 offset f9:s5 @605..642",
            "array-range-offset f10:s5 = f10:v1 in indirect(f10:s1) : a0 @750..756",
            "array-position-check f10:s5 RangeOffset -> f10:b1 else f10:b2 @750..756",
            "f10:v3 = io write handle f10:v2 source indirect(f10:s1) : a0 readonly anchor f10:s4 offset f10:s5 @724..757",
            "f11:v1 = io close handle f11:v0 @806..823",
        ]
    );
}

#[test]
fn io_lowering_anchors_buffers_and_ends_them_after_the_operation() {
    let program = io_program();
    for definition in program.executable_definitions() {
        for block in &definition.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Io(io) = instruction else {
                    continue;
                };
                let Some(buffer) = (match &io.operation {
                    MirIoOperation::Open { path, .. } => Some(path),
                    MirIoOperation::Read { destination, .. } => Some(destination),
                    MirIoOperation::Write { source, .. } => Some(source),
                    _ => None,
                }) else {
                    continue;
                };
                assert!(definition
                    .body()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::AnchorBegin { anchor, .. })
                            if *anchor == buffer.anchor
                    )));
                assert!(definition
                    .body()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::AnchorEnd { anchor, .. })
                            if *anchor == buffer.anchor
                    )));
            }
        }
    }
}

#[test]
fn io_accepts_byte_array_elements_with_their_outer_backing_anchor() {
    let program = fixture_io_program_with_additional_bodies(concat!(
        "public fn write_nested(handle: i64, ref sources: u8[][], offset: u64) -> i64 {\n",
        "  return _io_write(handle, sources[0], offset);\n",
        "}\n",
    ));
    verify_mir(&program).unwrap();
    let nested = io_instructions(&program)
        .into_iter()
        .find_map(|io| match &io.operation {
            MirIoOperation::Write { source, .. }
                if matches!(source.place.base, MirPlaceBase::ArrayAlias(_)) =>
            {
                Some(source)
            }
            _ => None,
        })
        .expect("nested byte-array argument lowers through an alias carrier");
    let function = match nested.anchor.callable() {
        CallableId::Function(function) => program.definitions.get(function).unwrap(),
        _ => unreachable!(),
    };
    assert_ne!(
        function.storage(nested.anchor).unwrap().ty,
        MirType::Array(nested.array),
        "the outer descriptor, rather than the nested byte array, owns the backing anchor"
    );
}

#[test]
fn read_and_write_lower_arguments_once_in_left_to_right_order() {
    let program = io_program();
    for read in [true, false] {
        let function_id = io_function(&program, |operation| {
            if read {
                matches!(operation, MirIoOperation::Read { .. })
            } else {
                matches!(operation, MirIoOperation::Write { .. })
            }
        });
        let function = program.definitions.get(function_id).unwrap();
        let handle = function.parameters[0];
        let offset = function.parameters[2];
        for parameter in [handle, offset] {
            assert_eq!(
                function
                    .body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter(|instruction| matches!(
                        instruction,
                        MirInstruction::Assign(MirAssignment {
                            rvalue: MirRvalue { kind: MirRvalueKind::Load(place), .. },
                            ..
                        }) if place.base.storage() == parameter
                    ))
                    .count(),
                1
            );
        }

        let ordered = function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction {
                MirInstruction::Store(store)
                    if function
                        .storage(store.destination.base.storage())
                        .is_some_and(|storage| storage.kind == MirStorageKind::ScalarSpill) =>
                {
                    Some("handle-spill")
                }
                MirInstruction::Array(MirArrayInstruction::AnchorBegin { .. }) => Some("anchor"),
                MirInstruction::Array(MirArrayInstruction::Offset { .. }) => Some("offset"),
                MirInstruction::Io(_) => Some("io"),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ordered, ["handle-spill", "anchor", "offset", "io"]);

        let io_block = function
            .body
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, MirInstruction::Io(_)))
            })
            .unwrap()
            .id;
        assert!(function.body.blocks.iter().any(|block| matches!(
            block.terminator,
            Some(MirTerminator::ArrayPositionCheck {
                kind: MirArrayPositionKind::RangeOffset,
                success_target,
                ..
            }) if success_target == io_block
        )));
        assert!(!function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction, MirInstruction::Call(_))));
    }
}

#[test]
fn malformed_io_types_access_and_results_are_rejected() {
    let mut wrong_access = io_program();
    let function_id = io_function(&wrong_access, |operation| {
        matches!(operation, MirIoOperation::Write { .. })
    });
    let write = wrong_access
        .definitions
        .get_mut_for_test(function_id)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Io(MirIoInstruction {
                operation: MirIoOperation::Write { source, .. },
                ..
            }) => Some(source),
            _ => None,
        })
        .unwrap();
    write.access = MirAliasAccess::Mutable;
    assert!(verify_mir(&wrong_access)
        .unwrap_err()
        .to_string()
        .contains("exact `u8[]` place, compatible access"));

    let mut absent_result = io_program();
    let function_id = io_function(&absent_result, |_| true);
    let function = absent_result
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let undeclared_result = ValueId::new(function.callable(), function.values.len() + 10);
    let io = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Io(io) => Some(io),
            _ => None,
        })
        .unwrap();
    io.result = undeclared_result;
    assert!(verify_mir(&absent_result)
        .unwrap_err()
        .to_string()
        .contains("standard-I/O result"));

    let mut duplicate_result = io_program();
    let function_id = io_function(&duplicate_result, |_| true);
    let function = duplicate_result
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let block = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::Io(_)))
        })
        .unwrap();
    let index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Io(_)))
        .unwrap();
    let duplicate = block.instructions[index].clone();
    block.instructions.insert(index + 1, duplicate);
    assert!(verify_mir(&duplicate_result)
        .unwrap_err()
        .to_string()
        .contains("defined more than once"));
}

#[test]
fn malformed_io_anchor_and_bounds_dataflow_are_rejected() {
    let mut missing_check = io_program();
    let function_id = io_function(&missing_check, |operation| {
        matches!(operation, MirIoOperation::Read { .. })
    });
    let function = missing_check
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let check = function
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::ArrayPositionCheck {
                kind: kind @ MirArrayPositionKind::RangeOffset,
                ..
            }) => Some(kind),
            _ => None,
        })
        .unwrap();
    *check = MirArrayPositionKind::SliceBound;
    assert!(verify_mir(&missing_check)
        .unwrap_err()
        .to_string()
        .contains("dominated by its successful offset bounds check"));

    let mut missing_anchor = io_program();
    let function_id = io_function(&missing_anchor, |operation| {
        matches!(operation, MirIoOperation::Open { .. })
    });
    let function = missing_anchor
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let block = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Io(MirIoInstruction {
                        operation: MirIoOperation::Open { .. },
                        ..
                    })
                )
            })
        })
        .unwrap();
    block.instructions.retain(|instruction| {
        !matches!(
            instruction,
            MirInstruction::Array(MirArrayInstruction::AnchorBegin { .. })
        )
    });
    assert!(verify_mir(&missing_anchor)
        .unwrap_err()
        .to_string()
        .contains("exact compatible backing anchor"));
}

#[test]
fn malformed_io_scalar_and_byte_array_types_are_rejected() {
    let mut wrong_scalar = io_program();
    let function_id = io_function(&wrong_scalar, |operation| {
        matches!(operation, MirIoOperation::StandardHandle { .. })
    });
    let function = wrong_scalar
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let stream = function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Io(MirIoInstruction {
                operation: MirIoOperation::StandardHandle { stream },
                ..
            }) => Some(*stream),
            _ => None,
        })
        .unwrap();
    function.values[stream.index()].ty = MirType::I64;
    assert!(verify_mir(&wrong_scalar)
        .unwrap_err()
        .to_string()
        .contains("stream selector must be a block-local `u8` value"));

    let mut non_byte = io_program();
    let byte_array = io_instructions(&non_byte)
        .iter()
        .find_map(|instruction| match &instruction.operation {
            MirIoOperation::Open { path, .. } => Some(path.array),
            _ => None,
        })
        .unwrap();
    non_byte.array_types.entries_mut_for_test()[byte_array.index()].element = MirType::I64;
    assert!(verify_mir(&non_byte)
        .unwrap_err()
        .to_string()
        .contains("exact `u8[]` place"));
}

#[test]
fn io_rejects_early_lifetime_end_and_residual_intrinsic_calls() {
    let mut early_end = io_program();
    let function_id = io_function(&early_end, |operation| {
        matches!(operation, MirIoOperation::Open { .. })
    });
    let function = early_end.definitions.get_mut_for_test(function_id).unwrap();
    let (block_index, io_index, anchor, span) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find_map(|(io_index, instruction)| match instruction {
                    MirInstruction::Io(MirIoInstruction {
                        operation: MirIoOperation::Open { path, .. },
                        span,
                        ..
                    }) => Some((block_index, io_index, path.anchor, *span)),
                    _ => None,
                })
        })
        .unwrap();
    function.body.blocks[block_index].instructions.insert(
        io_index,
        MirInstruction::StorageDead(MirStorageDead {
            storage: anchor,
            span,
        }),
    );
    let errors = verify_mir(&early_end).unwrap_err().to_string();
    assert!(errors.contains("array owner state remains active at storage-dead"));
    assert!(errors.contains("backing anchor to be live"));

    let mut residual_call = io_program();
    let intrinsic = residual_call
        .declarations
        .iter()
        .find(|declaration| {
            declaration.linkage
                == MirFunctionLinkage::Intrinsic {
                    intrinsic: Intrinsic::IoStandardHandle,
                }
        })
        .map(|declaration| declaration.id)
        .unwrap();
    let function_id = io_function(&residual_call, |operation| {
        matches!(operation, MirIoOperation::StandardHandle { .. })
    });
    let function = residual_call
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let instruction = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| matches!(instruction, MirInstruction::Io(_)))
        .unwrap();
    let MirInstruction::Io(io) = instruction.clone() else {
        unreachable!()
    };
    let MirIoOperation::StandardHandle { stream } = io.operation else {
        unreachable!()
    };
    *instruction = MirInstruction::Call(MirCall {
        target: MirCallTarget::Direct(intrinsic),
        receiver: None,
        arguments: vec![MirArgument::Value(stream)],
        result: Some(io.result),
        shared_result: None,
        destination: None,
        span: io.span,
    });
    assert!(verify_mir(&residual_call)
        .unwrap_err()
        .to_string()
        .contains("intrinsic function must not remain as an ordinary MIR call"));
}
