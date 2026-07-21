//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        BlockFlow, HirAccess, HirBinaryOperation, HirBlock, HirCallArgument, HirClassDeclaration,
        HirConditional, HirExpression, HirExpressionKind, HirFunctionDeclaration,
        HirFunctionDefinition, HirFunctionLinkage, HirLocal, HirMemberDefinition, HirParameter,
        HirParameterMode, HirProgram, HirStatement, HirUnaryOperation, Type,
    },
    identity::{BindingId, CallableId, ClassId},
};

use super::{build::MirBodyBuilder, model::*};

pub fn lower_hir(hir: &HirProgram) -> MirProgram {
    let classes = hir.classes.iter().map(lower_class_declaration).collect();
    let declarations = hir.declarations.iter().map(lower_declaration).collect();
    let definitions = hir
        .declarations
        .iter()
        .map(|declaration| {
            hir.definitions
                .get(declaration.id)
                .map(|definition| lower_function_definition(declaration, definition))
        })
        .collect();
    let member_definitions = hir
        .class_definitions
        .iter()
        .flat_map(|class| std::iter::once(&class.initializer).chain(class.methods.iter()))
        .map(|definition| lower_member_definition(hir, definition))
        .collect();
    let mir = MirProgram {
        classes: MirClassDeclarationTable::new(classes),
        declarations: MirFunctionDeclarationTable::new(declarations),
        definitions: MirFunctionDefinitionTable::new(definitions),
        member_definitions: MirMemberDefinitionTable::new(member_definitions),
        entry_function: hir.entry_function,
        span: hir.span,
    };

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    mir
}

fn lower_class_declaration(class: &HirClassDeclaration) -> MirClassDeclaration {
    MirClassDeclaration {
        id: class.id,
        name: class.name.clone(),
        fields: class
            .fields
            .iter()
            .map(|field| MirFieldDeclaration {
                id: field.id,
                name: field.name.clone(),
                ty: lower_type(field.ty),
                span: field.span,
            })
            .collect(),
        initializers: vec![MirInitializerDeclaration {
            id: class.initializer.id,
            parameters: class
                .initializer
                .parameters
                .iter()
                .map(lower_parameter)
                .collect(),
            span: class.initializer.span,
        }],
        methods: class
            .methods
            .iter()
            .map(|method| MirMethodDeclaration {
                id: method.id,
                name: method.name.clone(),
                receiver_access: match method.receiver_access {
                    HirAccess::ReadOnly => MirReceiverAccess::ReadOnly,
                    HirAccess::Mutable => MirReceiverAccess::Mutable,
                },
                parameters: method.parameters.iter().map(lower_parameter).collect(),
                return_type: lower_type(method.return_type),
                span: method.span,
            })
            .collect(),
        span: class.span,
    }
}

fn lower_declaration(declaration: &HirFunctionDeclaration) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id: declaration.id,
        name: declaration.name.clone(),
        parameters: declaration.parameters.iter().map(lower_parameter).collect(),
        return_type: lower_type(declaration.return_type),
        linkage: match &declaration.linkage {
            HirFunctionLinkage::Internal => MirFunctionLinkage::Internal,
            HirFunctionLinkage::External { symbol } => MirFunctionLinkage::External {
                symbol: symbol.clone(),
            },
        },
        span: declaration.span,
    }
}

fn lower_function_definition(
    declaration: &HirFunctionDeclaration,
    definition: &HirFunctionDefinition,
) -> MirFunctionDefinition {
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: declaration.id.into(),
        parameters: &declaration.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: declaration.return_type,
        receiver_class: None,
    });
    MirFunctionDefinition {
        function: declaration.id,
        parameters: lowered.parameters,
        storage: lowered.storage,
        values: lowered.values,
        body: lowered.body,
        span: definition.span,
    }
}

fn lower_member_definition(
    hir: &HirProgram,
    definition: &HirMemberDefinition,
) -> MirMemberDefinition {
    let signature = hir
        .callable_signature(definition.callable)
        .expect("typed member definition must have a signature");
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: definition.callable,
        parameters: signature.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: signature.return_type,
        receiver_class: definition.callable.class(),
    });
    MirMemberDefinition {
        callable: definition.callable,
        receiver: lowered.receiver.expect("member body must lower a receiver"),
        parameters: lowered.parameters,
        storage: lowered.storage,
        values: lowered.values,
        body: lowered.body,
        span: definition.span,
    }
}

struct BodyLoweringInput<'hir> {
    callable: CallableId,
    parameters: &'hir [HirParameter],
    locals: &'hir [HirLocal],
    source_body: &'hir HirBlock,
    return_type: Type,
    receiver_class: Option<ClassId>,
}

struct LoweredBody {
    receiver: Option<StorageId>,
    parameters: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBody,
}

