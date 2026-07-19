//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        HirBinaryOperation, HirBlock, HirExpression, HirExpressionKind, HirFunctionDeclaration,
        HirFunctionDefinition, HirFunctionLinkage, HirProgram, HirStatement, HirUnaryOperation,
        Type,
    },
    resolve::BindingId,
};

use super::model::*;

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
    instructions: Vec<MirInstruction>,
    terminator: Option<MirTerminator>,
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
            instructions: Vec::new(),
            terminator: None,
        };
        lowerer.allocate_storage();
        lowerer.lower_block(&definition.body);
        if lowerer.terminator.is_none() && declaration.return_type == Type::Unit {
            lowerer.terminator = Some(MirTerminator::Return {
                value: None,
                span: definition.body.span,
            });
        }
        assert!(
            lowerer.terminator.is_some(),
            "type-checked function must lower to a terminated entry block"
        );

        let entry = BlockId::new(declaration.id, 0);
        MirFunctionDefinition {
            function: declaration.id,
            parameters: lowerer.parameter_storage,
            storage: lowerer.storage,
            values: lowerer.values,
            body: MirBody {
                entry,
                blocks: vec![MirBasicBlock {
                    id: entry,
                    instructions: lowerer.instructions,
                    terminator: lowerer.terminator,
                    span: definition.body.span,
                }],
            },
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
            if self.terminator.is_some() {
                break;
            }
            match statement {
                HirStatement::Local(local) => {
                    let value = self
                        .lower_expression(&local.initializer)
                        .expect("typed local initializer must produce a value");
                    let storage = self.local_storage[local.local.index()];
                    self.instructions.push(MirInstruction::Store(MirStore {
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
                    self.terminator = Some(MirTerminator::Return {
                        value,
                        span: statement.span,
                    });
                }
                HirStatement::Call(statement) => {
                    let result = self.lower_expression(&statement.call);
                    assert!(result.is_none(), "typed call statement must return unit");
                }
                HirStatement::Block(block) => self.lower_block(block),
            }
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
                self.instructions.push(MirInstruction::Call(MirCall {
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
        self.instructions
            .push(MirInstruction::Assign(MirAssignment {
                result,
                rvalue: MirRvalue { kind, ty },
                span,
            }));
        result
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
        Type::Unit => MirType::Unit,
    }
}
