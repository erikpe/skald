use std::{
    io::Write,
    process::{Command, Stdio},
};

use crate::{
    backend::{emit_assembly, Target},
    identity::{BindingId, FunctionId, LocalId, ParameterId},
    mir::{
        verify_mir, BlockId, MirAssignment, MirBasicBlock, MirBinaryOperation, MirBody, MirCall,
        MirCallTarget, MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionDefinition,
        MirFunctionDefinitionTable, MirFunctionLinkage, MirInstruction, MirProgram, MirRvalue,
        MirRvalueKind, MirStorage, MirStorageKind, MirStore, MirTerminator, MirType,
        MirUnaryOperation, MirValue, StorageId, ValueId,
    },
    source::SourceDatabase,
    test_support::{lower_source_to_assembly, lower_source_to_mir, TemporaryFile},
};

fn lower_text(text: &str) -> MirProgram {
    lower_source_to_mir(text)
}

fn assembly(text: &str) -> String {
    lower_source_to_assembly(text, Target::X86_64SysV).unwrap()
}

fn test_span() -> crate::source::Span {
    let mut sources = SourceDatabase::new();
    let source = sources.add("backend-mir-test.ska", "");
    crate::source::Span::empty(source, 0)
}

fn f64_arithmetic_program() -> MirProgram {
    let span = test_span();
    let compute_id = FunctionId::new(0);
    let validate_id = FunctionId::new(1);
    let main_id = FunctionId::new(2);
    let value = |function, index, ty| MirValue {
        id: ValueId::new(function, index),
        ty,
        span,
    };
    let assignment = |function, index, kind, ty| {
        MirInstruction::Assign(MirAssignment {
            result: ValueId::new(function, index),
            rvalue: MirRvalue { kind, ty },
            span,
        })
    };

    let compute = MirFunctionDefinition {
        function: compute_id,
        parameters: vec![],
        storage: vec![MirStorage {
            id: StorageId::new(compute_id, 0),
            source: BindingId::Local(LocalId::new(compute_id, 0)),
            name: "result".to_owned(),
            kind: MirStorageKind::Local,
            ty: MirType::F64,
            span,
        }],
        values: (0..8)
            .map(|index| value(compute_id, index, MirType::F64))
            .collect(),
        body: MirBody {
            entry: BlockId::new(compute_id, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(compute_id, 0),
                instructions: vec![
                    assignment(
                        compute_id,
                        0,
                        MirRvalueKind::ConstantF64Bits(1.5_f64.to_bits()),
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        1,
                        MirRvalueKind::ConstantF64Bits(2.0_f64.to_bits()),
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        2,
                        MirRvalueKind::Binary {
                            operation: MirBinaryOperation::MultiplyF64,
                            left: ValueId::new(compute_id, 0),
                            right: ValueId::new(compute_id, 1),
                        },
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        3,
                        MirRvalueKind::Unary {
                            operation: MirUnaryOperation::NegateF64,
                            operand: ValueId::new(compute_id, 2),
                        },
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        4,
                        MirRvalueKind::ConstantF64Bits(0.5_f64.to_bits()),
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        5,
                        MirRvalueKind::Binary {
                            operation: MirBinaryOperation::AddF64,
                            left: ValueId::new(compute_id, 3),
                            right: ValueId::new(compute_id, 4),
                        },
                        MirType::F64,
                    ),
                    assignment(
                        compute_id,
                        6,
                        MirRvalueKind::Binary {
                            operation: MirBinaryOperation::SubtractF64,
                            left: ValueId::new(compute_id, 5),
                            right: ValueId::new(compute_id, 4),
                        },
                        MirType::F64,
                    ),
                    MirInstruction::Store(MirStore {
                        storage: StorageId::new(compute_id, 0),
                        value: ValueId::new(compute_id, 6),
                        span,
                    }),
                    assignment(
                        compute_id,
                        7,
                        MirRvalueKind::Load(StorageId::new(compute_id, 0)),
                        MirType::F64,
                    ),
                ],
                terminator: Some(MirTerminator::Return {
                    value: Some(ValueId::new(compute_id, 7)),
                    span,
                }),
                span,
            }],
        },
        span,
    };
    let main = MirFunctionDefinition {
        function: main_id,
        parameters: vec![],
        storage: vec![],
        values: vec![
            value(main_id, 0, MirType::F64),
            value(main_id, 1, MirType::I64),
        ],
        body: MirBody {
            entry: BlockId::new(main_id, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(main_id, 0),
                instructions: vec![
                    MirInstruction::Call(MirCall {
                        target: MirCallTarget::Direct(compute_id),
                        arguments: vec![],
                        result: Some(ValueId::new(main_id, 0)),
                        span,
                    }),
                    MirInstruction::Call(MirCall {
                        target: MirCallTarget::Direct(validate_id),
                        arguments: vec![ValueId::new(main_id, 0)],
                        result: Some(ValueId::new(main_id, 1)),
                        span,
                    }),
                ],
                terminator: Some(MirTerminator::Return {
                    value: Some(ValueId::new(main_id, 1)),
                    span,
                }),
                span,
            }],
        },
        span,
    };

    MirProgram {
        declarations: MirFunctionDeclarationTable::new(vec![
            MirFunctionDeclaration {
                id: compute_id,
                name: "compute".to_owned(),
                parameter_types: vec![],
                return_type: MirType::F64,
                linkage: MirFunctionLinkage::Internal,
                span,
            },
            MirFunctionDeclaration {
                id: validate_id,
                name: "validate_f64".to_owned(),
                parameter_types: vec![MirType::F64],
                return_type: MirType::I64,
                linkage: MirFunctionLinkage::External {
                    symbol: "validate_f64".to_owned(),
                },
                span,
            },
            MirFunctionDeclaration {
                id: main_id,
                name: "main".to_owned(),
                parameter_types: vec![],
                return_type: MirType::I64,
                linkage: MirFunctionLinkage::Internal,
                span,
            },
        ]),
        definitions: MirFunctionDefinitionTable::new(vec![Some(compute), None, Some(main)]),
        entry_function: main_id,
        span,
    }
}

