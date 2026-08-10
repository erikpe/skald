//! Statement dispatch, statement-family rules, and structured block flow.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirBaseInitialization, HirBlock, HirBreak, HirCallArgument, HirCallStatement,
        HirConditional, HirConditionalArm, HirContinue, HirControlEffects, HirLocalDecl,
        HirLocalInitializer, HirObjectReturn, HirOptionalAssignment, HirOptionalPlace,
        HirOptionalStorage, HirOptionalWriteKind, HirPanic, HirPrimitiveAssignment,
        HirPrimitivePlace, HirPrimitiveStorage, HirReturn, HirReturnValue, HirSharedAssignment,
        HirStatement, HirWhile, Type,
    },
    resolve::{
        ResolvedBlock, ResolvedBreak, ResolvedConditional, ResolvedContinue,
        ResolvedExpressionStatement, ResolvedLocalDecl, ResolvedReturn, ResolvedStatement,
        ResolvedWhile,
    },
};

use super::{
    direct_call_through_groups, is_call_through_groups, lower_type, require_type, CallableChecker,
    MemberBodyKind, INVALID_CALL_STATEMENT, INVALID_INITIALIZER_BODY, INVALID_RETURN,
    READ_ONLY_RECEIVER,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_block(&mut self, block: &ResolvedBlock) -> HirBlock {
        let mut statements = Vec::with_capacity(block.statements.len());
        let mut effects = HirControlEffects::fallthrough();
        for statement in &block.statements {
            let checked = self.check_statement(statement);
            effects = effects.then(checked.effects);
            if let Some(statement) = checked.hir {
                statements.push(statement);
            }
        }

        HirBlock {
            statements,
            effects,
            span: block.span,
        }
    }

    fn check_statement(&mut self, statement: &ResolvedStatement) -> CheckedStatement {
        if self
            .member_body_kind
            .is_some_and(MemberBodyKind::initializes_receiver)
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
            ResolvedStatement::Break(statement) => self.check_break_statement(statement),
            ResolvedStatement::Continue(statement) => self.check_continue_statement(statement),
            ResolvedStatement::Expression(statement) => self.check_call_statement(statement),
            ResolvedStatement::Conditional(conditional) => {
                self.check_conditional_statement(conditional)
            }
            ResolvedStatement::While(statement) => self.check_while_statement(statement),
            ResolvedStatement::Block(block) => self.check_nested_block_statement(block),
            ResolvedStatement::PrimitiveBindingAssignment(assignment) => {
                self.check_primitive_binding_assignment(assignment)
            }
            ResolvedStatement::FieldAssignment(assignment) => {
                self.check_field_assignment(assignment)
            }
            ResolvedStatement::StaticFieldAssignment(assignment) => {
                self.check_static_field_assignment(assignment)
            }
            ResolvedStatement::ObjectAssignment(assignment) => {
                self.check_object_assignment(assignment)
            }
            ResolvedStatement::SharedAssignment(assignment) => {
                self.check_shared_assignment(assignment)
            }
            ResolvedStatement::OptionalAssignment(assignment) => {
                self.check_optional_assignment(assignment)
            }
            ResolvedStatement::ArrayAssignment(assignment) => {
                CheckedStatement::falls_through(self.check_array_assignment(assignment))
            }
        }
    }

    fn check_primitive_binding_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedPrimitiveBindingAssignment,
    ) -> CheckedStatement {
        let mutable = self
            .binding_access(assignment.destination, false, assignment.span)
            .is_some_and(|access| access == HirAccess::Mutable);
        if !mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    "cannot assign through a read-only primitive alias",
                )
                .with_primary_label(
                    assignment.span,
                    "primitive assignment requires mutable access",
                ),
            );
        }
        let expected = self.binding_type(assignment.destination);
        debug_assert!(matches!(
            expected,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
        ));
        let source = self
            .check_expression(&assignment.source)
            .filter(|source| {
                require_type(
                    source.ty,
                    expected,
                    source.span,
                    "primitive binding assignment",
                    self.diagnostics,
                )
            })
            .filter(|_| mutable)
            .map(|source| {
                HirStatement::PrimitiveAssignment(HirPrimitiveAssignment {
                    destination: HirPrimitivePlace {
                        storage: HirPrimitiveStorage::Binding(assignment.destination),
                        span: assignment.span,
                    },
                    source,
                    span: assignment.span,
                })
            });
        CheckedStatement::falls_through(source)
    }

    fn check_static_field_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedStaticFieldAssignment,
    ) -> CheckedStatement {
        let Some((place, ty)) = self.check_static_place(assignment.field, assignment.span) else {
            return CheckedStatement::falls_through(None);
        };
        let hir = match ty {
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool => self
                .check_expression(&assignment.value)
                .filter(|source| {
                    require_type(
                        source.ty,
                        ty,
                        source.span,
                        "static field assignment",
                        self.diagnostics,
                    )
                })
                .map(|source| {
                    HirStatement::PrimitiveAssignment(HirPrimitiveAssignment {
                        destination: HirPrimitivePlace {
                            storage: HirPrimitiveStorage::Static(place),
                            span: assignment.span,
                        },
                        source,
                        span: assignment.span,
                    })
                }),
            Type::OptionalPrimitive(payload) => self
                .check_optional_source(&assignment.value, payload, "optional static assignment")
                .map(|source| {
                    HirStatement::OptionalAssignment(HirOptionalAssignment {
                        destination: HirOptionalPlace {
                            storage: HirOptionalStorage::Static(place),
                            payload,
                            span: assignment.span,
                        },
                        payload,
                        source,
                        kind: HirOptionalWriteKind::Assign,
                        span: assignment.span,
                    })
                }),
            Type::OptionalClass(class) => self
                .check_class_optional_assignment(
                    crate::hir::HirClassOptionalPlace {
                        storage: HirOptionalStorage::Static(place),
                        class,
                        span: assignment.span,
                    },
                    &assignment.value,
                    "class optional static assignment",
                )
                .map(HirStatement::ClassOptionalAssignment),
            Type::OptionalShared(target) => self
                .check_optional_shared_assignment(
                    crate::hir::HirOptionalSharedPlace {
                        storage: HirOptionalStorage::Static(place),
                        target,
                        span: assignment.span,
                    },
                    &assignment.value,
                    "optional shared static assignment",
                )
                .map(HirStatement::OptionalSharedAssignment),
            Type::Array(array) => self
                .check_array_initialize(array, &assignment.value, "static array replacement")
                .map(|value| {
                    HirStatement::ArrayAssignment(crate::hir::HirArrayAssignment {
                        destination: crate::hir::HirArrayPlace::Static {
                            place,
                            array,
                            span: assignment.span,
                        },
                        value,
                        evaluation:
                            crate::hir::HirArrayEvaluationOrder::DestinationThenSourceThenReplace,
                        span: assignment.span,
                    })
                }),
            _ => unreachable!("enabled static storage type must have a statement family"),
        };
        CheckedStatement::falls_through(hir)
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

    fn check_optional_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedOptionalAssignment,
    ) -> CheckedStatement {
        let mutable = self
            .binding_access(assignment.destination, false, assignment.span)
            .is_some_and(|access| access == HirAccess::Mutable);
        if !mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    "cannot replace a read-only optional container",
                )
                .with_primary_label(
                    assignment.span,
                    "optional assignment requires mutable access",
                ),
            );
            return CheckedStatement::falls_through(None);
        }
        match self.binding_type(assignment.destination) {
            Type::OptionalPrimitive(payload) => {
                let source = self.check_optional_source(
                    &assignment.source,
                    payload,
                    "optional local assignment",
                );
                CheckedStatement::falls_through(source.map(|source| {
                    HirStatement::OptionalAssignment(HirOptionalAssignment {
                        destination: HirOptionalPlace {
                            storage: HirOptionalStorage::Binding(assignment.destination),
                            payload,
                            span: assignment.span,
                        },
                        payload,
                        source,
                        kind: HirOptionalWriteKind::Assign,
                        span: assignment.span,
                    })
                }))
            }
            Type::OptionalClass(class) => {
                let destination = crate::hir::HirClassOptionalPlace {
                    storage: HirOptionalStorage::Binding(assignment.destination),
                    class,
                    span: assignment.span,
                };
                let value = self.check_class_optional_assignment(
                    destination,
                    &assignment.source,
                    "class optional local assignment",
                );
                CheckedStatement::falls_through(value.map(HirStatement::ClassOptionalAssignment))
            }
            Type::OptionalShared(target) => {
                let destination = crate::hir::HirOptionalSharedPlace {
                    storage: HirOptionalStorage::Binding(assignment.destination),
                    target,
                    span: assignment.span,
                };
                let value = self.check_optional_shared_assignment(
                    destination,
                    &assignment.source,
                    "optional shared local assignment",
                );
                CheckedStatement::falls_through(value.map(HirStatement::OptionalSharedAssignment))
            }
            _ => unreachable!("resolved optional assignment must target optional storage"),
        }
    }

    fn check_base_initialization(
        &mut self,
        statement: &crate::resolve::ResolvedBaseInitialization,
    ) -> CheckedStatement {
        if self.member_body_kind
            != Some(crate::typeck::function::MemberBodyKind::OrdinaryInitializer)
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
        let expected = lower_type(self.program, &metadata.type_syntax);
        let initializer = match expected {
            Type::Class(class) => {
                self.check_object_local_initializer(local.local, class, &local.initializer)
            }
            Type::Array(array) => self
                .check_array_initialize(array, &local.initializer, "array local initializer")
                .map(HirLocalInitializer::Array),
            Type::Shared(target) => self
                .check_shared_transfer(&local.initializer, target, "shared local initializer")
                .map(HirLocalInitializer::Shared),
            Type::OptionalPrimitive(payload) => self
                .check_optional_source(
                    &local.initializer,
                    payload,
                    "primitive optional local initializer",
                )
                .map(HirLocalInitializer::Optional),
            Type::OptionalClass(class) => self
                .check_class_optional_initialize(
                    class,
                    &local.initializer,
                    "class optional local initializer",
                )
                .map(HirLocalInitializer::ClassOptional),
            Type::OptionalShared(target) => self
                .check_optional_shared_initialize(
                    target,
                    &local.initializer,
                    "optional shared local initializer",
                )
                .map(HirLocalInitializer::OptionalShared),
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
                    return CheckedStatement::exits_function(None);
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
                    return CheckedStatement::exits_function(None);
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
                        return CheckedStatement::exits_function(None);
                    };
                    let omitted_copy = match &construction.mode {
                        crate::hir::HirConstructionMode::Initialize { .. } => {
                            let Some(operation) =
                                self.copy_capabilities.constructor(class).selected()
                            else {
                                self.report_unavailable_copy_operation(class, true, value.span());
                                return CheckedStatement::exits_function(None);
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
                            return CheckedStatement::exits_function(None);
                        }
                    }
                    let Some(source) = self.check_object_source(value, class, "object return")
                    else {
                        return CheckedStatement::exits_function(None);
                    };
                    let Some(operation) = self.copy_capabilities.constructor(class).selected()
                    else {
                        self.report_unavailable_copy_operation(class, true, value.span());
                        return CheckedStatement::exits_function(None);
                    };
                    HirObjectReturn::Copy {
                        source: Box::new(source),
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
            (Type::Array(array), Some(value)) => self
                .check_array_initialize(array, value, "array return")
                .map(|value| {
                    HirStatement::Return(HirReturn {
                        value: Some(HirReturnValue::Array(value)),
                        span: statement.span,
                    })
                }),
            (Type::Array(_), None) => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_RETURN, "array return requires a value")
                        .with_primary_label(statement.span, "expected `return array_value;`"),
                );
                None
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
            (Type::OptionalPrimitive(payload), Some(value)) => self
                .check_optional_source(value, payload, "primitive optional return")
                .map(|value| {
                    HirStatement::Return(HirReturn {
                        value: Some(HirReturnValue::Optional(value)),
                        span: statement.span,
                    })
                }),
            (Type::OptionalPrimitive(payload), None) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_RETURN,
                        format!(
                            "{} must return a `{}` value",
                            self.callable_name,
                            Type::OptionalPrimitive(payload).name()
                        ),
                    )
                    .with_primary_label(statement.span, "expected `return optional_expression;`"),
                );
                None
            }
            (Type::OptionalClass(class), Some(value)) => self
                .check_class_optional_initialize(class, value, "class optional return")
                .map(|value| {
                    HirStatement::Return(HirReturn {
                        value: Some(HirReturnValue::ClassOptional(value)),
                        span: statement.span,
                    })
                }),
            (Type::OptionalClass(class), None) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_RETURN,
                        format!(
                            "{} must return a `{}` value",
                            self.callable_name,
                            Type::OptionalClass(class).name()
                        ),
                    )
                    .with_primary_label(statement.span, "expected an optional object value"),
                );
                None
            }
            (Type::OptionalShared(target), Some(value)) => self
                .check_optional_shared_initialize(target, value, "optional shared return")
                .map(|value| {
                    HirStatement::Return(HirReturn {
                        value: Some(HirReturnValue::OptionalShared(value)),
                        span: statement.span,
                    })
                }),
            (Type::OptionalShared(target), None) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_RETURN,
                        format!(
                            "{} must return a `{}` value",
                            self.callable_name,
                            Type::OptionalShared(target).name()
                        ),
                    )
                    .with_primary_label(statement.span, "expected an optional shared-owner value"),
                );
                None
            }
        };
        CheckedStatement::exits_function(hir)
    }

    fn check_call_statement(
        &mut self,
        statement: &ResolvedExpressionStatement,
    ) -> CheckedStatement {
        if let Some(call) = direct_call_through_groups(&statement.expression) {
            let target = self
                .program
                .declarations
                .get(call.function)
                .expect("resolved direct-call target must exist");
            if matches!(
                target.linkage,
                crate::resolve::ResolvedFunctionLinkage::Intrinsic {
                    intrinsic: crate::intrinsic::Intrinsic::Panic,
                }
            ) {
                let arguments = self.check_arguments(
                    &call.arguments,
                    &target.parameters,
                    call.callee_span,
                    "panic",
                    Some(&target.name),
                    Some(target.name_span),
                );
                let panic = arguments.and_then(|mut arguments| match arguments.pop() {
                    Some(HirCallArgument::Copy(message)) if arguments.is_empty() => {
                        Some(HirStatement::Panic(HirPanic {
                            message,
                            span: statement.span,
                        }))
                    }
                    Some(_) | None => None,
                });
                return CheckedStatement::diverges(panic);
            }
        }
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
        let mut effects = HirControlEffects::default();
        for arm in &conditional.arms {
            let condition = self.check_expression(&arm.condition);
            let body = self.check_block(&arm.body);
            effects = effects.union(body.effects.clone());
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
        if let Some(block) = &else_block {
            effects = effects.union(block.effects.clone());
        } else {
            effects = effects.union(HirControlEffects::fallthrough());
        }

        let hir = valid.then_some(HirStatement::Conditional(HirConditional {
            arms,
            else_block,
            effects: effects.clone(),
            span: conditional.span,
        }));
        CheckedStatement { hir, effects }
    }

    fn check_break_statement(&self, statement: &ResolvedBreak) -> CheckedStatement {
        CheckedStatement {
            hir: Some(HirStatement::Break(HirBreak {
                target: statement.target,
                span: statement.span,
            })),
            effects: HirControlEffects::break_to(statement.target),
        }
    }

    fn check_continue_statement(&self, statement: &ResolvedContinue) -> CheckedStatement {
        CheckedStatement {
            hir: Some(HirStatement::Continue(HirContinue {
                target: statement.target,
                span: statement.span,
            })),
            effects: HirControlEffects::continue_to(statement.target),
        }
    }

    fn check_while_statement(&mut self, statement: &ResolvedWhile) -> CheckedStatement {
        let condition = self.check_expression(&statement.condition);
        let body = self.check_block(&statement.body);
        let effects = body.effects.clone().through_loop(statement.loop_id);
        let hir = condition
            .filter(|condition| {
                require_type(
                    condition.ty,
                    Type::Bool,
                    condition.span,
                    "while condition",
                    self.diagnostics,
                )
            })
            .map(|condition| {
                HirStatement::While(HirWhile::new(
                    statement.loop_id,
                    condition,
                    body,
                    statement.span,
                ))
            });
        CheckedStatement { hir, effects }
    }

    fn check_nested_block_statement(&mut self, block: &ResolvedBlock) -> CheckedStatement {
        let block = self.check_block(block);
        let effects = block.effects.clone();
        CheckedStatement {
            hir: Some(HirStatement::Block(block)),
            effects,
        }
    }
}

pub(super) struct CheckedStatement {
    hir: Option<HirStatement>,
    effects: HirControlEffects,
}

impl CheckedStatement {
    pub(super) fn falls_through(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            effects: HirControlEffects::fallthrough(),
        }
    }

    fn exits_function(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            effects: HirControlEffects::function_exit(),
        }
    }

    fn diverges(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            effects: HirControlEffects::divergence(),
        }
    }
}

#[cfg(test)]
mod tests;
