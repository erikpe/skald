//! Per-callable context, statement checking, and structured control flow.

use std::collections::BTreeSet;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        BlockFlow, HirAccess, HirBlock, HirCallStatement, HirConditional, HirConditionalArm,
        HirConstruction, HirFieldAssignment, HirFunctionDefinition, HirLocal, HirLocalDecl,
        HirLocalInitializer, HirMemberDefinition, HirReturn, HirStatement, Type,
    },
    identity::{BindingId, CallableId, ClassId, FieldId},
    resolve::{
        ResolvedBlock, ResolvedConditional, ResolvedFunctionDeclaration,
        ResolvedFunctionDefinition, ResolvedLocal, ResolvedMemberDefinition, ResolvedParameter,
        ResolvedProgram, ResolvedStatement,
    },
};

use super::{
    expression::{is_call_through_groups, require_type},
    program::{
        lower_type, FIELD_INITIALIZATION, INVALID_CALL_STATEMENT, INVALID_CONSTRUCTION,
        INVALID_INITIALIZER_BODY, INVALID_OBJECT_CONTEXT, INVALID_RETURN, MISSING_RETURN,
        READ_ONLY_RECEIVER,
    },
};

#[derive(Clone, Copy)]
pub(super) struct ReceiverContext {
    pub(super) class: ClassId,
    pub(super) access: HirAccess,
    pub(super) initializer: bool,
}

pub(super) struct MemberCheckContext<'program> {
    pub(super) callable: CallableId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) definition: &'program ResolvedMemberDefinition,
    pub(super) return_type: Type,
    pub(super) receiver: ReceiverContext,
    pub(super) callable_name: String,
}

pub(super) struct CallableChecker<'program, 'diagnostics> {
    pub(super) program: &'program ResolvedProgram,
    pub(super) callable: CallableId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) locals: &'program [ResolvedLocal],
    body: &'program ResolvedBlock,
    definition_span: crate::source::Span,
    callable_name: String,
    pub(super) return_type: Type,
    pub(super) receiver: Option<ReceiverContext>,
    pub(super) initialized_fields: BTreeSet<FieldId>,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
}