fn mixed_exhausted_abi_program() -> MirProgram {
    let span = test_span();
    let mixed_id = FunctionId::new(0);
    let main_id = FunctionId::new(1);
    let mut parameter_types = Vec::new();
    for _ in 0..6 {
        parameter_types.extend([MirType::I64, MirType::F64]);
    }
    parameter_types.extend([MirType::F64, MirType::F64, MirType::I64, MirType::F64]);

    let storage: Vec<_> = parameter_types
        .iter()
        .enumerate()
        .map(|(index, ty)| MirStorage {
            id: StorageId::new(mixed_id, index),
            source: BindingId::Parameter(ParameterId::new(mixed_id, index)),
            name: format!("p{index}"),
            kind: MirStorageKind::Parameter,
            ty: *ty,
            span,
        })
        .collect();
    let mixed = MirFunctionDefinition {
        function: mixed_id,
        parameters: storage.iter().map(|storage| storage.id).collect(),
        storage,
        values: vec![MirValue {
            id: ValueId::new(mixed_id, 0),
            ty: MirType::F64,
            span,
        }],
        body: MirBody {
            entry: BlockId::new(mixed_id, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(mixed_id, 0),
                instructions: vec![MirInstruction::Assign(MirAssignment {
                    result: ValueId::new(mixed_id, 0),
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Load(StorageId::new(mixed_id, 15)),
                        ty: MirType::F64,
                    },
                    span,
                })],
                terminator: Some(MirTerminator::Return {
                    value: Some(ValueId::new(mixed_id, 0)),
                    span,
                }),
                span,
            }],
        },
        span,
    };

    let mut values = Vec::new();
    let mut instructions = Vec::new();
    for (index, ty) in parameter_types.iter().copied().enumerate() {
        values.push(MirValue {
            id: ValueId::new(main_id, index),
            ty,
            span,
        });
        let kind = if ty == MirType::F64 {
            MirRvalueKind::ConstantF64Bits((index as f64).to_bits())
        } else {
            MirRvalueKind::ConstantI64(index as i64)
        };
        instructions.push(MirInstruction::Assign(MirAssignment {
            result: ValueId::new(main_id, index),
            rvalue: MirRvalue { kind, ty },
            span,
        }));
    }
    let call_result = ValueId::new(main_id, values.len());
    values.push(MirValue {
        id: call_result,
        ty: MirType::F64,
        span,
    });
    instructions.push(MirInstruction::Call(MirCall {
        target: MirCallTarget::Direct(mixed_id),
        arguments: (0..parameter_types.len())
            .map(|index| ValueId::new(main_id, index))
            .collect(),
        result: Some(call_result),
        span,
    }));
    let return_value = ValueId::new(main_id, values.len());
    values.push(MirValue {
        id: return_value,
        ty: MirType::I64,
        span,
    });
    instructions.push(MirInstruction::Assign(MirAssignment {
        result: return_value,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantI64(0),
            ty: MirType::I64,
        },
        span,
    }));
    let main = MirFunctionDefinition {
        function: main_id,
        parameters: vec![],
        storage: vec![],
        values,
        body: MirBody {
            entry: BlockId::new(main_id, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(main_id, 0),
                instructions,
                terminator: Some(MirTerminator::Return {
                    value: Some(return_value),
                    span,
                }),
                span,
            }],
        },
        span,
    };

    MirProgram {
        declarations: MirFunctionDeclarationTable::new(vec![
            MirFunctionDeclaration {
                id: mixed_id,
                name: "mixed".to_owned(),
                parameter_types,
                return_type: MirType::F64,
                linkage: MirFunctionLinkage::Internal,
                span,
            },
            MirFunctionDeclaration {
                id: main_id,
                name: "main".to_owned(),
                parameter_types: vec![],
                return_type: MirType::I64,
                linkage: MirFunctionLinkage::Internal,
                span,
            },
        ]),
        definitions: MirFunctionDefinitionTable::new(vec![Some(mixed), Some(main)]),
        entry_function: main_id,
        span,
    }
}

