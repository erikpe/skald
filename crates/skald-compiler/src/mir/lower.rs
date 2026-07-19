//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        HirBinaryOperation, HirBlock, HirExpression, HirExpressionKind, HirFunction, HirProgram,
        HirStatement, HirUnaryOperation, Type,
    },
    resolve::BindingId,
};

use super::model::*;

pub fn lower_hir(hir: &HirProgram) -> MirProgram {
    let functions = hir.functions.iter().map(FunctionLowerer::lower).collect();
    let mir = MirProgram {
        functions: MirFunctionTable::new(functions),
        entry_function: hir.entry_function,
        span: hir.span,
    };

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    mir
}

struct FunctionLowerer<'hir> {
    function: &'hir HirFunction,
    parameter_storage: Vec<StorageId>,
    local_storage: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    instructions: Vec<MirInstruction>,
    terminator: Option<MirTerminator>,
}

impl<'hir> FunctionLowerer<'hir> {
    fn lower(function: &'hir HirFunction) -> MirFunction {
        let mut lowerer = Self {
            function,
            parameter_storage: Vec::with_capacity(function.parameters.len()),
            local_storage: Vec::with_capacity(function.locals.len()),
            storage: Vec::with_capacity(function.parameters.len() + function.locals.len()),
            values: Vec::new(),
            instructions: Vec::new(),
            terminator: None,
        };
        lowerer.allocate_storage();
        lowerer.lower_block(&function.body);
        assert!(
            lowerer.terminator.is_some(),
            "type-checked function must lower to a terminated entry block"
        );

        let entry = BlockId::new(function.id, 0);
        MirFunction {
            id: function.id,
            name: function.name.clone(),
            parameters: lowerer.parameter_storage,
            return_type: lower_type(function.return_type),
            storage: lowerer.storage,
            values: lowerer.values,
            body: MirBody {
                entry,
                blocks: vec![MirBasicBlock {
                    id: entry,
                    instructions: lowerer.instructions,
                    terminator: lowerer.terminator,
                    span: function.body.span,
                }],
            },
            span: function.span,
        }
    }

    fn allocate_storage(&mut self) {
        for parameter in &self.function.parameters {
            let id = StorageId::new(self.function.id, self.storage.len());
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
        for local in &self.function.locals {
            let id = StorageId::new(self.function.id, self.storage.len());
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
                    let value = self.lower_expression(&local.initializer);
                    let storage = self.local_storage[local.local.index()];
                    self.instructions.push(MirInstruction::Store(MirStore {
                        storage,
                        value,
                        span: local.span,
                    }));
                }
                HirStatement::Return(statement) => {
                    let value = self.lower_expression(&statement.value);
                    self.terminator = Some(MirTerminator::Return {
                        value,
                        span: statement.span,
                    });
                }
                HirStatement::Block(block) => self.lower_block(block),
            }
        }
    }

    fn lower_expression(&mut self, expression: &HirExpression) -> ValueId {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                let storage = match binding {
                    BindingId::Parameter(id) => self.parameter_storage[id.index()],
                    BindingId::Local(id) => self.local_storage[id.index()],
                };
                self.assign(
                    MirRvalueKind::Load(storage),
                    lower_type(expression.ty),
                    expression.span,
                )
            }
            HirExpressionKind::Integer(value) => self.assign(
                MirRvalueKind::ConstantI64(*value),
                lower_type(expression.ty),
                expression.span,
            ),
            HirExpressionKind::Unary { operation, operand } => {
                let operand = self.lower_expression(operand);
                self.assign(
                    MirRvalueKind::Unary {
                        operation: match operation {
                            HirUnaryOperation::NegateI64 => MirUnaryOperation::NegateI64,
                        },
                        operand,
                    },
                    lower_type(expression.ty),
                    expression.span,
                )
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                // This order is semantic: left is fully lowered before right.
                let left = self.lower_expression(left);
                let right = self.lower_expression(right);
                self.assign(
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
                )
            }
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => {
                // Argument evaluation is likewise fixed left-to-right.
                let arguments = arguments
                    .iter()
                    .map(|argument| self.lower_expression(argument))
                    .collect();
                self.assign(
                    MirRvalueKind::DirectCall {
                        function: *function,
                        arguments,
                    },
                    lower_type(expression.ty),
                    expression.span,
                )
            }
            HirExpressionKind::Grouped(inner) => self.lower_expression(inner),
        }
    }

    fn assign(&mut self, kind: MirRvalueKind, ty: MirType, span: crate::source::Span) -> ValueId {
        let result = ValueId::new(self.function.id, self.values.len());
        self.values.push(MirValue {
            id: result,
            ty,
            span,
        });
        self.instructions
            .push(MirInstruction::Assign(MirAssignment {
                result,
                rvalue: MirRvalue { kind, ty },
                span,
            }));
        result
    }
}

