//! Small, explicit constructors for hand-built MIR in unit tests.
//!
//! These helpers remove structural boilerplate without supplying semantic
//! defaults. Tests still name every identity, type, ownership mode, and span
//! that participates in the contract under examination.

use crate::{
    identity::{BindingId, CallableId, ClassId, FunctionId, InitializerId, ModuleId, ParameterId},
    source::Span,
    test_support::lower_source_to_mir,
};

use super::{
    BlockId, MirAliasAccess, MirArgument, MirAssignment, MirBasicBlock, MirBinaryOperation,
    MirBody, MirCall, MirCallTarget, MirCleanup, MirComparisonOperand, MirComparisonPredicate,
    MirEndFullExpression, MirFunctionDeclaration, MirFunctionDefinition, MirFunctionLinkage,
    MirInitialize, MirInstruction, MirIntegerBitwiseOperation, MirIntegerType, MirMemberDefinition,
    MirMethodReceiver, MirParameter, MirParameterMode, MirPathCondition, MirPathConditionValue,
    MirPlace, MirPrimitiveComparison, MirProgram, MirRvalue, MirRvalueKind, MirStorage,
    MirStorageDead, MirStorageKind, MirStorageLive, MirStore, MirTerminator, MirType,
    MirUnaryOperation, MirValue, PathConditionId, StorageId, ValueId,
};

pub(crate) const fn parameter(mode: MirParameterMode, ty: MirType) -> MirParameter {
    MirParameter { mode, ty }
}

pub(crate) fn function_declaration(
    id: FunctionId,
    name: impl Into<String>,
    parameters: Vec<MirParameter>,
    return_type: MirType,
    linkage: MirFunctionLinkage,
    span: Span,
) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id,
        module: ModuleId::new(0),
        name: name.into(),
        parameters,
        return_type,
        linkage,
        span,
    }
}

pub(crate) fn storage(
    id: StorageId,
    source: Option<BindingId>,
    name: impl Into<String>,
    kind: MirStorageKind,
    ty: MirType,
    span: Span,
) -> MirStorage {
    MirStorage {
        id,
        source,
        name: name.into(),
        kind,
        ty,
        span,
    }
}

pub(crate) fn storage_live(storage: StorageId, span: Span) -> MirInstruction {
    MirInstruction::StorageLive(MirStorageLive { storage, span })
}

pub(crate) fn storage_dead(storage: StorageId, span: Span) -> MirInstruction {
    MirInstruction::StorageDead(MirStorageDead { storage, span })
}

/// Wraps storage that a hand-built fixture models as live for its complete
/// callable body. Source-derived fixtures should retain their precise lowered
/// lexical and full-expression epochs instead.
pub(crate) fn add_body_storage_lifetimes(storage: &[MirStorage], body: &mut MirBody, span: Span) {
    let managed: Vec<_> = storage
        .iter()
        .filter(|storage| {
            !matches!(
                storage.kind,
                MirStorageKind::Return
                    | MirStorageKind::Receiver
                    | MirStorageKind::Parameter
                    | MirStorageKind::AliasParameter(_)
            )
        })
        .map(|storage| storage.id)
        .collect();
    if managed.is_empty() {
        return;
    }

    for block in &mut body.blocks {
        block.instructions.retain(|instruction| match instruction {
            MirInstruction::StorageLive(operation) => !managed.contains(&operation.storage),
            MirInstruction::StorageDead(operation) => !managed.contains(&operation.storage),
            _ => true,
        });
    }

    let entry = body
        .blocks
        .iter_mut()
        .find(|block| block.id == body.entry)
        .expect("hand-built fixture entry block must exist");
    entry.instructions.splice(
        0..0,
        managed
            .iter()
            .copied()
            .map(|storage| storage_live(storage, span)),
    );
    for block in &mut body.blocks {
        if matches!(block.terminator, Some(MirTerminator::Return { .. })) {
            block.instructions.extend(
                managed
                    .iter()
                    .rev()
                    .copied()
                    .map(|storage| storage_dead(storage, span)),
            );
        }
    }
}

pub(crate) fn receiver_storage(id: StorageId, class: ClassId, span: Span) -> MirStorage {
    storage(
        id,
        Some(BindingId::Receiver(id.callable())),
        "self",
        MirStorageKind::Receiver,
        MirType::Class(class),
        span,
    )
}