fn conditional_return_mir(condition_value: bool) -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let condition = ValueId::new(function.function, 0);
    let true_value = ValueId::new(function.function, 1);
    let false_value = ValueId::new(function.function, 2);
    function.values = vec![
        MirValue {
            id: condition,
            ty: MirType::Bool,
            span,
        },
        MirValue {
            id: true_value,
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: false_value,
            ty: MirType::I64,
            span,
        },
    ];
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    function.body.entry = entry;
    function.body.blocks = vec![
        MirBasicBlock {
            id: entry,
            instructions: vec![constant_bool(condition, condition_value, span)],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        },
        MirBasicBlock {
            id: true_block,
            instructions: vec![constant_i64(true_value, 37, span)],
            terminator: Some(MirTerminator::Return {
                value: Some(true_value),
                span,
            }),
            span,
        },
        MirBasicBlock {
            id: false_block,
            instructions: vec![constant_i64(false_value, 12, span)],
            terminator: Some(MirTerminator::Return {
                value: Some(false_value),
                span,
            }),
            span,
        },
    ];
    assert!(verify_mir(&mir).is_ok());
    mir
}

fn branch_call_diamond_mir() -> MirProgram {
    let mut mir = lower_text(concat!(
        "fn left() -> i64 { return 11; }\n",
        "fn right() -> i64 { return 22; }\n",
        "fn main() -> i64 { var result: i64 = 0; return result; }\n",
    ));
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let storage = function.storage[0].id;
    let condition = ValueId::new(function.function, 0);
    let left_result = ValueId::new(function.function, 1);
    let right_result = ValueId::new(function.function, 2);
    let joined_result = ValueId::new(function.function, 3);
    function.values = [
        (condition, MirType::Bool),
        (left_result, MirType::I64),
        (right_result, MirType::I64),
        (joined_result, MirType::I64),
    ]
    .into_iter()
    .map(|(id, ty)| MirValue { id, ty, span })
    .collect();
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    let join = BlockId::new(function.function, 3);
    function.body.entry = entry;
    function.body.blocks = vec![
        MirBasicBlock {
            id: entry,
            instructions: vec![constant_bool(condition, true, span)],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        },
        call_and_store_block(
            true_block,
            FunctionId::new(0),
            left_result,
            storage,
            join,
            span,
        ),
        call_and_store_block(
            false_block,
            FunctionId::new(1),
            right_result,
            storage,
            join,
            span,
        ),
        MirBasicBlock {
            id: join,
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: joined_result,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::Load(storage),
                    ty: MirType::I64,
                },
                span,
            })],
            terminator: Some(MirTerminator::Return {
                value: Some(joined_result),
                span,
            }),
            span,
        },
    ];
    assert!(verify_mir(&mir).is_ok());
    mir
}

