use super::*;
use crate::{
    external::{ExternalLink, ExternalLinkTable},
    identity::ExternalLinkId,
    mir::{MirClassDeclarationTable, MirInterfaceDeclarationTable},
};

#[derive(Clone, Copy, Debug)]
pub(super) enum PrimitiveValue {
    I64(i64),
    U64(u64),
    U8(u8),
    F64Bits(u64),
    Bool(bool),
}

impl PrimitiveValue {
    pub(super) const fn ty(self) -> MirPrimitiveType {
        match self {
            Self::I64(_) => MirPrimitiveType::I64,
            Self::U64(_) => MirPrimitiveType::U64,
            Self::U8(_) => MirPrimitiveType::U8,
            Self::F64Bits(_) => MirPrimitiveType::F64,
            Self::Bool(_) => MirPrimitiveType::Bool,
        }
    }

    fn rvalue(self) -> MirRvalueKind {
        match self {
            Self::I64(value) => MirRvalueKind::ConstantI64(value),
            Self::U64(value) => MirRvalueKind::ConstantU64(value),
            Self::U8(value) => MirRvalueKind::ConstantU8(value),
            Self::F64Bits(bits) => MirRvalueKind::ConstantF64Bits(bits),
            Self::Bool(value) => MirRvalueKind::ConstantBool(value),
        }
    }
}

pub(super) fn primitive_cast_program(
    source: PrimitiveValue,
    target: MirPrimitiveType,
) -> MirProgram {
    let span = test_span();
    let cast_id = FunctionId::new(0);
    let validate_id = FunctionId::new(1);
    let main_id = FunctionId::new(2);
    let source_type = source.ty().value_type();
    let target_type = target.value_type();

    let mut cast = fixture_function_definition(
        cast_id,
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![fixture_storage(
                StorageId::new(cast_id, 0),
                Some(BindingId::Local(LocalId::new(cast_id, 0))),
                "cast-result",
                MirStorageKind::Local,
                target_type,
                span,
            )],
            values: vec![
                fixture_value(ValueId::new(cast_id, 0), source_type, span),
                fixture_value(ValueId::new(cast_id, 1), target_type, span),
                fixture_value(ValueId::new(cast_id, 2), target_type, span),
            ],
            instructions: vec![
                fixture_assign(ValueId::new(cast_id, 0), source.rvalue(), source_type, span),
                fixture_assign(
                    ValueId::new(cast_id, 1),
                    MirRvalueKind::PrimitiveCast {
                        operation: MirPrimitiveCast::new(source.ty(), target),
                        operand: ValueId::new(cast_id, 0),
                    },
                    target_type,
                    span,
                ),
                fixture_store(
                    StorageId::new(cast_id, 0).into(),
                    ValueId::new(cast_id, 1),
                    span,
                ),
                fixture_assign(
                    ValueId::new(cast_id, 2),
                    MirRvalueKind::Load(StorageId::new(cast_id, 0).into()),
                    target_type,
                    span,
                ),
            ],
            terminator: Some(MirTerminator::Return {
                value: Some(ValueId::new(cast_id, 2)),
                span,
            }),
            span,
        },
    );
    fixture_add_body_storage_lifetimes(&cast.storage, &mut cast.body, span);

    let main = fixture_function_definition(
        main_id,
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![],
            values: vec![
                fixture_value(ValueId::new(main_id, 0), target_type, span),
                fixture_value(ValueId::new(main_id, 1), MirType::I64, span),
            ],
            instructions: vec![
                fixture_call(
                    MirCallTarget::Direct(cast_id),
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

    let program = MirProgram {
        modules: crate::module::ProgramModuleTable::singleton(
            span.source_id(),
            std::path::Path::new("main.ska"),
        ),
        external_links: ExternalLinkTable::new(vec![ExternalLink {
            id: ExternalLinkId::new(0),
            symbol: "validate_primitive_cast".to_owned(),
            declarations: vec![validate_id],
        }]),
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
                cast_id,
                "cast",
                vec![],
                target_type,
                MirFunctionLinkage::Internal,
                span,
            ),
            fixture_function_declaration(
                validate_id,
                "validate_primitive_cast",
                vec![fixture_parameter(MirParameterMode::Value, target_type)],
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
        definitions: MirFunctionDefinitionTable::new(vec![Some(cast), None, Some(main)]),
        entry_function: main_id,
        span,
    };
    verify_mir(&program).expect("primitive-cast backend fixture must be valid MIR");
    program
}