struct BodyLowerer<'hir> {
    input: BodyLoweringInput<'hir>,
    receiver_storage: Option<StorageId>,
    parameter_storage: Vec<StorageId>,
    local_storage: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBodyBuilder,
}

impl<'hir> BodyLowerer<'hir> {
    fn lower(input: BodyLoweringInput<'hir>) -> LoweredBody {
        let mut lowerer = Self {
            parameter_storage: Vec::with_capacity(input.parameters.len()),
            local_storage: Vec::with_capacity(input.locals.len()),
            storage: Vec::with_capacity(
                input.parameters.len()
                    + input.locals.len()
                    + usize::from(input.receiver_class.is_some()),
            ),
            values: Vec::new(),
            body: MirBodyBuilder::new(input.callable, input.source_body.span),
            receiver_storage: None,
            input,
        };
        lowerer.allocate_storage();
        lowerer.lower_block(lowerer.input.source_body);
        if !lowerer.body.is_current_terminated() && lowerer.input.return_type == Type::Unit {
            lowerer.terminate(MirTerminator::Return {
                value: None,
                span: lowerer.input.source_body.span,
            });
        }
        assert!(
            lowerer.body.is_current_terminated(),
            "type-checked callable must lower to a terminated entry block"
        );
        LoweredBody {
            receiver: lowerer.receiver_storage,
            parameters: lowerer.parameter_storage,
            storage: lowerer.storage,
            values: lowerer.values,
            body: lowerer.body.finish(),
        }
    }