pub(crate) fn empty_member_definition(
    callable: CallableId,
    class: ClassId,
    parameters: &[MirParameter],
    span: Span,
) -> MirMemberDefinition {
    let receiver = StorageId::new(callable, 0);
    let mut storage = vec![receiver_storage(receiver, class, span)];
    storage.extend(
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| storage_for_parameter(callable, index, *parameter, span)),
    );
    MirMemberDefinition {
        callable,
        class_owner: callable
            .class()
            .expect("member fixture needs a class owner"),
        return_storage: None,
        receiver: Some(receiver),
        parameters: storage.iter().skip(1).map(|storage| storage.id).collect(),
        storage,
        values: vec![],
        body: one_block_body(
            callable,
            vec![],
            Some(MirTerminator::Return { value: None, span }),
            span,
        ),
        span,
    }
}

fn storage_for_parameter(
    callable: CallableId,
    index: usize,
    parameter: MirParameter,
    span: Span,
) -> MirStorage {
    storage(
        StorageId::new(callable, index + 1),
        Some(BindingId::Parameter(ParameterId::new(callable, index))),
        format!("parameter{index}"),
        match parameter.mode {
            MirParameterMode::Value => MirStorageKind::Parameter,
            MirParameterMode::ReadOnlyAlias => {
                MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly)
            }
            MirParameterMode::MutableAlias => {
                MirStorageKind::AliasParameter(MirAliasAccess::Mutable)
            }
        },
        parameter.ty,
        span,
    )
}

pub(crate) const fn value(id: ValueId, ty: MirType, span: Span) -> MirValue {
    MirValue { id, ty, span }
}

pub(crate) fn assign(
    result: ValueId,
    kind: MirRvalueKind,
    ty: MirType,
    span: Span,
) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue { kind, ty },
        span,
    })
}

#[derive(Clone, Copy)]
enum BitwiseFixtureOperation {
    Complement,
    Binary(MirIntegerBitwiseOperation),
}

#[derive(Clone, Copy)]
struct BitwiseFixtureCase {
    integer: MirIntegerType,
    operation: BitwiseFixtureOperation,
    left_bits: u64,
    right_bits: u64,
    expected_bits: u64,
}

const BITWISE_FIXTURE_CASES: [BitwiseFixtureCase; 12] = [
    BitwiseFixtureCase {
        integer: MirIntegerType::I64,
        operation: BitwiseFixtureOperation::Complement,
        left_bits: 0x5555_5555_5555_5555,
        right_bits: 0,
        expected_bits: 0xaaaa_aaaa_aaaa_aaaa,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::I64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::And),
        left_bits: u64::MAX,
        right_bits: 0x00ff_00ff_00ff_00ff,
        expected_bits: 0x00ff_00ff_00ff_00ff,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::I64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Or),
        left_bits: 0x8000_0000_0000_0000,
        right_bits: 0xff,
        expected_bits: 0x8000_0000_0000_00ff,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::I64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Xor),
        left_bits: u64::MAX,
        right_bits: 0xaaaa_aaaa_aaaa_aaaa,
        expected_bits: 0x5555_5555_5555_5555,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U64,
        operation: BitwiseFixtureOperation::Complement,
        left_bits: 0,
        right_bits: 0,
        expected_bits: u64::MAX,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::And),
        left_bits: 0xaaaa_aaaa_aaaa_aaaa,
        right_bits: 0xf0f0_f0f0_f0f0_f0f0,
        expected_bits: 0xa0a0_a0a0_a0a0_a0a0,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Or),
        left_bits: 0x8000_0000_0000_0000,
        right_bits: 0x7fff_ffff_ffff_ffff,
        expected_bits: u64::MAX,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U64,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Xor),
        left_bits: u64::MAX,
        right_bits: 0xaaaa_aaaa_aaaa_aaaa,
        expected_bits: 0x5555_5555_5555_5555,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U8,
        operation: BitwiseFixtureOperation::Complement,
        left_bits: 0x55,
        right_bits: 0,
        expected_bits: 0xaa,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U8,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::And),
        left_bits: 0xf3,
        right_bits: 0x3f,
        expected_bits: 0x33,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U8,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Or),
        left_bits: 0x80,
        right_bits: 0x0f,
        expected_bits: 0x8f,
    },
    BitwiseFixtureCase {
        integer: MirIntegerType::U8,
        operation: BitwiseFixtureOperation::Binary(MirIntegerBitwiseOperation::Xor),
        left_bits: 0xff,
        right_bits: 0xa5,
        expected_bits: 0x5a,
    },
];

