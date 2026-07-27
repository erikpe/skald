use super::*;
use crate::mir::{MirClassDeclarationTable, MirInterfaceDeclarationTable};

pub(super) fn f64_arithmetic_program() -> MirProgram {
    let span = test_span();
    let compute_id = FunctionId::new(0);
    let validate_id = FunctionId::new(1);
    let main_id = FunctionId::new(2);
    let value = |function, index, ty| fixture_value(ValueId::new(function, index), ty, span);
    let assignment =
        |function, index, kind, ty| fixture_assign(ValueId::new(function, index), kind, ty, span);

    let compute = fixture_function_definition(
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
        modules: crate::module::ProgramModuleTable::singleton(span.source_id()),
        array_types: Default::default(),
        classes: MirClassDeclarationTable::default(),
        interfaces: MirInterfaceDeclarationTable::default(),
        virtual_families: MirVirtualFamilyTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
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
                    symbol: "validate_f64".to_owned(),
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
        modules: crate::module::ProgramModuleTable::singleton(span.source_id()),
        array_types: Default::default(),
        classes: MirClassDeclarationTable::default(),
        interfaces: MirInterfaceDeclarationTable::default(),
        virtual_families: MirVirtualFamilyTable::default(),
        member_definitions: MirMemberDefinitionTable::default(),
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
