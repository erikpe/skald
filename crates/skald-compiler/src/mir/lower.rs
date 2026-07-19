//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        HirBinaryOperation, HirBlock, HirConditional, HirExpression, HirExpressionKind,
        HirFunctionDeclaration, HirFunctionDefinition, HirFunctionLinkage, HirProgram,
        HirStatement, HirUnaryOperation, Type,
    },
    resolve::BindingId,
};

use super::{build::MirBodyBuilder, model::*};

pub fn lower_hir(hir: &HirProgram) -> MirProgram {
    let declarations = hir.declarations.iter().map(lower_declaration).collect();
    let definitions = hir
        .declarations
        .iter()
        .map(|declaration| {
            hir.definitions
                .get(declaration.id)
                .map(|definition| FunctionLowerer::lower(declaration, definition))
        })
        .collect();
    let mir = MirProgram {
        declarations: MirFunctionDeclarationTable::new(declarations),
        definitions: MirFunctionDefinitionTable::new(definitions),
        entry_function: hir.entry_function,
        span: hir.span,
    };

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    mir
}

fn lower_declaration(declaration: &HirFunctionDeclaration) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id: declaration.id,
        name: declaration.name.clone(),
        parameter_types: declaration
            .parameters
            .iter()
            .map(|parameter| lower_type(parameter.ty))
            .collect(),
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

struct FunctionLowerer<'hir> {
    declaration: &'hir HirFunctionDeclaration,
    definition: &'hir HirFunctionDefinition,
    parameter_storage: Vec<StorageId>,
    local_storage: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBodyBuilder,
}

impl<'hir> FunctionLowerer<'hir> {
    fn lower(
        declaration: &'hir HirFunctionDeclaration,
        definition: &'hir HirFunctionDefinition,
    ) -> MirFunctionDefinition {
        let mut lowerer = Self {
            declaration,
            definition,
            parameter_storage: Vec::with_capacity(declaration.parameters.len()),
            local_storage: Vec::with_capacity(definition.locals.len()),
            storage: Vec::with_capacity(declaration.parameters.len() + definition.locals.len()),
            values: Vec::new(),
            body: MirBodyBuilder::new(declaration.id, definition.body.span),
        };
        lowerer.allocate_storage();
        lowerer.lower_block(&definition.body);
        if !lowerer.body.is_current_terminated() && declaration.return_type == Type::Unit {
            lowerer.terminate(MirTerminator::Return {
                value: None,
                span: definition.body.span,
            });
        }
        assert!(
            lowerer.body.is_current_terminated(),
            "type-checked function must lower to a terminated entry block"
        );

        MirFunctionDefinition {
            function: declaration.id,
            parameters: lowerer.parameter_storage,
            storage: lowerer.storage,
            values: lowerer.values,
            body: lowerer.body.finish(),
            span: definition.span,
        }
    }

    fn allocate_storage(&mut self) {
        for parameter in &self.declaration.parameters {
            let id = StorageId::new(self.declaration.id, self.storage.len());
            self.parameter_storage.push(id);
            self.storage.push(MirStorage {
                id,
                source: BindingId::Parameter(parameter.id),
                name: parameter.name.clone(),
                kind: MirStorageKind::Parameter,
                ty: lower_type(parameter.ty),
                span: parameter.span,
            });
        }
        for local in &self.definition.locals {
            let id = StorageId::new(self.declaration.id, self.storage.len());
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
                    let value = self
                        .lower_expression(&local.initializer)
                        .expect("typed local initializer must produce a value");
                    let storage = self.local_storage[local.local.index()];
                    self.emit(MirInstruction::Store(MirStore {
                        storage,
                        value,
                        span: local.span,
                    }));
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
            }
        }
    }

    fn lower_conditional(&mut self, conditional: &HirConditional) {
        debug_assert!(!conditional.arms.is_empty());
        let needs_join = conditional.else_block.is_none()
            || conditional
                .arms
                .iter()
                .any(|arm| !hir_block_guarantees_return(&arm.body))
            || conditional
                .else_block
                .as_ref()
                .is_some_and(|block| !hir_block_guarantees_return(block));

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
                let storage = match binding {
                    BindingId::Parameter(id) => self.parameter_storage[id.index()],
                    BindingId::Local(id) => self.local_storage[id.index()],
                };
                Some(self.assign(
                    MirRvalueKind::Load(storage),
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::Integer(value) => Some(self.assign(
                MirRvalueKind::ConstantI64(*value),
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
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        self.lower_expression(argument)
                            .expect("typed call argument must produce a value")
                    })
                    .collect();
                let result = (expression.ty != Type::Unit)
                    .then(|| self.new_value(lower_type(expression.ty), expression.span));
                self.emit(MirInstruction::Call(MirCall {
                    target: MirCallTarget::Direct(*function),
                    arguments,
                    result,
                    span: expression.span,
                }));
                result
            }
            HirExpressionKind::Grouped(inner) => self.lower_expression(inner),
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
        let result = ValueId::new(self.declaration.id, self.values.len());
        self.values.push(MirValue {
            id: result,
            ty,
            span,
        });
        result
    }
}

const fn lower_type(ty: Type) -> MirType {
    match ty {
        Type::I64 => MirType::I64,
        Type::Bool => MirType::Bool,
        Type::Unit => MirType::Unit,
    }
}

fn hir_block_guarantees_return(block: &HirBlock) -> bool {
    block.statements.iter().any(|statement| match statement {
        HirStatement::Return(_) => true,
        HirStatement::Block(block) => hir_block_guarantees_return(block),
        HirStatement::Conditional(conditional) => {
            conditional
                .else_block
                .as_ref()
                .is_some_and(hir_block_guarantees_return)
                && conditional
                    .arms
                    .iter()
                    .all(|arm| hir_block_guarantees_return(&arm.body))
        }
        HirStatement::Local(_) | HirStatement::Call(_) => false,
    })
}