fn fixture_integer_constant(integer: MirIntegerType, bits: u64) -> MirRvalueKind {
    match integer {
        MirIntegerType::I64 => MirRvalueKind::ConstantI64(bits as i64),
        MirIntegerType::U64 => MirRvalueKind::ConstantU64(bits),
        MirIntegerType::U8 => MirRvalueKind::ConstantU8(bits as u8),
    }
}

fn next_fixture_value(
    function: FunctionId,
    values: &mut Vec<MirValue>,
    ty: MirType,
    span: Span,
) -> ValueId {
    let id = ValueId::new(function, values.len());
    values.push(value(id, ty, span));
    id
}

/// Hand-built verified MIR covering every pure integer bitwise operation and
/// width without enabling its source syntax.
pub(crate) fn integer_bitwise_program() -> MirProgram {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .expect("fixture entry function exists");
    let function_id = function.function;
    let span = function.span;
    let success = BlockId::new(function_id, BITWISE_FIXTURE_CASES.len());
    let failure = BlockId::new(function_id, BITWISE_FIXTURE_CASES.len() + 1);
    let mut values = Vec::new();
    let mut blocks = Vec::new();

    for (index, case) in BITWISE_FIXTURE_CASES.into_iter().enumerate() {
        let ty = case.integer.operand_type();
        let left = next_fixture_value(function_id, &mut values, ty, span);
        let mut instructions = vec![assign(
            left,
            fixture_integer_constant(case.integer, case.left_bits),
            ty,
            span,
        )];
        let result = match case.operation {
            BitwiseFixtureOperation::Complement => {
                let result = next_fixture_value(function_id, &mut values, ty, span);
                instructions.push(assign(
                    result,
                    MirRvalueKind::Unary {
                        operation: MirUnaryOperation::BitwiseComplement(case.integer),
                        operand: left,
                    },
                    ty,
                    span,
                ));
                result
            }
            BitwiseFixtureOperation::Binary(operation) => {
                let right = next_fixture_value(function_id, &mut values, ty, span);
                instructions.push(assign(
                    right,
                    fixture_integer_constant(case.integer, case.right_bits),
                    ty,
                    span,
                ));
                let result = next_fixture_value(function_id, &mut values, ty, span);
                instructions.push(assign(
                    result,
                    MirRvalueKind::Binary {
                        operation: MirBinaryOperation::IntegerBitwise {
                            operation,
                            operand: case.integer,
                        },
                        left,
                        right,
                    },
                    ty,
                    span,
                ));
                result
            }
        };
        let expected = next_fixture_value(function_id, &mut values, ty, span);
        instructions.push(assign(
            expected,
            fixture_integer_constant(case.integer, case.expected_bits),
            ty,
            span,
        ));
        let comparison = next_fixture_value(function_id, &mut values, MirType::Bool, span);
        instructions.push(assign(
            comparison,
            MirRvalueKind::PrimitiveComparison {
                operation: MirPrimitiveComparison {
                    predicate: MirComparisonPredicate::Equal,
                    operand: MirComparisonOperand::Integer(case.integer),
                },
                left: result,
                right: expected,
            },
            MirType::Bool,
            span,
        ));

        blocks.push(block(
            BlockId::new(function_id, index),
            instructions,
            Some(MirTerminator::Branch {
                condition: comparison,
                true_target: if index + 1 == BITWISE_FIXTURE_CASES.len() {
                    success
                } else {
                    BlockId::new(function_id, index + 1)
                },
                false_target: failure,
                span,
            }),
            span,
        ));
    }

    for (block_id, result) in [(success, 91), (failure, 1)] {
        let value = next_fixture_value(function_id, &mut values, MirType::I64, span);
        blocks.push(block(
            block_id,
            vec![assign(
                value,
                MirRvalueKind::ConstantI64(result),
                MirType::I64,
                span,
            )],
            Some(MirTerminator::Return {
                value: Some(value),
                span,
            }),
            span,
        ));
    }

    function.values = values;
    function.storage.clear();
    function.parameters.clear();
    function.body = MirBody {
        entry: BlockId::new(function_id, 0),
        blocks,
        path_conditions: Vec::new(),
        logical_expressions: Vec::new(),
    };

    super::verify_mir(&program).expect("integer bitwise fixture must be valid");
    program
}