const fn lower_type(ty: Type) -> MirType {
    match ty {
        Type::I64 => MirType::I64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hir::HirProgram,
        lexer::lex,
        resolve::{resolve, FunctionId},
        source::SourceDatabase,
        syntax::parse,
        typeck::type_check,
    };

    fn hir_text(text: &str) -> HirProgram {
        let mut sources = SourceDatabase::new();
        let source_id = sources.add("test.ska", text);
        let source = sources.get(source_id).unwrap();
        let lexed = lex(source);
        assert!(lexed.diagnostics.is_empty());
        let parsed = parse(source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty());
        let resolved = resolve(&parsed.ast);
        assert!(resolved.diagnostics.is_empty());
        let checked = type_check(&resolved.program);
        assert!(checked.diagnostics.is_empty());
        checked.hir.unwrap()
    }

    fn lower_text(text: &str) -> MirProgram {
        lower_hir(&hir_text(text))
    }

    #[test]
    fn lowers_storage_values_arithmetic_and_return_explicitly() {
        let mir = lower_text("fn main() -> i64 { var result: i64 = 1; return result + 2; }");
        assert!(super::super::verify_mir(&mir).is_ok());
        let function = mir.functions.get(mir.entry_function).unwrap();

        assert_eq!(function.storage.len(), 1);
        assert_eq!(function.storage[0].kind, MirStorageKind::Local);
        assert_eq!(function.values.len(), 4);
        let block = function.block(function.body.entry).unwrap();
        assert_eq!(block.instructions.len(), 5);
        assert!(matches!(
            block.instructions[0],
            MirInstruction::Assign(MirAssignment {
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantI64(1),
                    ..
                },
                ..
            })
        ));
        assert!(matches!(block.instructions[1], MirInstruction::Store(_)));
        assert!(matches!(
            block.instructions[4],
            MirInstruction::Assign(MirAssignment {
                rvalue: MirRvalue {
                    kind: MirRvalueKind::Binary {
                        operation: MirBinaryOperation::AddI64,
                        ..
                    },
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            block.terminator,
            Some(MirTerminator::Return { .. })
        ));
    }

    #[test]
    fn nested_call_arguments_lower_in_deterministic_left_to_right_order() {
        let mir = lower_text(concat!(
            "fn left() -> i64 { return 1; }\n",
            "fn right() -> i64 { return 2; }\n",
            "fn combine(a: i64, b: i64) -> i64 { return a + b; }\n",
            "fn main() -> i64 { return combine(left(), right()); }\n",
        ));
        let main = mir.functions.get(mir.entry_function).unwrap();
        let block = main.block(main.body.entry).unwrap();
        let calls: Vec<_> = block
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                MirInstruction::Assign(MirAssignment {
                    rvalue:
                        MirRvalue {
                            kind: MirRvalueKind::DirectCall { function, .. },
                            ..
                        },
                    ..
                }) => Some(*function),
                _ => None,
            })
            .collect();

        assert_eq!(
            calls.iter().map(|id| id.index()).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn lowering_discards_statements_after_an_unconditional_return() {
        let mir = lower_text("fn main() -> i64 { { return 1; } return 2; }");
        let main = mir.functions.get(mir.entry_function).unwrap();
        let block = main.block(main.body.entry).unwrap();

        assert_eq!(main.values.len(), 1);
        assert_eq!(block.instructions.len(), 1);
        assert!(matches!(
            block.terminator,
            Some(MirTerminator::Return { .. })
        ));
    }

    #[test]
    fn mir_dump_is_deterministic() {
        let mir = lower_text("fn main() -> i64 { return 42; }");

        assert_eq!(
            super::super::dump_mir(&mir),
            concat!(
                "MirProgram @0..31\n",
                "  Entry f0\n",
                "  Functions\n",
                "    Function f0 \"main\" -> i64 @0..31\n",
                "      Parameters\n",
                "      Storage\n",
                "      Values\n",
                "        f0:v0 : i64 @26..28\n",
                "      EntryBlock f0:b0\n",
                "      Blocks\n",
                "        f0:b0 @17..31\n",
                "          f0:v0 = const.i64 42 : i64 @26..28\n",
                "          return f0:v0 @19..29\n",
            )
        );
    }

    #[test]
    fn verifier_rejects_unterminated_blocks() {
        let mut mir = lower_text("fn main() -> i64 { return 0; }");
        mir.functions.entries_mut_for_test()[0].body.blocks[0].terminator = None;

        let errors = super::super::verify_mir(&mir).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("no terminator")));
    }

    #[test]
    fn verifier_rejects_use_before_definition() {
        let mut mir = lower_text("fn main() -> i64 { return 1 + 2; }");
        let function = &mut mir.functions.entries_mut_for_test()[0];
        function.body.blocks[0].instructions.swap(0, 2);

        let errors = super::super::verify_mir(&mir).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("used before it is defined")));
    }

    #[test]
    fn verifier_rejects_a_value_defined_in_terms_of_itself() {
        let mut mir = lower_text("fn main() -> i64 { return 1; }");
        let function = &mut mir.functions.entries_mut_for_test()[0];
        let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0]
        else {
            panic!("expected assignment");
        };
        assignment.rvalue.kind = MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: assignment.result,
        };

        let errors = super::super::verify_mir(&mir).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("used before it is defined")));
    }

    #[test]
    fn verifier_rejects_call_signature_mismatches() {
        let mut mir = lower_text(concat!(
            "fn one(value: i64) -> i64 { return value; }\n",
            "fn main() -> i64 { return one(1); }\n",
        ));
        let main = &mut mir.functions.entries_mut_for_test()[1];
        let call = main.body.blocks[0]
            .instructions
            .iter_mut()
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(MirAssignment {
                    rvalue:
                        MirRvalue {
                            kind: MirRvalueKind::DirectCall { arguments, .. },
                            ..
                        },
                    ..
                }) => Some(arguments),
                _ => None,
            })
            .unwrap();
        call.clear();

        let errors = super::super::verify_mir(&mir).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("has 0 arguments but requires 1")));
    }

    #[test]
    fn verifier_rejects_ids_owned_by_another_function() {
        let mut mir = lower_text("fn main() -> i64 { return 0; }");
        let foreign = FunctionId::new(99);
        mir.functions.entries_mut_for_test()[0].values[0].id = ValueId::new(foreign, 0);

        let errors = super::super::verify_mir(&mir).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("owned by another function")));
    }
}