fn call_and_store_block(
    id: BlockId,
    target: FunctionId,
    result: ValueId,
    storage: crate::mir::StorageId,
    join: BlockId,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id,
        instructions: vec![
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(target),
                arguments: Vec::new(),
                result: Some(result),
                span,
            }),
            MirInstruction::Store(MirStore {
                storage,
                value: result,
                span,
            }),
        ],
        terminator: Some(MirTerminator::Goto { target: join, span }),
        span,
    }
}

fn constant_bool(result: ValueId, value: bool, span: crate::source::Span) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantBool(value),
            ty: MirType::Bool,
        },
        span,
    })
}

fn constant_i64(result: ValueId, value: i64, span: crate::source::Span) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantI64(value),
            ty: MirType::I64,
        },
        span,
    })
}

fn assert_system_assembler_accepts(output: &str) {
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-c", "-o", "/dev/null", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the M0 Linux toolchain prerequisite requires `cc`");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();

    assert!(
        result.status.success(),
        "assembler rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn run_native_assembly(output: &str) -> std::process::ExitStatus {
    let executable = TemporaryFile::new("native-executable").unwrap();
    let mut child = Command::new("cc")
        .args(["-x", "assembler", "-o"])
        .arg(executable.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the M0 Linux toolchain prerequisite requires `cc`");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();
    let linked = child.wait_with_output().unwrap();
    assert!(
        linked.status.success(),
        "linker rejected generated output:\n{}\nassembly:\n{output}",
        String::from_utf8_lossy(&linked.stderr)
    );

    Command::new(executable.path()).status().unwrap()
}

#[test]
fn emits_a_deterministic_minimal_function() {
    let source = "fn main() -> i64 { return 42; }";
    let expected = concat!(
        ".text\n",
        ".p2align 4\n",
        ".type .Lska_fn_0, @function\n",
        ".Lska_fn_0:\n",
        "    pushq %rbp\n",
        "    movq %rsp, %rbp\n",
        "    subq $16, %rsp\n",
        ".Lska_fn_0_block_0:\n",
        "    movabsq $42, %rax\n",
        "    movq %rax, -8(%rbp)\n",
        "    movq -8(%rbp), %rax\n",
        "    jmp .Lska_fn_0_epilogue\n",
        ".Lska_fn_0_epilogue:\n",
        "    leave\n",
        "    ret\n",
        ".size .Lska_fn_0, .-.Lska_fn_0\n",
        "\n",
        ".p2align 4\n",
        ".globl main\n",
        ".type main, @function\n",
        "main:\n",
        "    pushq %rbp\n",
        "    movq %rsp, %rbp\n",
        "    call .Lska_fn_0\n",
        "    leave\n",
        "    ret\n",
        ".size main, .-main\n",
        "\n",
        ".section .note.GNU-stack,\"\",@progbits\n",
    );

    assert_eq!(assembly(source), expected);
    assert_eq!(assembly(source), assembly(source));
}

#[test]
fn lowers_source_conditionals_to_deterministic_block_branches() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  if (false) { return 1; }\n",
        "  elif (true) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
    );

    let output = assembly(source);
    assert_eq!(output, assembly(source));
    assert!(output.contains(".Lska_fn_0_block_0:"));
    assert!(output.contains("jne .Lska_fn_0_block_1"));
    assert!(output.contains("jmp .Lska_fn_0_block_2"));
    assert!(output.contains(".Lska_fn_0_block_4:"));
}

#[test]
fn selects_every_first_slice_arithmetic_operation_and_storage_copy() {
    let output = assembly(concat!(
        "fn helper(a: i64) -> i64 { return -a; }\n",
        "fn main() -> i64 { ",
        "var x: i64 = 9; return helper(x * 3 - 4 + 2); }",
    ));

    assert!(output.contains("negq %rax"));
    assert!(output.contains("imulq %rcx, %rax"));
    assert!(output.contains("subq %rcx, %rax"));
    assert!(output.contains("addq %rcx, %rax"));
    assert!(output.contains("call .Lska_fn_0"));
    assert!(output.contains("movq %rax, -8(%rbp)"));
}