pub(crate) fn call(
    target: MirCallTarget,
    receiver: Option<MirMethodReceiver>,
    arguments: Vec<MirArgument>,
    result: Option<ValueId>,
    destination: Option<MirPlace>,
    span: Span,
) -> MirInstruction {
    MirInstruction::Call(MirCall {
        target,
        receiver: receiver.map(Into::into),
        arguments,
        result,
        shared_result: None,
        destination,
        span,
    })
}

pub(crate) fn store(destination: MirPlace, value: ValueId, span: Span) -> MirInstruction {
    MirInstruction::Store(MirStore {
        destination,
        value,
        span,
    })
}

pub(crate) fn block(
    id: BlockId,
    instructions: Vec<MirInstruction>,
    terminator: Option<MirTerminator>,
    span: Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id,
        instructions,
        terminator,
        span,
    }
}

pub(crate) fn one_block_body(
    callable: CallableId,
    instructions: Vec<MirInstruction>,
    terminator: Option<MirTerminator>,
    span: Span,
) -> MirBody {
    let entry = BlockId::new(callable, 0);
    MirBody {
        entry,
        blocks: vec![block(entry, instructions, terminator, span)],
        path_conditions: Vec::new(),
        logical_expressions: Vec::new(),
    }
}

pub(crate) struct OneBlockDefinition {
    pub(crate) return_storage: Option<StorageId>,
    pub(crate) parameters: Vec<StorageId>,
    pub(crate) storage: Vec<MirStorage>,
    pub(crate) values: Vec<MirValue>,
    pub(crate) instructions: Vec<MirInstruction>,
    pub(crate) terminator: Option<MirTerminator>,
    pub(crate) span: Span,
}

pub(crate) fn function_definition(
    function: FunctionId,
    definition: OneBlockDefinition,
) -> MirFunctionDefinition {
    MirFunctionDefinition {
        function,
        return_storage: definition.return_storage,
        parameters: definition.parameters,
        storage: definition.storage,
        values: definition.values,
        body: one_block_body(
            function.into(),
            definition.instructions,
            definition.terminator,
            definition.span,
        ),
        span: definition.span,
    }
}

pub(crate) fn member_definition(
    callable: CallableId,
    receiver: Option<StorageId>,
    definition: OneBlockDefinition,
) -> MirMemberDefinition {
    MirMemberDefinition {
        callable,
        class_owner: callable
            .class()
            .expect("member fixture needs a class owner"),
        return_storage: definition.return_storage,
        receiver,
        parameters: definition.parameters,
        storage: definition.storage,
        values: definition.values,
        body: one_block_body(
            callable,
            definition.instructions,
            definition.terminator,
            definition.span,
        ),
        span: definition.span,
    }
}