    fn allocate_storage(&mut self) {
        if let Some(class) = self.input.receiver_class {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.receiver_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: BindingId::Receiver(self.input.callable),
                name: "self".to_owned(),
                kind: MirStorageKind::Receiver,
                ty: MirType::Class(class),
                span: self.input.source_body.span,
            });
        }
        for parameter in self.input.parameters {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.parameter_storage.push(id);
            self.storage.push(MirStorage {
                id,
                source: BindingId::Parameter(parameter.id),
                name: parameter.name.clone(),
                kind: match parameter.mode {
                    HirParameterMode::Value => MirStorageKind::Parameter,
                    HirParameterMode::ReadOnlyAlias => {
                        MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly)
                    }
                    HirParameterMode::MutableAlias => {
                        MirStorageKind::AliasParameter(MirAliasAccess::Mutable)
                    }
                },
                ty: lower_type(parameter.ty),
                span: parameter.span,
            });
        }
        for local in self.input.locals {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.local_storage.push(id);
            self.storage.push(MirStorage {
                id,
                source: BindingId::Local(local.id),
                name: local.name.clone(),
                kind: MirStorageKind::Local,
                ty: lower_type(local.ty),
                span: local.span,
            });
        }
    }

    fn lower_block(&mut self, block: &HirBlock) {
        for statement in &block.statements {
            if self.body.is_current_terminated() {
                break;
            }
            match statement {
                HirStatement::Local(local) => {
                    let storage = self.local_storage[local.local.index()];
                    match &local.initializer {
                        crate::hir::HirLocalInitializer::Value(initializer) => {
                            let value = self
                                .lower_expression(initializer)
                                .expect("typed scalar local initializer must produce a value");
                            self.emit(MirInstruction::Store(MirStore {
                                destination: storage.into(),
                                value,
                                span: local.span,
                            }));
                        }
                        crate::hir::HirLocalInitializer::Construct(construction) => {
                            let arguments = self.lower_call_arguments(&construction.arguments);
                            self.emit(MirInstruction::Initialize(MirInitialize {
                                destination: storage.into(),
                                target: construction.initializer,
                                arguments,
                                span: construction.span,
                            }));
                        }
                    }
                }
                HirStatement::Return(statement) => {
                    let value = statement.value.as_ref().map(|value| {
                        self.lower_expression(value)
                            .expect("typed return expression must produce a value")
                    });
                    self.terminate(MirTerminator::Return {
                        value,
                        span: statement.span,
                    });
                }
                HirStatement::Call(statement) => {
                    let result = self.lower_expression(&statement.call);
                    assert!(result.is_none(), "typed call statement must return unit");
                }
                HirStatement::Conditional(conditional) => {
                    self.lower_conditional(conditional);
                }
                HirStatement::Block(block) => self.lower_block(block),
                HirStatement::FieldAssignment(assignment) => {
                    // The receiver place is selected before the value. Stage-0
                    // receivers are stable bindings and emit no instructions.
                    let destination = self.lower_field_place(&assignment.place);
                    let value = self
                        .lower_expression(&assignment.value)
                        .expect("typed field assignment must produce a scalar value");
                    self.emit(MirInstruction::Store(MirStore {
                        destination,
                        value,
                        span: assignment.span,
                    }));
                }
            }
        }
    }

    fn lower_conditional(&mut self, conditional: &HirConditional) {
        debug_assert!(!conditional.arms.is_empty());
        let needs_join = conditional.flow == BlockFlow::FallsThrough;

        // Allocate the complete shape before emitting edges. IDs therefore
        // follow source structure rather than a traversal chosen by lowering:
        // condition, body, next condition, body, else, join.
        let mut condition_blocks = vec![self.body.current()];
        let mut body_blocks = Vec::with_capacity(conditional.arms.len());
        for (index, arm) in conditional.arms.iter().enumerate() {
            body_blocks.push(self.body.allocate_block(arm.body.span));
            if index + 1 < conditional.arms.len() {
                condition_blocks.push(
                    self.body
                        .allocate_block(conditional.arms[index + 1].condition.span),
                );
            }
        }
        let else_block = conditional
            .else_block
            .as_ref()
            .map(|block| self.body.allocate_block(block.span));
        let join_block = needs_join.then(|| self.body.allocate_block(conditional.span));

        for (index, arm) in conditional.arms.iter().enumerate() {
            self.body
                .select_block(condition_blocks[index])
                .expect("allocated conditional block must be selectable");
            let condition = self
                .lower_expression(&arm.condition)
                .expect("typed conditional condition must produce a value");
            let false_target = condition_blocks
                .get(index + 1)
                .copied()
                .or(else_block)
                .or(join_block)
                .expect("a conditional's false path must have a target");
            self.terminate(MirTerminator::Branch {
                condition,
                true_target: body_blocks[index],
                false_target,
                span: arm.span,
            });

            self.body
                .select_block(body_blocks[index])
                .expect("allocated conditional body must be selectable");
            self.lower_block(&arm.body);
            if !self.body.is_current_terminated() {
                self.terminate(MirTerminator::Goto {
                    target: join_block.expect("a falling-through arm requires a join block"),
                    span: arm.body.span,
                });
            }
        }

        if let (Some(source), Some(block)) = (&conditional.else_block, else_block) {
            self.body
                .select_block(block)
                .expect("allocated else block must be selectable");
            self.lower_block(source);
            if !self.body.is_current_terminated() {
                self.terminate(MirTerminator::Goto {
                    target: join_block.expect("a falling-through else requires a join block"),
                    span: source.span,
                });
            }
        }

        if let Some(join) = join_block {
            self.body
                .select_block(join)
                .expect("allocated conditional join must be selectable");
        }
    }

    fn lower_expression(&mut self, expression: &HirExpression) -> Option<ValueId> {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                let storage = self.storage_for_binding(*binding);
                Some(self.assign(
                    MirRvalueKind::Load(storage.into()),
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::I64(value) => Some(self.assign(
                MirRvalueKind::ConstantI64(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::U64(value) => Some(self.assign(
                MirRvalueKind::ConstantU64(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::U8(value) => Some(self.assign(
                MirRvalueKind::ConstantU8(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::F64Bits(bits) => Some(self.assign(
                MirRvalueKind::ConstantF64Bits(*bits),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::Boolean(value) => Some(self.assign(
                MirRvalueKind::ConstantBool(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::Unary { operation, operand } => {
                let operand = self
                    .lower_expression(operand)
                    .expect("typed unary operand must produce a value");
                Some(self.assign(
                    MirRvalueKind::Unary {
                        operation: match operation {
                            HirUnaryOperation::NegateI64 => MirUnaryOperation::NegateI64,
                            HirUnaryOperation::NegateF64 => MirUnaryOperation::NegateF64,
                        },
                        operand,
                    },
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                // This order is semantic: left is fully lowered before right.
                let left = self
                    .lower_expression(left)
                    .expect("typed binary operand must produce a value");
                let right = self
                    .lower_expression(right)
                    .expect("typed binary operand must produce a value");
                Some(self.assign(
                    MirRvalueKind::Binary {
                        operation: match operation {
                            HirBinaryOperation::AddI64 => MirBinaryOperation::AddI64,
                            HirBinaryOperation::SubtractI64 => MirBinaryOperation::SubtractI64,
                            HirBinaryOperation::MultiplyI64 => MirBinaryOperation::MultiplyI64,
                            HirBinaryOperation::AddU64 => MirBinaryOperation::AddU64,
                            HirBinaryOperation::SubtractU64 => MirBinaryOperation::SubtractU64,
                            HirBinaryOperation::MultiplyU64 => MirBinaryOperation::MultiplyU64,
                            HirBinaryOperation::AddU8 => MirBinaryOperation::AddU8,
                            HirBinaryOperation::SubtractU8 => MirBinaryOperation::SubtractU8,
                            HirBinaryOperation::MultiplyU8 => MirBinaryOperation::MultiplyU8,
                            HirBinaryOperation::AddF64 => MirBinaryOperation::AddF64,
                            HirBinaryOperation::SubtractF64 => MirBinaryOperation::SubtractF64,
                            HirBinaryOperation::MultiplyF64 => MirBinaryOperation::MultiplyF64,
                        },
                        left,
                        right,
                    },
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => {
                // Argument evaluation is likewise fixed left-to-right.
                let arguments = self.lower_call_arguments(arguments);
                let result = (expression.ty != Type::Unit)
                    .then(|| self.new_value(lower_type(expression.ty), expression.span));
                self.emit(MirInstruction::Call(MirCall {
                    target: MirCallTarget::Direct(*function),
                    receiver: None,
                    arguments,
                    result,
                    span: expression.span,
                }));
                result
            }
            HirExpressionKind::Grouped(inner) => self.lower_expression(inner),
            HirExpressionKind::FieldRead(place) => Some(self.assign(
                MirRvalueKind::Load(self.lower_field_place(place)),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::MethodCall {
                receiver,
                method,
                arguments,
            } => {
                // Receiver selection precedes all explicit argument effects.
                let receiver = self.lower_object_place(receiver);
                let arguments = self.lower_call_arguments(arguments);
                let result = (expression.ty != Type::Unit)
                    .then(|| self.new_value(lower_type(expression.ty), expression.span));
                self.emit(MirInstruction::Call(MirCall {
                    target: MirCallTarget::Method(*method),
                    receiver: Some(receiver),
                    arguments,
                    result,
                    span: expression.span,
                }));
                result
            }
        }
    }

    fn lower_field_place(&self, place: &crate::hir::HirFieldPlace) -> MirPlace {
        self.lower_object_place(&place.receiver)
            .project_field(place.field)
    }

    fn lower_call_arguments(&mut self, arguments: &[HirCallArgument]) -> Vec<MirArgument> {
        arguments
            .iter()
            .map(|argument| match argument {
                HirCallArgument::Value(expression) => MirArgument::Value(
                    self.lower_expression(expression)
                        .expect("typed value argument must produce a scalar value"),
                ),
                HirCallArgument::Place(place) => MirArgument::Place(self.lower_object_place(place)),
            })
            .collect()
    }

    fn lower_object_place(&self, place: &crate::hir::HirObjectPlace) -> MirPlace {
        let storage = self.storage_for_binding(place.root());
        let root = match self.storage[storage.index()].kind {
            MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
            MirStorageKind::Receiver | MirStorageKind::Parameter | MirStorageKind::Local => {
                MirPlace::base(storage)
            }
        };
        place
            .projections()
            .iter()
            .fold(root, |projected, &field| projected.project_field(field))
    }

    fn storage_for_binding(&self, binding: BindingId) -> StorageId {
        assert_eq!(
            binding.callable(),
            self.input.callable,
            "typed binding must belong to the current callable"
        );
        match binding {
            BindingId::Receiver(_) => self
                .receiver_storage
                .expect("receiver binding requires member receiver storage"),
            BindingId::Parameter(id) => self.parameter_storage[id.index()],
            BindingId::Local(id) => self.local_storage[id.index()],
        }
    }

    fn assign(&mut self, kind: MirRvalueKind, ty: MirType, span: crate::source::Span) -> ValueId {
        let result = self.new_value(ty, span);
        self.emit(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span,
        }));
        result
    }

    fn emit(&mut self, instruction: MirInstruction) {
        self.body
            .push_instruction(instruction)
            .expect("HIR lowering must not emit after a terminator");
    }

    fn terminate(&mut self, terminator: MirTerminator) {
        self.body
            .terminate(terminator)
            .expect("HIR lowering must terminate each block exactly once");
    }

    fn new_value(&mut self, ty: MirType, span: crate::source::Span) -> ValueId {
        assert!(
            ty.is_scalar_value(),
            "typed HIR lowering must not materialize a non-scalar MIR value"
        );
        let result = ValueId::new(self.input.callable, self.values.len());
        self.values.push(MirValue {
            id: result,
            ty,
            span,
        });
        result
    }
}

fn lower_parameter(parameter: &HirParameter) -> MirParameter {
    let ty = lower_type(parameter.ty);
    match parameter.mode {
        HirParameterMode::Value => MirParameter::value(ty),
        HirParameterMode::ReadOnlyAlias => MirParameter::read_only_alias(ty),
        HirParameterMode::MutableAlias => MirParameter::mutable_alias(ty),
    }
}

const fn lower_type(ty: Type) -> MirType {
    match ty {
        Type::I64 => MirType::I64,
        Type::U64 => MirType::U64,
        Type::U8 => MirType::U8,
        Type::F64 => MirType::F64,
        Type::Bool => MirType::Bool,
        Type::Unit => MirType::Unit,
        Type::Class(class) => MirType::Class(class),
    }
}
