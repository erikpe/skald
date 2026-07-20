//! Per-function context, statement checking, and structured control flow.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        BlockFlow, HirBlock, HirCallStatement, HirConditional, HirConditionalArm,
        HirFunctionDefinition, HirLocal, HirLocalDecl, HirReturn, HirStatement, Type,
    },
    resolve::{
        ResolvedBlock, ResolvedConditional, ResolvedFunctionDeclaration,
        ResolvedFunctionDefinition, ResolvedProgram, ResolvedStatement,
    },
};

use super::{
    expression::{is_direct_call_through_groups, require_type},
    program::{lower_type, INVALID_CALL_STATEMENT, INVALID_RETURN, MISSING_RETURN},
};

pub(super) struct FunctionChecker<'program, 'diagnostics> {
    pub(super) program: &'program ResolvedProgram,
    pub(super) declaration: &'program ResolvedFunctionDeclaration,
    pub(super) definition: &'program ResolvedFunctionDefinition,
    return_type: Type,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
}

impl<'program, 'diagnostics> FunctionChecker<'program, 'diagnostics> {
    pub(super) fn new(
        program: &'program ResolvedProgram,
        declaration: &'program ResolvedFunctionDeclaration,
        definition: &'program ResolvedFunctionDefinition,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            declaration,
            definition,
            return_type: lower_type(&declaration.return_type),
            diagnostics,
        }
    }

    pub(super) fn check(mut self) -> HirFunctionDefinition {
        let locals = self
            .definition
            .locals
            .iter()
            .map(|local| HirLocal {
                id: local.id,
                name: local.name.clone(),
                name_span: local.name_span,
                ty: lower_type(&local.type_syntax),
                span: local.span,
            })
            .collect();
        let body = self.check_block(&self.definition.body);

        if self.return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!(
                        "function `{}` does not return a value",
                        self.declaration.name
                    ),
                )
                .with_primary_label(
                    self.definition.body.span,
                    "a return value is required on every path",
                )
                .with_note(format!(
                    "function `{}` declares return type `{}`",
                    self.declaration.name,
                    self.return_type.name()
                )),
            );
        }

        HirFunctionDefinition {
            function: self.definition.function,
            locals,
            body,
            span: self.definition.span,
        }
    }

    fn check_block(&mut self, block: &ResolvedBlock) -> HirBlock {
        let mut statements = Vec::with_capacity(block.statements.len());
        let mut flow = BlockFlow::FallsThrough;
        for statement in &block.statements {
            let checked = self.check_statement(statement);
            flow = flow.then(checked.flow);
            if let Some(statement) = checked.hir {
                statements.push(statement);
            }
        }

        HirBlock {
            statements,
            flow,
            span: block.span,
        }
    }

    fn check_statement(&mut self, statement: &ResolvedStatement) -> CheckedStatement {
        match statement {
            ResolvedStatement::Local(local) => {
                let metadata = self
                    .definition
                    .local(local.local)
                    .expect("resolved local declaration must reference local metadata");
                let expected = lower_type(&metadata.type_syntax);
                let Some(initializer) = self.check_expression(&local.initializer) else {
                    return CheckedStatement::falls_through(None);
                };
                let hir = require_type(
                    initializer.ty,
                    expected,
                    initializer.span,
                    "local initializer",
                    self.diagnostics,
                )
                .then_some(HirStatement::Local(HirLocalDecl {
                    local: local.local,
                    initializer,
                    span: local.span,
                }));
                CheckedStatement::falls_through(hir)
            }
            ResolvedStatement::Return(statement) => {
                let hir = match (self.return_type, &statement.value) {
                    (Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool, Some(value)) => {
                        let Some(value) = self.check_expression(value) else {
                            return CheckedStatement::terminates(None);
                        };
                        require_type(
                            value.ty,
                            self.return_type,
                            value.span,
                            "return value",
                            self.diagnostics,
                        )
                        .then_some(HirStatement::Return(HirReturn {
                            value: Some(value),
                            span: statement.span,
                        }))
                    }
                    (Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool, None) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_RETURN,
                                format!(
                                    "{} `{}` function must return a value",
                                    self.return_type.indefinite_article(),
                                    self.return_type.name()
                                ),
                            )
                            .with_primary_label(statement.span, "expected `return expression;`"),
                        );
                        None
                    }
                    (Type::Unit, Some(value)) => {
                        // Preserve independent expression diagnostics even when
                        // the return form itself is invalid.
                        let _ = self.check_expression(value);
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_RETURN,
                                "a `unit` function cannot return a value",
                            )
                            .with_primary_label(statement.span, "use `return;` instead"),
                        );
                        None
                    }
                    (Type::Unit, None) => Some(HirStatement::Return(HirReturn {
                        value: None,
                        span: statement.span,
                    })),
                };
                CheckedStatement::terminates(hir)
            }
            ResolvedStatement::Expression(statement) => {
                let Some(expression) = self.check_expression(&statement.expression) else {
                    return CheckedStatement::falls_through(None);
                };
                if !is_direct_call_through_groups(&statement.expression) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CALL_STATEMENT,
                            "only function calls can be used as expression statements",
                        )
                        .with_primary_label(statement.span, "this expression is not a call"),
                    );
                    return CheckedStatement::falls_through(None);
                }
                if expression.ty != Type::Unit {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CALL_STATEMENT,
                            "a call statement must call a function returning `unit`",
                        )
                        .with_primary_label(
                            statement.span,
                            format!("this call returns `{}`", expression.ty.name()),
                        )
                        .with_note("use the returned value instead of discarding it"),
                    );
                    return CheckedStatement::falls_through(None);
                }
                CheckedStatement::falls_through(Some(HirStatement::Call(HirCallStatement {
                    call: expression,
                    span: statement.span,
                })))
            }
            ResolvedStatement::Conditional(conditional) => self.check_conditional(conditional),
            ResolvedStatement::Block(block) => {
                let block = self.check_block(block);
                let flow = block.flow;
                CheckedStatement {
                    hir: Some(HirStatement::Block(block)),
                    flow,
                }
            }
            ResolvedStatement::FieldAssignment(_) => {
                unreachable!("object programs stop at the pre-OBJ7 type-check boundary")
            }
        }
    }

    fn check_conditional(&mut self, conditional: &ResolvedConditional) -> CheckedStatement {
        let mut arms = Vec::with_capacity(conditional.arms.len());
        let mut valid = true;
        let mut all_arms_terminate = true;
        for arm in &conditional.arms {
            let condition = self.check_expression(&arm.condition);
            let body = self.check_block(&arm.body);
            all_arms_terminate &= body.flow == BlockFlow::Terminates;
            match condition {
                Some(condition)
                    if require_type(
                        condition.ty,
                        Type::Bool,
                        condition.span,
                        "conditional condition",
                        self.diagnostics,
                    ) =>
                {
                    arms.push(HirConditionalArm {
                        condition,
                        body,
                        span: arm.span,
                    })
                }
                _ => valid = false,
            }
        }
        let else_block = conditional
            .else_block
            .as_ref()
            .map(|block| self.check_block(block));
        let flow = if all_arms_terminate
            && else_block
                .as_ref()
                .is_some_and(|block| block.flow == BlockFlow::Terminates)
        {
            BlockFlow::Terminates
        } else {
            BlockFlow::FallsThrough
        };

        let hir = valid.then_some(HirStatement::Conditional(HirConditional {
            arms,
            else_block,
            flow,
            span: conditional.span,
        }));
        CheckedStatement { hir, flow }
    }
}

struct CheckedStatement {
    hir: Option<HirStatement>,
    flow: BlockFlow,
}

impl CheckedStatement {
    const fn falls_through(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            flow: BlockFlow::FallsThrough,
        }
    }

    const fn terminates(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            flow: BlockFlow::Terminates,
        }
    }
}