/// One selected inline temporary plus its complete conditional cleanup graph.
///
/// This fixture is shared by MIR verification and backend legality tests. It
/// deliberately starts from declared source metadata and hand-builds a narrow
/// malformed/valid mutation surface that source lowering does not expose.
pub(crate) fn conditional_full_expression_cleanup_program() -> MirProgram {
    let mut mir =
        lower_source_to_mir("class Token { init() {} }\nfn main() -> i64 { return 0; }\n");
    let function = mir.entry_function;
    let callable = CallableId::Function(function);
    let span = mir.definitions.get(function).unwrap().span;
    let class = ClassId::new(0);
    let initializer = InitializerId::new(class, 0);
    let activation = StorageId::new(callable, 0);
    let first = StorageId::new(callable, 1);
    let second = StorageId::new(callable, 2);
    let result = StorageId::new(callable, 3);
    let condition = PathConditionId::new(callable, 0);
    let blocks: Vec<_> = (0..10).map(|index| BlockId::new(callable, index)).collect();
    let values: Vec<_> = [
        MirType::Bool,
        MirType::Bool,
        MirType::Bool,
        MirType::I64,
        MirType::Bool,
        MirType::Bool,
        MirType::I64,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| value(ValueId::new(callable, index), ty, span))
    .collect();
    let cleanup = |storage| MirCleanup {
        destination: MirPlace::base(storage),
        target: class,
        span,
    };

    let definition = mir.definitions.get_mut_for_test(function).unwrap();
    definition.storage = vec![
        storage(
            activation,
            None,
            "condition",
            MirStorageKind::PathCondition,
            MirType::Bool,
            span,
        ),
        storage(
            first,
            None,
            "first",
            MirStorageKind::Temporary,
            MirType::Class(class),
            span,
        ),
        storage(
            second,
            None,
            "second",
            MirStorageKind::Temporary,
            MirType::Class(class),
            span,
        ),
        storage(
            result,
            None,
            "result",
            MirStorageKind::ScalarSpill,
            MirType::I64,
            span,
        ),
    ];
    definition.values = values;
    definition.body = MirBody {
        entry: blocks[0],
        blocks: vec![
            block(
                blocks[0],
                vec![
                    storage_live(activation, span),
                    storage_live(result, span),
                    assign(
                        ValueId::new(callable, 0),
                        MirRvalueKind::ConstantBool(true),
                        MirType::Bool,
                        span,
                    ),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(callable, 0),
                    true_target: blocks[1],
                    false_target: blocks[2],
                    span,
                }),
                span,
            ),
            block(
                blocks[1],
                vec![
                    storage_live(first, span),
                    MirInstruction::Initialize(MirInitialize {
                        destination: MirPlace::base(first),
                        target: initializer,
                        arguments: Vec::new(),
                        span,
                    }),
                    storage_live(second, span),
                    MirInstruction::Initialize(MirInitialize {
                        destination: MirPlace::base(second),
                        target: initializer,
                        arguments: Vec::new(),
                        span,
                    }),
                    assign(
                        ValueId::new(callable, 1),
                        MirRvalueKind::ConstantBool(true),
                        MirType::Bool,
                        span,
                    ),
                    store(MirPlace::base(activation), ValueId::new(callable, 1), span),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            block(
                blocks[2],
                vec![
                    assign(
                        ValueId::new(callable, 2),
                        MirRvalueKind::ConstantBool(false),
                        MirType::Bool,
                        span,
                    ),
                    store(MirPlace::base(activation), ValueId::new(callable, 2), span),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            block(
                blocks[3],
                vec![
                    assign(
                        ValueId::new(callable, 3),
                        MirRvalueKind::ConstantI64(41),
                        MirType::I64,
                        span,
                    ),
                    store(MirPlace::base(result), ValueId::new(callable, 3), span),
                    assign(
                        ValueId::new(callable, 4),
                        MirRvalueKind::PathCondition(MirPathConditionValue {
                            condition,
                            activation,
                        }),
                        MirType::Bool,
                        span,
                    ),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(callable, 4),
                    true_target: blocks[4],
                    false_target: blocks[5],
                    span,
                }),
                span,
            ),
            block(
                blocks[4],
                vec![MirInstruction::EndFullExpression(MirEndFullExpression {
                    temporaries: vec![cleanup(second), cleanup(first)],
                    span,
                })],
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            block(
                blocks[5],
                Vec::new(),
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            block(
                blocks[6],
                vec![
                    MirInstruction::EndFullExpression(MirEndFullExpression {
                        temporaries: Vec::new(),
                        span,
                    }),
                    assign(
                        ValueId::new(callable, 5),
                        MirRvalueKind::PathCondition(MirPathConditionValue {
                            condition,
                            activation,
                        }),
                        MirType::Bool,
                        span,
                    ),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(callable, 5),
                    true_target: blocks[7],
                    false_target: blocks[8],
                    span,
                }),
                span,
            ),
            block(
                blocks[7],
                vec![storage_dead(second, span), storage_dead(first, span)],
                Some(MirTerminator::Goto {
                    target: blocks[9],
                    span,
                }),
                span,
            ),
            block(
                blocks[8],
                Vec::new(),
                Some(MirTerminator::Goto {
                    target: blocks[9],
                    span,
                }),
                span,
            ),
            block(
                blocks[9],
                vec![
                    storage_dead(activation, span),
                    assign(
                        ValueId::new(callable, 6),
                        MirRvalueKind::Load(MirPlace::base(result)),
                        MirType::I64,
                        span,
                    ),
                    storage_dead(result, span),
                ],
                Some(MirTerminator::Return {
                    value: Some(ValueId::new(callable, 6)),
                    span,
                }),
                span,
            ),
        ],
        path_conditions: vec![MirPathCondition {
            id: condition,
            parent: None,
            activation,
            active_predecessor: blocks[1],
            inactive_predecessor: blocks[2],
            merge: blocks[3],
            span,
        }],
        logical_expressions: vec![],
    };
    mir
}

#[cfg(test)]
mod tests {
    use crate::{
        identity::{ClassId, MethodId},
        source::SourceDatabase,
    };

    use super::*;

    #[test]
    fn fixtures_preserve_explicit_identity_type_ownership_and_span_inputs() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("mir-fixtures.ska", "");
        let span = Span::empty(source, 0);
        let function = FunctionId::new(0);
        let result = ValueId::new(function, 0);
        let home = StorageId::new(function, 0);
        let parameter_metadata = parameter(MirParameterMode::MutableAlias, MirType::I64);
        let declaration = function_declaration(
            function,
            "fixture",
            vec![parameter_metadata],
            MirType::I64,
            MirFunctionLinkage::Internal,
            span,
        );
        let storage_metadata = storage(
            home,
            None,
            "home",
            MirStorageKind::Local,
            MirType::I64,
            span,
        );
        let assignment = assign(result, MirRvalueKind::ConstantI64(7), MirType::I64, span);
        let store = store(home.into(), result, span);
        let call = call(
            MirCallTarget::Direct(function),
            None,
            vec![MirArgument::Value(result)],
            Some(result),
            None,
            span,
        );
        let definition = function_definition(
            function,
            OneBlockDefinition {
                return_storage: None,
                parameters: vec![home],
                storage: vec![storage_metadata.clone()],
                values: vec![value(result, MirType::I64, span)],
                instructions: vec![assignment, store, call],
                terminator: Some(MirTerminator::Return {
                    value: Some(result),
                    span,
                }),
                span,
            },
        );

        assert_eq!(declaration.parameters, [parameter_metadata]);
        assert_eq!(definition.storage, [storage_metadata]);
        assert_eq!(definition.body.entry, BlockId::new(function, 0));
        assert_eq!(definition.body.blocks[0].instructions.len(), 3);
        assert_eq!(definition.span, span);

        let class = ClassId::new(0);
        let method = MethodId::new(class, 0);
        let receiver = StorageId::new(method, 0);
        let receiver_metadata = receiver_storage(receiver, class, span);
        let member = member_definition(
            method.into(),
            Some(receiver),
            OneBlockDefinition {
                return_storage: None,
                parameters: Vec::new(),
                storage: vec![receiver_metadata.clone()],
                values: Vec::new(),
                instructions: Vec::new(),
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            },
        );

        assert_eq!(member.receiver, Some(receiver));
        assert_eq!(member.storage, [receiver_metadata]);
        assert_eq!(member.body.entry.callable(), method.into());

        let receiverless = member_definition(
            method.into(),
            None,
            OneBlockDefinition {
                return_storage: None,
                parameters: Vec::new(),
                storage: Vec::new(),
                values: Vec::new(),
                instructions: Vec::new(),
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            },
        );
        assert_eq!(receiverless.class_owner, class);
        assert_eq!(receiverless.receiver, None);
        assert!(receiverless.storage.is_empty());
    }

    #[test]
    fn fixtures_keep_deliberately_malformed_mir_representable() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("malformed-mir-fixture.ska", "");
        let span = Span::empty(source, 0);
        let function = FunctionId::new(0);
        let result = ValueId::new(function, 0);
        let mismatched = assign(
            result,
            MirRvalueKind::ConstantBool(true),
            MirType::I64,
            span,
        );
        let unfinished = one_block_body(function.into(), vec![mismatched], None, span);

        assert!(unfinished.blocks[0].terminator.is_none());
        let MirInstruction::Assign(assignment) = &unfinished.blocks[0].instructions[0] else {
            panic!("expected assignment fixture");
        };
        assert_eq!(assignment.rvalue.ty, MirType::I64);
        assert!(matches!(
            assignment.rvalue.kind,
            MirRvalueKind::ConstantBool(true)
        ));
    }
}