impl<'program, 'diagnostics> CallableChecker<'program, 'diagnostics> {
    pub(super) fn new(
        program: &'program ResolvedProgram,
        declaration: &'program ResolvedFunctionDeclaration,
        definition: &'program ResolvedFunctionDefinition,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            callable: declaration.id.into(),
            parameters: &declaration.parameters,
            locals: &definition.locals,
            body: &definition.body,
            definition_span: definition.span,
            callable_name: format!("function `{}`", declaration.name),
            return_type: lower_type(&declaration.return_type),
            receiver: None,
            initialized_fields: BTreeSet::new(),
            diagnostics,
        }
    }

    pub(super) fn check(mut self) -> HirFunctionDefinition {
        let locals = self.lower_locals();
        let body = self.check_block(self.body);

        if self.return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(self.body.span, "a return value is required on every path")
                .with_note(format!(
                    "{} declares return type `{}`",
                    self.callable_name,
                    self.return_type.name()
                )),
            );
        }

        HirFunctionDefinition {
            function: self
                .callable
                .as_function()
                .expect("function checker needs function ID"),
            locals,
            body,
            span: self.definition_span,
        }
    }

    pub(super) fn new_member(
        program: &'program ResolvedProgram,
        context: MemberCheckContext<'program>,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            callable: context.callable,
            parameters: context.parameters,
            locals: &context.definition.locals,
            body: &context.definition.body,
            definition_span: context.definition.span,
            callable_name: context.callable_name,
            return_type: context.return_type,
            receiver: Some(context.receiver),
            initialized_fields: BTreeSet::new(),
            diagnostics,
        }
    }

    pub(super) fn check_member(mut self) -> HirMemberDefinition {
        let locals = self.lower_locals();
        let body = self.check_block(self.body);
        let receiver = self.receiver.expect("member checker needs receiver");
        if receiver.initializer {
            let class = self
                .program
                .class(receiver.class)
                .expect("member receiver must reference a class");
            for field in &class.fields {
                if !self.initialized_fields.contains(&field.id) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            FIELD_INITIALIZATION,
                            format!("field `{}` is not initialized", field.name),
                        )
                        .with_primary_label(
                            field.name_span,
                            "this field needs one assignment in `init`",
                        ),
                    );
                }
            }
        } else if self.return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(self.body.span, "a return value is required on every path")
                .with_note(format!(
                    "{} declares return type `{}`",
                    self.callable_name,
                    self.return_type.name()
                )),
            );
        }
        HirMemberDefinition {
            callable: self.callable,
            locals,
            body,
            span: self.definition_span,
        }
    }

    fn lower_locals(&self) -> Vec<HirLocal> {
        self.locals
            .iter()
            .map(|local| HirLocal {
                id: local.id,
                name: local.name.clone(),
                name_span: local.name_span,
                ty: lower_type(&local.type_syntax),
                span: local.span,
            })
            .collect()
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
        if self.receiver.is_some_and(|receiver| receiver.initializer)
            && !matches!(statement, ResolvedStatement::FieldAssignment(_))
        {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INITIALIZER_BODY,
                    "initializer bodies contain only direct field assignments",
                )
                .with_primary_label(
                    statement.span(),
                    "expected `self.field = primitive_expression;`",
                ),
            );
            return CheckedStatement::falls_through(None);
        }
        match statement {
            ResolvedStatement::Local(local) => {
                let metadata = self
                    .locals
                    .get(local.local.index())
                    .filter(|metadata| metadata.id == local.local)
                    .expect("resolved local declaration must reference local metadata");
                let expected = lower_type(&metadata.type_syntax);
                let initializer = match expected {
                    Type::Class(class) => self
                        .check_construction_initializer(class, &local.initializer)
                        .map(HirLocalInitializer::Construct),
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
                    (Type::Class(_), value) => {
                        if let Some(value) = value {
                            let _ = self.check_expression(value);
                        }
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_OBJECT_CONTEXT,
                                "object-valued returns are unavailable in the stage-0 profile",
                            )
                            .with_primary_label(
                                statement.span,
                                "expected a primitive or `unit` result",
                            ),
                        );
                        None
                    }
                };
                CheckedStatement::terminates(hir)
            }
            ResolvedStatement::Expression(statement) => {
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
            ResolvedStatement::Conditional(conditional) => self.check_conditional(conditional),
            ResolvedStatement::Block(block) => {
                let block = self.check_block(block);
                let flow = block.flow;
                CheckedStatement {
                    hir: Some(HirStatement::Block(block)),
                    flow,
                }
            }
            ResolvedStatement::FieldAssignment(assignment) => {
                self.check_field_assignment(assignment)
            }
        }
    }

    fn check_construction_initializer(
        &mut self,
        expected_class: ClassId,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<HirConstruction> {
        let crate::resolve::ResolvedExpression::Construct(construction) = expression else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "an object local must be initialized by direct construction",
                )
                .with_primary_label(
                    expression.span(),
                    "expected an ungrouped `Class(...)` expression",
                ),
            );
            return None;
        };
        if construction.class != expected_class {
            let actual_name = &self
                .program
                .class(construction.class)
                .expect("resolved constructor class must exist")
                .name;
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved local class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    "constructor type does not match the object local",
                )
                .with_primary_label(
                    construction.callee_span,
                    format!("constructs `{actual_name}`"),
                )
                .with_note(format!("the local requires `{expected_name}`")),
            );
            return None;
        }
        let initializer = self
            .program
            .initializer(construction.initializer)
            .expect("resolved construction must reference an initializer");
        let arguments = self.check_arguments(
            &construction.arguments,
            &initializer.parameters,
            construction.callee_span,
            "initializer",
            None,
            None,
        )?;
        Some(HirConstruction {
            class: construction.class,
            initializer: construction.initializer,
            arguments,
            span: construction.span,
        })
    }

    fn check_field_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> CheckedStatement {
        let place = self.check_field_place(assignment.receiver, assignment.field, assignment.span);
        let value = self.check_expression(&assignment.value);
        let Some(place) = place else {
            return CheckedStatement::falls_through(None);
        };
        let mut valid = true;
        if place.receiver.access == HirAccess::ReadOnly {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    "cannot assign through a read-only receiver",
                )
                .with_primary_label(
                    assignment.member_span,
                    "field assignment requires mutable receiver access",
                ),
            );
            valid = false;
        }
        if self.receiver.is_some_and(|receiver| receiver.initializer) {
            if place.receiver.binding != BindingId::Receiver(self.callable) {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INITIALIZER_BODY,
                        "an initializer can assign only its own fields",
                    )
                    .with_primary_label(assignment.span, "expected a field of `self`"),
                );
                valid = false;
            } else if !self.initialized_fields.insert(place.field) {
                let field = self
                    .program
                    .field(place.field)
                    .expect("selected field must exist");
                self.diagnostics.push(
                    Diagnostic::error(
                        FIELD_INITIALIZATION,
                        format!("field `{}` is initialized more than once", field.name),
                    )
                    .with_primary_label(assignment.member_span, "duplicate field initialization"),
                );
                valid = false;
            }
        }
        let Some(value) = value else {
            return CheckedStatement::falls_through(None);
        };
        let field = self
            .program
            .field(place.field)
            .expect("selected field must exist");
        valid &= require_type(
            value.ty,
            lower_type(&field.type_syntax),
            value.span,
            "field assignment",
            self.diagnostics,
        );
        CheckedStatement::falls_through(valid.then_some(HirStatement::FieldAssignment(
            HirFieldAssignment {
                place,
                value,
                span: assignment.span,
            },
        )))
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