#[test]
fn lowers_u64_payloads_arithmetic_and_integer_class_calls() {
    let output = assembly(concat!(
        "fn seventh(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64) -> u64 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u64 = seventh(18446744073709551615u, 2u, 3u, 4u, 5u, 6u, 7u); return 0; }",
    ));

    assert!(output.contains("movabsq $0xffffffffffffffff, %rax"));
    assert!(output.contains("addq %rcx, %rax"));
    assert!(output.contains("imulq %rcx, %rax"));
    assert!(output.contains("subq %rcx, %rax"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("call .Lska_fn_0"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u64_calls_use_rax_for_the_full_width_result() {
    let output = assembly(concat!(
        "extern fn foreign_u64(value: u64) -> u64;\n",
        "fn main() -> i64 { var value: u64 = foreign_u64(18446744073709551615u); return 0; }",
    ));

    assert!(output.contains("movabsq $0xffffffffffffffff, %rax"));
    assert!(output.contains("movq -16(%rbp), %rdi"));
    assert!(output.contains("call foreign_u64\n    movq %rax,"));
}

#[test]
fn canonicalizes_u8_arithmetic_parameters_calls_and_returns() {
    let output = assembly(concat!(
        "fn seventh(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8) -> u8 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u8 = seventh(255u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8); return 0; }",
    ));

    assert!(output.contains("movq %rdi, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("movq 16(%rbp), %rax\n    movzbq %al, %rax"));
    assert!(output.matches("movzbq %al, %rax").count() >= 12);
    assert!(output.contains("addq %rcx, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("imulq %rcx, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("subq %rcx, %rax\n    movzbq %al, %rax"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u8_results_are_zero_extended_before_storage() {
    let output = assembly(concat!(
        "extern fn foreign_u8(value: u8) -> u8;\n",
        "fn main() -> i64 { var value: u8 = foreign_u8(255u8); return 0; }",
    ));

    assert!(output.contains("call foreign_u8\n    movzbq %al, %rax\n    movq %rax,"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn lowers_verified_f64_mir_with_sse2_and_xmm_abi_results() {
    let program = f64_arithmetic_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("movabsq $4609434218613702656, %rax\n    movq %rax, %xmm14"));
    assert!(output.contains("mulsd %xmm15, %xmm14"));
    assert!(output.contains("xorpd %xmm15, %xmm14"));
    assert!(output.contains("addsd %xmm15, %xmm14"));
    assert!(output.contains("subsd %xmm15, %xmm14"));
    assert!(output.contains("movsd %xmm14, -8(%rbp)"));
    assert!(output.contains("movsd -72(%rbp), %xmm0"));
    assert!(output.contains("call .Lska_fn_0\n    movsd %xmm0,"));
    assert!(output.contains("movsd ") && output.contains(", %xmm0\n    call validate_f64"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn verified_f64_mir_executes_through_internal_and_external_abi_boundaries() {
    let mut output = emit_assembly(Target::X86_64SysV, &f64_arithmetic_program()).unwrap();
    output.push_str(concat!(
        "\n.text\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq %xmm0, %rax\n",
        "    movabsq $0xc008000000000000, %rcx\n",
        "    cmpq %rcx, %rax\n",
        "    setne %al\n",
        "    movzbq %al, %rax\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));

    assert!(run_native_assembly(&output).success());
}

#[test]
fn external_f64_results_are_read_from_xmm0() {
    let mut program = f64_arithmetic_program();
    program.declarations.entries_mut_for_test()[0].linkage = MirFunctionLinkage::External {
        symbol: "compute".to_owned(),
    };
    program.definitions.remove_for_test(FunctionId::new(0));
    verify_mir(&program).unwrap();

    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains("call compute\n    movsd %xmm0,"));
    output.push_str(concat!(
        "\n.text\n",
        ".globl compute\n",
        ".type compute, @function\n",
        "compute:\n",
        "    movabsq $0xc008000000000000, %rax\n",
        "    movq %rax, %xmm0\n",
        "    ret\n",
        ".size compute, .-compute\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq %xmm0, %rax\n",
        "    movabsq $0xc008000000000000, %rcx\n",
        "    cmpq %rcx, %rax\n",
        "    setne %al\n",
        "    movzbq %al, %rax\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));
    assert!(run_native_assembly(&output).success());
}

#[test]
fn source_f64_uses_independent_integer_and_sse_argument_registers() {
    let output = assembly(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn choose(integer: i64, floating: f64, other: i64, another: f64) -> f64 { return floating + another; }\n",
        "fn main() -> i64 { observe(choose(1, 1.5, 2, 2.25)); return 0; }",
    ));

    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movsd %xmm0, -16(%rbp)"));
    assert!(output.contains("movq %rsi, -24(%rbp)"));
    assert!(output.contains("movsd %xmm1, -32(%rbp)"));
    assert!(output.contains("addsd %xmm15, %xmm14"));
    assert!(output.contains("call .Lska_fn_1\n    movsd %xmm0,"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn mixed_scalar_layout_independently_exhausts_register_classes() {
    let program = mixed_exhausted_abi_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movsd %xmm0,"));
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("movsd 24(%rbp), %xmm14"));
    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("movsd %xmm14, 8(%rsp)"));
    assert!(output.contains("addq $16, %rsp"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn malformed_f64_mir_is_a_structured_backend_error() {
    let mut program = f64_arithmetic_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected f64 constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message.contains("input MIR failed verification"));
    assert!(error.message.contains("f64 constant is not `f64`"));
}

#[test]
fn unit_calls_and_returns_do_not_move_a_fictitious_result() {
    let output = assembly(concat!(
        "fn notify(value: i64) -> unit {}\n",
        "fn main() -> i64 { notify(42); return 7; }\n",
    ));

    assert!(output.contains("call .Lska_fn_0\n    movabsq $7, %rax"));
    assert!(!output.contains("call .Lska_fn_0\n    movq %rax,"));
    assert!(output.contains(
        ".Lska_fn_0:\n    pushq %rbp\n    movq %rsp, %rbp\n    subq $16, %rsp\n    movq %rdi, -8(%rbp)\n.Lska_fn_0_block_0:\n    jmp .Lska_fn_0_epilogue\n.Lska_fn_0_epilogue:\n    leave\n    ret"
    ));
}

#[test]
fn lowers_register_and_stack_arguments_at_the_abi_boundary() {
    let output = assembly(concat!(
        "fn seventh(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, ",
        "g: i64) -> i64 { return g; }\n",
        "fn main() -> i64 { return seventh(1, 2, 3, 4, 5, 6, 7); }",
    ));

    for spill in [
        "movq %rdi, -8(%rbp)",
        "movq %rsi, -16(%rbp)",
        "movq %rdx, -24(%rbp)",
        "movq %rcx, -32(%rbp)",
        "movq %r8, -40(%rbp)",
        "movq %r9, -48(%rbp)",
    ] {
        assert!(output.contains(spill), "missing `{spill}` in:\n{output}");
    }
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("call .Lska_fn_0\n    addq $16, %rsp"));
}

#[test]
fn uses_no_unpreserved_callee_saved_scratch_registers() {
    let output = assembly("fn main() -> i64 { return (2 + 3) * 4; }");

    for register in ["%rbx", "%r12", "%r13", "%r14", "%r15"] {
        assert!(!output.contains(register));
    }
    assert!(output.contains("pushq %rbp"));
    assert!(output.contains("leave"));
}

#[test]
fn emits_a_c_compatible_entry_boundary() {
    let output = assembly("fn helper() -> i64 { return 1; } fn main() -> i64 { return 2; }");

    assert!(output.contains(".globl main\n.type main, @function\nmain:"));
    assert!(output.contains("main:\n    pushq %rbp\n    movq %rsp, %rbp\n    call .Lska_fn_1"));
    assert!(!output.contains(".globl .Lska_fn_"));
}

#[test]
fn external_calls_use_the_declared_symbol_without_emitting_a_body() {
    let mir = lower_text(concat!(
        // Deliberately resembles an old internal symbol. The leading dot on
        // target-private symbols keeps the two namespaces disjoint.
        "extern fn ska_fn_1(value: i64) -> i64;\n",
        "fn main() -> i64 { return ska_fn_1(9); }\n",
    ));

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    assert!(output.contains("call ska_fn_1"));
    assert!(!output.contains("\nska_fn_1:\n"));
    assert!(output.contains(".Lska_fn_1:"));
}

#[test]
fn lowers_boolean_values_through_internal_and_external_abi_boundaries() {
    let output = assembly(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { return value; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(output.contains("movabsq $1, %rax"));
    assert!(output.contains("call .Lska_fn_1"));
    assert!(output.contains("call external_flag\n    movzbq %al, %rax"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
}

#[test]
fn lowers_forward_and_backward_jumps_in_stable_block_order() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    function.values.clear();
    function.body.blocks[0].instructions.clear();
    let second = BlockId::new(function.function, 1);
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: second,
        span,
    });
    function.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Goto {
            target: function.body.entry,
            span,
        }),
        span,
    });
    assert!(verify_mir(&mir).is_ok());

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    let first_position = output.find(".Lska_fn_0_block_0:").unwrap();
    let second_position = output.find(".Lska_fn_0_block_1:").unwrap();
    assert!(first_position < second_position);
    assert!(output.contains(".Lska_fn_0_block_0:\n    jmp .Lska_fn_0_block_1"));
    assert!(output.contains(".Lska_fn_0_block_1:\n    jmp .Lska_fn_0_block_0"));
}

#[test]
fn lowers_boolean_branches_and_returns_in_both_arms() {
    let output = emit_assembly(Target::X86_64SysV, &conditional_return_mir(true)).unwrap();

    assert!(output.contains(
        "movq -8(%rbp), %rax\n    testq %rax, %rax\n    jne .Lska_fn_0_block_1\n    jmp .Lska_fn_0_block_2"
    ));
    assert!(output.contains(".Lska_fn_0_block_1:\n    movabsq $37, %rax"));
    assert!(output.contains(".Lska_fn_0_block_2:\n    movabsq $12, %rax"));
    assert_eq!(output.matches("jmp .Lska_fn_0_epilogue").count(), 2);
    assert_eq!(output.matches(".Lska_fn_0_epilogue:").count(), 1);
}

#[test]
fn lowers_a_diamond_with_branch_local_calls_and_a_storage_join() {
    let output = emit_assembly(Target::X86_64SysV, &branch_call_diamond_mir()).unwrap();

    for index in 0..=3 {
        assert_eq!(
            output
                .matches(&format!(".Lska_fn_2_block_{index}:"))
                .count(),
            1
        );
    }
    assert!(output.contains(".Lska_fn_2_block_1:\n    call .Lska_fn_0"));
    assert!(output.contains(".Lska_fn_2_block_2:\n    call .Lska_fn_1"));
    assert_eq!(output.matches("jmp .Lska_fn_2_block_3").count(), 2);
    assert!(output.contains(".Lska_fn_2_block_3:\n    movq -8(%rbp), %rax"));
}

#[test]
fn jumps_to_a_non_first_entry_before_emitting_blocks_in_id_order() {
    let mut mir = conditional_return_mir(true);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.entry = function.body.blocks[1].id;
    assert!(verify_mir(&mir).is_ok());

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    let entry_jump = output.find("jmp .Lska_fn_0_block_1").unwrap();
    let first_block = output.find(".Lska_fn_0_block_0:").unwrap();
    let selected_block = output.find(".Lska_fn_0_block_1:").unwrap();
    assert!(entry_jump < first_block);
    assert!(first_block < selected_block);
}

#[test]
fn malformed_control_flow_is_a_structured_backend_error() {
    let mut mir = conditional_return_mir(true);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let Some(MirTerminator::Branch { true_target, .. }) = &mut function.body.blocks[0].terminator
    else {
        panic!("expected branch terminator");
    };
    *true_target = BlockId::new(function.function, 99);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error
        .message()
        .contains("control-flow target f0:b99 is not declared"));
}

#[test]
fn hand_built_conditional_executes_both_branch_directions() {
    for (condition, expected_status) in [(true, 37), (false, 12)] {
        let mir = conditional_return_mir(condition);
        let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
        let status = run_native_assembly(&output);

        assert_eq!(status.code(), Some(expected_status));
    }
}

#[test]
fn generated_text_is_accepted_by_the_system_assembler() {
    let straight_line = assembly(concat!(
        "fn calculate(a: i64, b: i64) -> i64 { return -a * b + 3; }\n",
        "fn main() -> i64 { return calculate(6, 7); }",
    ));
    let multi_block = emit_assembly(Target::X86_64SysV, &branch_call_diamond_mir()).unwrap();

    assert_system_assembler_accepts(&straight_line);
    assert_system_assembler_accepts(&multi_block);
}
