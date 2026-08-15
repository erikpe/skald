use super::*;
use crate::{
    external::{ExternalLink, ExternalLinkTable},
    identity::ExternalLinkId,
    mir::{MirClassDeclarationTable, MirInterfaceDeclarationTable},
};

pub(super) fn integer_bitwise_program() -> MirProgram {
    fixture_integer_bitwise_program()
}

pub(super) fn eager_boolean_program() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let function_id = function.function;
    let value = |index, ty| fixture_value(ValueId::new(function_id, index), ty, span);
    let assignment =
        |index, kind, ty| fixture_assign(ValueId::new(function_id, index), kind, ty, span);
    let block = |index, instructions, condition, expected, next: BlockId, failure: BlockId| {
        fixture_block(
            BlockId::new(function_id, index),
            instructions,
            Some(MirTerminator::Branch {
                condition,
                true_target: if expected { next } else { failure },
                false_target: if expected { failure } else { next },
                span,
            }),
            span,
        )
    };

    function.values = (0..16)
        .map(|index| value(index, MirType::Bool))
        .chain((16..18).map(|index| value(index, MirType::I64)))
        .collect();

    let success = BlockId::new(function_id, 6);
    let failure = BlockId::new(function_id, 7);
    function.body.entry = BlockId::new(function_id, 0);
    function.body.blocks = vec![
        block(
            0,
            vec![
                assignment(0, MirRvalueKind::ConstantBool(false), MirType::Bool),
                assignment(
                    1,
                    MirRvalueKind::Unary {
                        operation: MirUnaryOperation::LogicalNotBool,
                        operand: ValueId::new(function_id, 0),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 1),
            true,
            BlockId::new(function_id, 1),
            failure,
        ),
        block(
            1,
            vec![
                assignment(2, MirRvalueKind::ConstantBool(true), MirType::Bool),
                assignment(
                    3,
                    MirRvalueKind::Unary {
                        operation: MirUnaryOperation::LogicalNotBool,
                        operand: ValueId::new(function_id, 2),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 3),
            false,
            BlockId::new(function_id, 2),
            failure,
        ),
        block(
            2,
            vec![
                assignment(4, MirRvalueKind::ConstantBool(true), MirType::Bool),
                assignment(5, MirRvalueKind::ConstantBool(true), MirType::Bool),
                assignment(
                    6,
                    MirRvalueKind::PrimitiveComparison {
                        operation: MirPrimitiveComparison {
                            predicate: MirComparisonPredicate::Equal,
                            operand: MirComparisonOperand::Bool,
                        },
                        left: ValueId::new(function_id, 4),
                        right: ValueId::new(function_id, 5),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 6),
            true,
            BlockId::new(function_id, 3),
            failure,
        ),
        block(
            3,
            vec![
                assignment(7, MirRvalueKind::ConstantBool(true), MirType::Bool),
                assignment(8, MirRvalueKind::ConstantBool(false), MirType::Bool),
                assignment(
                    9,
                    MirRvalueKind::PrimitiveComparison {
                        operation: MirPrimitiveComparison {
                            predicate: MirComparisonPredicate::Equal,
                            operand: MirComparisonOperand::Bool,
                        },
                        left: ValueId::new(function_id, 7),
                        right: ValueId::new(function_id, 8),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 9),
            false,
            BlockId::new(function_id, 4),
            failure,
        ),
        block(
            4,
            vec![
                assignment(10, MirRvalueKind::ConstantBool(false), MirType::Bool),
                assignment(11, MirRvalueKind::ConstantBool(false), MirType::Bool),
                assignment(
                    12,
                    MirRvalueKind::PrimitiveComparison {
                        operation: MirPrimitiveComparison {
                            predicate: MirComparisonPredicate::NotEqual,
                            operand: MirComparisonOperand::Bool,
                        },
                        left: ValueId::new(function_id, 10),
                        right: ValueId::new(function_id, 11),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 12),
            false,
            BlockId::new(function_id, 5),
            failure,
        ),
        block(
            5,
            vec![
                assignment(13, MirRvalueKind::ConstantBool(false), MirType::Bool),
                assignment(14, MirRvalueKind::ConstantBool(true), MirType::Bool),
                assignment(
                    15,
                    MirRvalueKind::PrimitiveComparison {
                        operation: MirPrimitiveComparison {
                            predicate: MirComparisonPredicate::NotEqual,
                            operand: MirComparisonOperand::Bool,
                        },
                        left: ValueId::new(function_id, 13),
                        right: ValueId::new(function_id, 14),
                    },
                    MirType::Bool,
                ),
            ],
            ValueId::new(function_id, 15),
            true,
            success,
            failure,
        ),
        fixture_block(
            success,
            vec![assignment(16, MirRvalueKind::ConstantI64(91), MirType::I64)],
            Some(MirTerminator::Return {
                value: Some(ValueId::new(function_id, 16)),
                span,
            }),
            span,
        ),
        fixture_block(
            failure,
            vec![assignment(17, MirRvalueKind::ConstantI64(1), MirType::I64)],
            Some(MirTerminator::Return {
                value: Some(ValueId::new(function_id, 17)),
                span,
            }),
            span,
        ),
    ];

    verify_mir(&mir).expect("eager boolean fixture must be valid");
    mir
}

pub(super) fn f64_arithmetic_program() -> MirProgram {
    let span = test_span();
    let compute_id = FunctionId::new(0);
    let validate_id = FunctionId::new(1);
    let main_id = FunctionId::new(2);
    let value = |function, index, ty| fixture_value(ValueId::new(function, index), ty, span);
    let assignment =
        |function, index, kind, ty| fixture_assign(ValueId::new(function, index), kind, ty, span);

    let mut compute = fixture_function_definition(
        compute_id,
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![fixture_storage(
                StorageId::new(compute_id, 0),
                Some(BindingId::Local(LocalId::new(compute_id, 0))),
                "result",
                MirStorageKind::Local,
                MirType::F64,
                span,
            )],
            values: (0..8)
                .map(|index| value(compute_id, index, MirType::F64))
                .collect(),
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
                fixture_store(
                    StorageId::new(compute_id, 0).into(),
                    ValueId::new(compute_id, 6),
                    span,
                ),
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
        },
    );
    fixture_add_body_storage_lifetimes(&compute.storage, &mut compute.body, span);

    let main = fixture_function_definition(
        main_id,
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![],
            values: vec![
                value(main_id, 0, MirType::F64),
                value(main_id, 1, MirType::I64),
            ],
            instructions: vec![
                fixture_call(
                    MirCallTarget::Direct(compute_id),
                    None,
                    vec![],
                    Some(ValueId::new(main_id, 0)),
                    None,
                    span,
                ),
                fixture_call(
                    MirCallTarget::Direct(validate_id),
                    None,
                    MirArgument::values([ValueId::new(main_id, 0)]),
                    Some(ValueId::new(main_id, 1)),
                    None,
                    span,
                ),
            ],
            terminator: Some(MirTerminator::Return {
                value: Some(ValueId::new(main_id, 1)),
                span,
            }),
            span,
        },
    );

    MirProgram {
        modules: crate::module::ProgramModuleTable::singleton(
            span.source_id(),
            std::path::Path::new("main.ska"),
        ),
        external_links: ExternalLinkTable::new(vec![ExternalLink {
            id: ExternalLinkId::new(0),
            symbol: "validate_f64".to_owned(),
            declarations: vec![validate_id],
        }]),
        function_types: Default::default(),
        array_types: Default::default(),
        optional_types: Default::default(),
        optional_box_types: Default::default(),
        string_language_item: None,
        literal_data: Default::default(),
        classes: MirClassDeclarationTable::default(),
        interfaces: MirInterfaceDeclarationTable::default(),
        virtual_families: MirVirtualFamilyTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
        static_lifecycle: None,
        declarations: MirFunctionDeclarationTable::new(vec![
            fixture_function_declaration(
                compute_id,
                "compute",
                vec![],
                MirType::F64,
                MirFunctionLinkage::Internal,
                span,
            ),
            fixture_function_declaration(
                validate_id,
                "validate_f64",
                vec![fixture_parameter(MirParameterMode::Value, MirType::F64)],
                MirType::I64,
                MirFunctionLinkage::External {
                    link: ExternalLinkId::new(0),
                },
                span,
            ),
            fixture_function_declaration(
                main_id,
                "main",
                vec![],
                MirType::I64,
                MirFunctionLinkage::Internal,
                span,
            ),
        ]),
        definitions: MirFunctionDefinitionTable::new(vec![Some(compute), None, Some(main)]),
        entry_function: main_id,
        span,
    }
}

pub(super) fn f64_division_program(dividend_bits: u64, divisor_bits: u64) -> MirProgram {
    let mut program = f64_arithmetic_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let span = function.span;
    let function_id = function.function;
    let block = function.body.entry;
    let value = |index| fixture_value(ValueId::new(function_id, index), MirType::F64, span);
    let assignment =
        |index, kind| fixture_assign(ValueId::new(function_id, index), kind, MirType::F64, span);

    function.storage.clear();
    function.values = (0..3).map(value).collect();
    function.body.blocks = vec![fixture_block(
        block,
        vec![
            assignment(0, MirRvalueKind::ConstantF64Bits(dividend_bits)),
            assignment(1, MirRvalueKind::ConstantF64Bits(divisor_bits)),
            assignment(
                2,
                MirRvalueKind::Binary {
                    operation: MirBinaryOperation::DivideF64,
                    left: ValueId::new(function_id, 0),
                    right: ValueId::new(function_id, 1),
                },
            ),
        ],
        Some(MirTerminator::Return {
            value: Some(ValueId::new(function_id, 2)),
            span,
        }),
        span,
    )];

    verify_mir(&program).expect("floating division fixture must be valid");
    program
}

pub(super) fn f64_comparison_program(
    predicate: MirComparisonPredicate,
    left_bits: u64,
    right_bits: u64,
    expected: bool,
) -> MirProgram {
    let mut program = lower_text("fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let span = function.span;
    let function_id = function.function;
    let entry = BlockId::new(function_id, 0);
    let success = BlockId::new(function_id, 1);
    let failure = BlockId::new(function_id, 2);
    let value = |index, ty| fixture_value(ValueId::new(function_id, index), ty, span);
    let assignment =
        |index, kind, ty| fixture_assign(ValueId::new(function_id, index), kind, ty, span);

    function.values = vec![
        value(0, MirType::F64),
        value(1, MirType::F64),
        value(2, MirType::Bool),
        value(3, MirType::I64),
        value(4, MirType::I64),
    ];
    function.body.entry = entry;
    function.body.blocks = vec![
        fixture_block(
            entry,
            vec![
                assignment(0, MirRvalueKind::ConstantF64Bits(left_bits), MirType::F64),
                assignment(1, MirRvalueKind::ConstantF64Bits(right_bits), MirType::F64),
                assignment(
                    2,
                    MirRvalueKind::PrimitiveComparison {
                        operation: MirPrimitiveComparison {
                            predicate,
                            operand: MirComparisonOperand::F64,
                        },
                        left: ValueId::new(function_id, 0),
                        right: ValueId::new(function_id, 1),
                    },
                    MirType::Bool,
                ),
            ],
            Some(MirTerminator::Branch {
                condition: ValueId::new(function_id, 2),
                true_target: if expected { success } else { failure },
                false_target: if expected { failure } else { success },
                span,
            }),
            span,
        ),
        fixture_block(
            success,
            vec![assignment(3, MirRvalueKind::ConstantI64(0), MirType::I64)],
            Some(MirTerminator::Return {
                value: Some(ValueId::new(function_id, 3)),
                span,
            }),
            span,
        ),
        fixture_block(
            failure,
            vec![assignment(4, MirRvalueKind::ConstantI64(1), MirType::I64)],
            Some(MirTerminator::Return {
                value: Some(ValueId::new(function_id, 4)),
                span,
            }),
            span,
        ),
    ];

    verify_mir(&program).expect("floating comparison fixture must be valid");
    program
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
        .map(|(index, ty)| {
            fixture_storage(
                StorageId::new(mixed_id, index),
                Some(BindingId::Parameter(ParameterId::new(mixed_id, index))),
                format!("p{index}"),
                MirStorageKind::Parameter,
                *ty,
                span,
            )
        })
        .collect();
    let parameters = storage.iter().map(|storage| storage.id).collect();
    let mixed = fixture_function_definition(
        mixed_id,
        OneBlockDefinition {
            return_storage: None,
            parameters,
            storage,
            values: vec![fixture_value(ValueId::new(mixed_id, 0), MirType::F64, span)],
            instructions: vec![fixture_assign(
                ValueId::new(mixed_id, 0),
                MirRvalueKind::Load(StorageId::new(mixed_id, 15).into()),
                MirType::F64,
                span,
            )],
            terminator: Some(MirTerminator::Return {
                value: Some(ValueId::new(mixed_id, 0)),
                span,
            }),
            span,
        },
    );

    let mut values = Vec::new();
    let mut instructions = Vec::new();
    for (index, ty) in parameter_types.iter().copied().enumerate() {
        values.push(fixture_value(ValueId::new(main_id, index), ty, span));
        let kind = if ty == MirType::F64 {
            MirRvalueKind::ConstantF64Bits((index as f64).to_bits())
        } else {
            MirRvalueKind::ConstantI64(index as i64)
        };
        instructions.push(fixture_assign(ValueId::new(main_id, index), kind, ty, span));
    }
    let call_result = ValueId::new(main_id, values.len());
    values.push(fixture_value(call_result, MirType::F64, span));
    instructions.push(fixture_call(
        MirCallTarget::Direct(mixed_id),
        None,
        MirArgument::values((0..parameter_types.len()).map(|index| ValueId::new(main_id, index))),
        Some(call_result),
        None,
        span,
    ));
    let return_value = ValueId::new(main_id, values.len());
    values.push(fixture_value(return_value, MirType::I64, span));
    instructions.push(fixture_assign(
        return_value,
        MirRvalueKind::ConstantI64(0),
        MirType::I64,
        span,
    ));
    let main = fixture_function_definition(
        main_id,
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![],
            values,
            instructions,
            terminator: Some(MirTerminator::Return {
                value: Some(return_value),
                span,
            }),
            span,
        },
    );

    MirProgram {
        modules: crate::module::ProgramModuleTable::singleton(
            span.source_id(),
            std::path::Path::new("main.ska"),
        ),
        external_links: ExternalLinkTable::default(),
        function_types: Default::default(),
        array_types: Default::default(),
        optional_types: Default::default(),
        optional_box_types: Default::default(),
        string_language_item: None,
        literal_data: Default::default(),
        classes: MirClassDeclarationTable::default(),
        interfaces: MirInterfaceDeclarationTable::default(),
        virtual_families: MirVirtualFamilyTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
        static_lifecycle: None,
        declarations: MirFunctionDeclarationTable::new(vec![
            fixture_function_declaration(
                mixed_id,
                "mixed",
                parameter_types
                    .into_iter()
                    .map(|ty| fixture_parameter(MirParameterMode::Value, ty))
                    .collect(),
                MirType::F64,
                MirFunctionLinkage::Internal,
                span,
            ),
            fixture_function_declaration(
                main_id,
                "main",
                vec![],
                MirType::I64,
                MirFunctionLinkage::Internal,
                span,
            ),
        ]),
        definitions: MirFunctionDefinitionTable::new(vec![Some(mixed), Some(main)]),
        entry_function: main_id,
        span,
    }
}
