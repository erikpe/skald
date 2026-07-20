use super::*;
use crate::mir::MirClassDeclarationTable;

pub(super) fn f64_arithmetic_program() -> MirProgram {
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
                        destination: StorageId::new(compute_id, 0).into(),
                        value: ValueId::new(compute_id, 6),
                        span,
                    }),
                    assignment(
                        compute_id,
                        7,
                        MirRvalueKind::Load(StorageId::new(compute_id, 0).into()),
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
                        receiver: None,
                        arguments: vec![],
                        result: Some(ValueId::new(main_id, 0)),
                        span,
                    }),
                    MirInstruction::Call(MirCall {
                        target: MirCallTarget::Direct(validate_id),
                        receiver: None,
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
        classes: MirClassDeclarationTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
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

pub(super) fn mixed_exhausted_abi_program() -> MirProgram {
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
                        kind: MirRvalueKind::Load(StorageId::new(mixed_id, 15).into()),
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
        receiver: None,
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
        classes: MirClassDeclarationTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
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
