//! Statement dispatch, statement-family rules, and structured block flow.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        BlockFlow, HirBaseInitialization, HirBlock, HirCallStatement, HirConditional,
        HirConditionalArm, HirLocalDecl, HirLocalInitializer, HirObjectReturn, HirReturn,
        HirReturnValue, HirSharedAssignment, HirStatement, Type,
    },
    resolve::{
        ResolvedBlock, ResolvedConditional, ResolvedExpressionStatement, ResolvedLocalDecl,
        ResolvedReturn, ResolvedStatement,
    },
};

use super::{
    is_call_through_groups, lower_type, require_type, CallableChecker, INVALID_CALL_STATEMENT,
    INVALID_INITIALIZER_BODY, INVALID_RETURN,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_block(&mut self, block: &ResolvedBlock) -> HirBlock {
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
        if self
            .receiver
            .is_some_and(|receiver| receiver.body_kind.initializes_receiver())
            && !matches!(
                statement,
                ResolvedStatement::BaseInitialization(_) | ResolvedStatement::FieldAssignment(_)
            )
        {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INITIALIZER_BODY,
                    "initializer bodies contain only direct field assignments",
                )
                .with_primary_label(
                    statement.span(),
                    "expected direct initialization of a field of `self`",
                ),
            );
            return CheckedStatement::falls_through(None);
        }

        match statement {
            ResolvedStatement::BaseInitialization(statement) => {
                self.check_base_initialization(statement)
            }
            ResolvedStatement::Local(local) => self.check_local_statement(local),
            ResolvedStatement::Return(statement) => self.check_return_statement(statement),
            ResolvedStatement::Expression(statement) => self.check_call_statement(statement),
            ResolvedStatement::Conditional(conditional) => {
                self.check_conditional_statement(conditional)
            }
            ResolvedStatement::Block(block) => self.check_nested_block_statement(block),
            ResolvedStatement::FieldAssignment(assignment) => {
                self.check_field_assignment(assignment)
            }
            ResolvedStatement::ObjectAssignment(assignment) => {
                self.check_object_assignment(assignment)
            }
            ResolvedStatement::SharedAssignment(assignment) => {
                self.check_shared_assignment(assignment)
            }
        }
    }

    fn check_shared_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedSharedAssignment,
    ) -> CheckedStatement {
        let target = crate::typeck::shared::lower_shared_target(assignment.target);
        let value =
            self.check_shared_transfer(&assignment.source, target, "shared local assignment");
        CheckedStatement::falls_through(value.map(|value| {
            HirStatement::SharedAssignment(HirSharedAssignment {
                destination: assignment.destination,
                value,
                span: assignment.span,
            })
        }))
    }

    fn check_base_initialization(
        &mut self,
        statement: &crate::resolve::ResolvedBaseInitialization,
    ) -> CheckedStatement {
        let receiver = self
            .receiver
            .expect("resolved base initialization must occur in a member");
        if receiver.body_kind != crate::typeck::function::MemberBodyKind::OrdinaryInitializer
            || self.base_initialized
        {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INITIALIZER_BODY,
                    "base initialization must occur exactly once before derived fields",
                )
                .with_primary_label(statement.span, "invalid base-initialization position"),
            );
            return CheckedStatement::falls_through(None);
        }
        let Some(initializer_id) = self.select_base_initializer(statement) else {
            return CheckedStatement::falls_through(None);
        };
        let initializer = self
            .program
            .initializer(initializer_id)
            .expect("selected base initialization must reference an initializer");
        let arguments = self.check_arguments(
            &statement.arguments,
            &initializer.parameters,
            statement.super_span,
            "base initializer",
            None,
            Some(initializer.span),
        );
        let Some(arguments) = arguments else {
            return CheckedStatement::falls_through(None);
        };
        self.base_initialized = true;
        CheckedStatement::falls_through(Some(HirStatement::BaseInitialization(
            HirBaseInitialization {
                base: statement.base,
                initializer: initializer_id,
                arguments,
                span: statement.span,
            },
        )))
    }

    fn check_local_statement(&mut self, local: &ResolvedLocalDecl) -> CheckedStatement {
        let metadata = self
            .locals
            .get(local.local.index())
            .filter(|metadata| metadata.id == local.local)
            .expect("resolved local declaration must reference local metadata");
        let expected = lower_type(&metadata.type_syntax);
        let initializer = match expected {
            Type::Class(class) => {
                self.check_object_local_initializer(local.local, class, &local.initializer)
            }
            Type::Shared(target) => self
                .check_shared_transfer(&local.initializer, target, "shared local initializer")
                .map(HirLocalInitializer::Shared),
            _ => self
                .check_expression(&local.initializer)
                .and_then(|initializer| {
                    require_type(
                        initializer.ty,
                        expected,
                        initializer.span,
                        "local initializer",
                        self.diagnostics,
                    )
                    .then_some(HirLocalInitializer::Value(initializer))
                }),
        };
        let hir = initializer.map(|initializer| {
            HirStatement::Local(HirLocalDecl {
                local: local.local,
                initializer,
                span: local.span,
            })
        });
        CheckedStatement::falls_through(hir)
    }

    fn check_return_statement(&mut self, statement: &ResolvedReturn) -> CheckedStatement {
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
                    value: Some(HirReturnValue::Scalar(value)),
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
            (Type::Unit | Type::Obj | Type::Interface(_), Some(value)) => {
                // Preserve independent expression diagnostics even when the
                // return form itself is invalid.
                let _ = self.check_expression(value);
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_RETURN,
                        format!("{} cannot return a value", self.callable_name),
                    )
                    .with_primary_label(statement.span, "use `return;` instead"),
                );
                None
            }
            (Type::Unit | Type::Obj | Type::Interface(_), None) => {
                Some(HirStatement::Return(HirReturn {
                    value: None,
                    span: statement.span,
                }))
            }
            (Type::Class(class), value) => {
                let Some(value) = value else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_RETURN,
                            format!("{} must return an object", self.callable_name),
                        )
                        .with_primary_label(statement.span, "expected `return object_place;`"),
                    );
                    return CheckedStatement::terminates(None);
                };
                let object_return = if matches!(
                    value,
                    crate::resolve::ResolvedExpression::Construct(construction)
                        if construction.class == class
                ) {
                    let crate::resolve::ResolvedExpression::Construct(construction) = value else {
                        unreachable!("matching construction must remain a construction")
                    };
                    let Some(construction) =
                        self.check_object_construction(class, construction, "return destination")
                    else {
                        return CheckedStatement::terminates(None);
                    };
                    let omitted_copy = match &construction.mode {
                        crate::hir::HirConstructionMode::Initialize { .. } => {
                            let Some(operation) =
                                self.copy_capabilities.constructor(class).selected()
                            else {
                                self.report_unavailable_copy_operation(class, true, value.span());
                                return CheckedStatement::terminates(None);
                            };
                            Some(operation)
                        }
                        crate::hir::HirConstructionMode::Copy { .. } => None,
                    };
                    HirObjectReturn::Construct {
                        construction,
                        omitted_copy,
                    }
                } else {
                    if let crate::resolve::ResolvedExpression::Construct(construction) = value {
                        if self.program.hierarchy.is_subtype(construction.class, class)
                            != Some(true)
                        {
                            let _ = self.check_object_construction(
                                class,
                                construction,
                                "return destination",
                            );
                            return CheckedStatement::terminates(None);
                        }
                    }
                    let Some(source) = self.check_object_source(value, class, "object return")
                    else {
                        return CheckedStatement::terminates(None);
                    };
                    let Some(operation) = self.copy_capabilities.constructor(class).selected()
                    else {
                        self.report_unavailable_copy_operation(class, true, value.span());
                        return CheckedStatement::terminates(None);
                    };
                    HirObjectReturn::Copy {
                        source,
                        operation,
                        class,
                        span: value.span(),
                    }
                };
                Some(HirStatement::Return(HirReturn {
                    value: Some(HirReturnValue::Object(object_return)),
                    span: statement.span,
                }))
            }
            (Type::Shared(target), Some(value)) => self
                .check_shared_transfer(value, target, "shared return")
                .map(|value| {
                    HirStatement::Return(HirReturn {
                        value: Some(HirReturnValue::Shared(value)),
                        span: statement.span,
                    })
                }),
            (Type::Shared(target), None) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_RETURN,
                        format!(
                            "{} must return a `{}` owner",
                            self.callable_name,
                            Type::Shared(target).name()
                        ),
                    )
                    .with_primary_label(statement.span, "expected `return shared_expression;`"),
                );
                None
            }
        };
        CheckedStatement::terminates(hir)
    }

    fn check_call_statement(
        &mut self,
        statement: &ResolvedExpressionStatement,
    ) -> CheckedStatement {
        let Some(expression) = self.check_expression(&statement.expression) else {
            return CheckedStatement::falls_through(None);
        };
        if !is_call_through_groups(&statement.expression) {
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

    fn check_conditional_statement(
        &mut self,
        conditional: &ResolvedConditional,
    ) -> CheckedStatement {
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
                    });
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

    fn check_nested_block_statement(&mut self, block: &ResolvedBlock) -> CheckedStatement {
        let block = self.check_block(block);
        let flow = block.flow;
        CheckedStatement {
            hir: Some(HirStatement::Block(block)),
            flow,
        }
    }
}

pub(super) struct CheckedStatement {
    hir: Option<HirStatement>,
    flow: BlockFlow,
}

impl CheckedStatement {
    pub(super) const fn falls_through(hir: Option<HirStatement>) -> Self {
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

#[cfg(test)]
mod tests;
