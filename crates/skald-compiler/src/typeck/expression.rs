//! Expression, call, binding, and primitive-operation checking.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    hir::{
        HirAccess, HirBinaryOperation, HirCallArgument, HirExpression, HirExpressionKind,
        HirFieldPlace, HirObjectPlace, HirUnaryOperation, Type,
    },
    identity::{BindingId, FieldId},
    resolve::{
        ResolvedBinaryOperator, ResolvedExpression, ResolvedObjectPlace, ResolvedParameter,
        ResolvedParameterBindingMode, ResolvedUnaryOperator,
    },
    source::Span,
};

use super::{
    function::CallableChecker,
    literal::{classify_i64_magnitude, i64_literal_through_groups, Magnitude},
    program::{
        lower_parameter_mode, lower_type, FIELD_INITIALIZATION, INSUFFICIENT_ALIAS_ACCESS,
        INVALID_ALIAS_ARGUMENT, INVALID_CONSTRUCTION, INVALID_INITIALIZER_BODY,
        INVALID_OBJECT_CONTEXT, READ_ONLY_RECEIVER, TYPE_MISMATCH, WRONG_ARGUMENT_COUNT,
    },
};

const NUMERIC_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];
const NEGATABLE_TYPE_NAMES: &[&str] = &["i64", "f64"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_expression(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirExpression> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let ty = self.binding_type(binding.binding);
                if matches!(ty, Type::Class(_)) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "an inline object cannot be used as an ordinary value",
                        )
                        .with_primary_label(
                            binding.span,
                            "use the object as a field or method receiver",
                        ),
                    );
                    return None;
                }
                Some(HirExpression {
                    kind: HirExpressionKind::Binding(binding.binding),
                    ty,
                    span: binding.span,
                })
            }
            ResolvedExpression::NumericLiteral(literal) => self.check_numeric_literal(literal),
            ResolvedExpression::Boolean(boolean) => Some(HirExpression {
                kind: HirExpressionKind::Boolean(boolean.value),
                ty: Type::Bool,
                span: boolean.span,
            }),
            ResolvedExpression::Unary(unary) => {
                if unary.operator == ResolvedUnaryOperator::Negate {
                    if let Some(literal) = i64_literal_through_groups(&unary.operand) {
                        match classify_i64_magnitude(&literal.spelling) {
                            Magnitude::MinimumBoundary => {
                                return Some(HirExpression {
                                    kind: HirExpressionKind::I64(i64::MIN),
                                    ty: Type::I64,
                                    span: unary.span,
                                });
                            }
                            Magnitude::TooLarge => {
                                self.report_integer_out_of_range(
                                    unary.span,
                                    format!("-{}", literal.spelling),
                                );
                                return None;
                            }
                            Magnitude::PositiveI64 => {}
                        }
                    }
                }

                let operand = self.check_expression(&unary.operand)?;
                let operation = match operand.ty {
                    Type::I64 => HirUnaryOperation::NegateI64,
                    Type::F64 => HirUnaryOperation::NegateF64,
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                TYPE_MISMATCH,
                                format!(
                                    "unary negation requires an {} operand",
                                    format_type_list(NEGATABLE_TYPE_NAMES)
                                ),
                            )
                            .with_primary_label(
                                operand.span,
                                format!("operand has type `{}`", operand.ty.name()),
                            ),
                        );
                        return None;
                    }
                };
                let ty = operand.ty;
                Some(HirExpression {
                    kind: HirExpressionKind::Unary {
                        operation,
                        operand: Box::new(operand),
                    },
                    ty,
                    span: unary.span,
                })
            }
            ResolvedExpression::Binary(binary) => {
                let left = self.check_expression(&binary.left);
                let right = self.check_expression(&binary.right);
                let (left, right) = match (left, right) {
                    (Some(left), Some(right)) => (left, right),
                    _ => return None,
                };
                let operation = if left.ty == right.ty {
                    select_binary_operation(binary.operator, left.ty)
                } else {
                    None
                };
                let Some(operation) = operation else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            TYPE_MISMATCH,
                            "binary arithmetic requires operands of the same numeric type",
                        )
                        .with_primary_label(
                            binary.operator_span,
                            "operator cannot be applied to these operand types",
                        )
                        .with_secondary_label(
                            left.span,
                            format!("left operand has type `{}`", left.ty.name()),
                        )
                        .with_secondary_label(
                            right.span,
                            format!("right operand has type `{}`", right.ty.name()),
                        )
                        .with_note(format!(
                            "numeric operand types are {}",
                            format_type_list(NUMERIC_TYPE_NAMES)
                        )),
                    );
                    return None;
                };
                let ty = left.ty;

                Some(HirExpression {
                    kind: HirExpressionKind::Binary {
                        operation,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    span: binary.span,
                })
            }
            ResolvedExpression::DirectCall(call) => {
                let target = self
                    .program
                    .declarations
                    .get(call.function)
                    .expect("resolved direct-call target must exist");
                let arguments = self.check_arguments(
                    &call.arguments,
                    &target.parameters,
                    call.callee_span,
                    "function",
                    Some(&target.name),
                    Some(target.name_span),
                )?;
                Some(HirExpression {
                    kind: HirExpressionKind::DirectCall {
                        function: call.function,
                        arguments,
                    },
                    ty: lower_type(&target.return_type),
                    span: call.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => {
                let inner = self.check_expression(&grouped.expression)?;
                let ty = inner.ty;
                Some(HirExpression {
                    kind: HirExpressionKind::Grouped(Box::new(inner)),
                    ty,
                    span: grouped.span,
                })
            }
            ResolvedExpression::FieldAccess(access) => {
                let place = self.check_field_place(access.receiver, access.field, access.span)?;
                if self.receiver.is_some_and(|receiver| receiver.initializer)
                    && place.receiver.binding == BindingId::Receiver(self.callable)
                    && !self.initialized_fields.contains(&place.field)
                {
                    let field = self
                        .program
                        .field(place.field)
                        .expect("selected field must exist");
                    self.diagnostics.push(
                        Diagnostic::error(
                            FIELD_INITIALIZATION,
                            format!("field `{}` is read before initialization", field.name),
                        )
                        .with_primary_label(access.member_span, "field is not initialized yet"),
                    );
                    return None;
                }
                let field = self
                    .program
                    .field(place.field)
                    .expect("selected field must exist");
                Some(HirExpression {
                    kind: HirExpressionKind::FieldRead(place),
                    ty: lower_type(&field.type_syntax),
                    span: access.span,
                })
            }
            ResolvedExpression::MethodCall(call) => self.check_method_call(call),
            ResolvedExpression::Construct(construction) => {
                for argument in &construction.arguments {
                    let _ = self.check_expression(argument);
                }
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CONSTRUCTION,
                        "construction is allowed only as an object local initializer",
                    )
                    .with_primary_label(construction.span, "use `var name: Class = Class(...);`"),
                );
                None
            }
        }
    }

    fn check_method_call(
        &mut self,
        call: &crate::resolve::ResolvedMethodCallExpr,
    ) -> Option<HirExpression> {
        let receiver = self.check_object_place(call.receiver)?;
        let method = self
            .program
            .method(call.method)
            .expect("resolved method call must reference a method");
        let mut valid = true;
        if self.receiver.is_some_and(|context| context.initializer) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INITIALIZER_BODY,
                    "an initializer cannot call instance methods",
                )
                .with_primary_label(call.member_span, "the complete receiver is not live yet"),
            );
            valid = false;
        }
        if method.receiver_access == crate::resolve::ResolvedReceiverAccess::Mutable
            && receiver.access == HirAccess::ReadOnly
        {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    format!(
                        "mutable method `{}` requires mutable receiver access",
                        method.name
                    ),
                )
                .with_primary_label(call.member_span, "called through a read-only receiver")
                .with_secondary_label(method.name_span, "mutable method declared here"),
            );
            valid = false;
        }
        let arguments = self.check_arguments(
            &call.arguments,
            &method.parameters,
            call.member_span,
            "method",
            Some(&method.name),
            Some(method.name_span),
        )?;
        valid.then_some(HirExpression {
            kind: HirExpressionKind::MethodCall {
                receiver,
                method: call.method,
                arguments,
            },
            ty: lower_type(&method.return_type),
            span: call.span,
        })
    }

    pub(super) fn check_arguments(
        &mut self,
        source: &[ResolvedExpression],
        parameters: &[ResolvedParameter],
        target_span: Span,
        target_kind: &'static str,
        target_name: Option<&str>,
        declaration_span: Option<Span>,
    ) -> Option<Vec<HirCallArgument>> {
        let mut arguments = Vec::with_capacity(source.len());
        let mut valid = true;
        for (index, argument) in source.iter().enumerate() {
            match parameters.get(index) {
                Some(parameter) => match self.check_argument(argument, parameter) {
                    Some(argument) => arguments.push(argument),
                    None => valid = false,
                },
                None => {
                    let _ = self.check_expression(argument);
                    valid = false;
                }
            }
        }
        if source.len() != parameters.len() {
            let target = target_name
                .map(|name| format!("{target_kind} `{name}`"))
                .unwrap_or_else(|| target_kind.to_owned());
            let mut diagnostic = Diagnostic::error(
                WRONG_ARGUMENT_COUNT,
                format!(
                    "{target} expects {} argument{} but received {}",
                    parameters.len(),
                    if parameters.len() == 1 { "" } else { "s" },
                    source.len()
                ),
            )
            .with_primary_label(target_span, "called with the wrong number of arguments");
            if let Some(declaration_span) = declaration_span {
                diagnostic = diagnostic
                    .with_secondary_label(declaration_span, format!("{target_kind} declared here"));
            }
            self.diagnostics.push(diagnostic);
            valid = false;
        }
        valid.then_some(arguments)
    }

    fn check_argument(
        &mut self,
        source: &ResolvedExpression,
        parameter: &ResolvedParameter,
    ) -> Option<HirCallArgument> {
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value => {
                let argument = self.check_expression(source)?;
                require_type(
                    argument.ty,
                    lower_type(&parameter.type_syntax),
                    argument.span,
                    "call argument",
                    self.diagnostics,
                )
                .then_some(HirCallArgument::Value(argument))
            }
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. } => {
                let place = self.check_alias_argument_place(source)?;
                let Type::Class(expected_class) = lower_type(&parameter.type_syntax) else {
                    return None;
                };
                if place.class != expected_class {
                    let actual = &self
                        .program
                        .class(place.class)
                        .expect("resolved alias argument class must exist")
                        .name;
                    let expected = &self
                        .program
                        .class(expected_class)
                        .expect("resolved alias parameter class must exist")
                        .name;
                    self.diagnostics.push(
                        Diagnostic::error(
                            TYPE_MISMATCH,
                            format!("alias argument has type `{actual}`, expected `{expected}`"),
                        )
                        .with_primary_label(place.span, "this place has the wrong class")
                        .with_secondary_label(
                            parameter.type_syntax.span,
                            "alias parameter type declared here",
                        ),
                    );
                    return None;
                }
                let required = lower_parameter_mode(parameter.binding_mode)
                    .required_access()
                    .expect("alias parameter mode must require place access");
                if !place.access.permits(required) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INSUFFICIENT_ALIAS_ACCESS,
                            "read-only access cannot satisfy a mutable alias parameter",
                        )
                        .with_primary_label(place.span, "this place provides read-only access")
                        .with_secondary_label(parameter.span, "mutable alias declared here"),
                    );
                    return None;
                }
                Some(HirCallArgument::Place(place))
            }
        }
    }

    pub(super) fn check_field_place(
        &mut self,
        place: ResolvedObjectPlace,
        field: FieldId,
        span: Span,
    ) -> Option<HirFieldPlace> {
        Some(HirFieldPlace {
            receiver: self.check_object_place(place)?,
            field,
            span,
        })
    }

    fn check_object_place(&mut self, place: ResolvedObjectPlace) -> Option<HirObjectPlace> {
        let checked = self.check_binding_place(place.binding, place.span, true)?;
        assert_eq!(
            checked.class, place.class,
            "resolved object-place class must match its binding"
        );
        Some(checked)
    }

    fn check_alias_argument_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirObjectPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                self.check_binding_place(binding.binding, binding.span, false)
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut place = self.check_alias_argument_place(&grouped.expression)?;
                place.span = grouped.span;
                Some(place)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_ALIAS_ARGUMENT,
                        "alias argument must be an existing object place",
                    )
                    .with_primary_label(
                        expression.span(),
                        "expected an object local, `self`, alias parameter, or grouping",
                    ),
                );
                None
            }
        }
    }

    fn check_binding_place(
        &mut self,
        binding: BindingId,
        span: Span,
        allow_initializing_self: bool,
    ) -> Option<HirObjectPlace> {
        let Type::Class(class) = self.binding_type(binding) else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "alias argument must designate an object",
                )
                .with_primary_label(span, "this binding has a primitive type"),
            );
            return None;
        };
        let access = match binding {
            BindingId::Receiver(_) => {
                let receiver = self
                    .receiver
                    .expect("resolved receiver place must occur in a member");
                if receiver.initializer && !allow_initializing_self {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_ALIAS_ARGUMENT,
                            "initializer `self` is not a live alias source",
                        )
                        .with_primary_label(span, "the object becomes live after `init` returns"),
                    );
                    return None;
                }
                receiver.access
            }
            BindingId::Local(_) => HirAccess::Mutable,
            BindingId::Parameter(id) => {
                let parameter = self.parameter(id);
                let Some(access) = lower_parameter_mode(parameter.binding_mode).required_access()
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "an object value parameter cannot be used as an alias place",
                        )
                        .with_primary_label(span, "use an explicit alias parameter"),
                    );
                    return None;
                };
                access
            }
        };
        Some(HirObjectPlace {
            binding,
            class,
            access,
            span,
        })
    }

    fn parameter(&self, id: crate::identity::ParameterId) -> &ResolvedParameter {
        self.parameters
            .get(id.index())
            .filter(|parameter| parameter.id == id)
            .expect("resolved parameter ID must exist")
    }

    fn binding_type(&self, binding: BindingId) -> Type {
        assert_eq!(
            binding.callable(),
            self.callable,
            "resolved binding must belong to the current callable"
        );
        match binding {
            BindingId::Receiver(_) => Type::Class(
                self.receiver
                    .expect("receiver binding must be checked in a member body")
                    .class,
            ),
            BindingId::Parameter(id) => lower_type(&self.parameter(id).type_syntax),
            BindingId::Local(id) => lower_type(
                &self
                    .locals
                    .get(id.index())
                    .filter(|local| local.id == id)
                    .expect("resolved local ID must exist")
                    .type_syntax,
            ),
        }
    }
}

fn select_binary_operation(
    operator: ResolvedBinaryOperator,
    operand_type: Type,
) -> Option<HirBinaryOperation> {
    match (operator, operand_type) {
        (ResolvedBinaryOperator::Add, Type::I64) => Some(HirBinaryOperation::AddI64),
        (ResolvedBinaryOperator::Subtract, Type::I64) => Some(HirBinaryOperation::SubtractI64),
        (ResolvedBinaryOperator::Multiply, Type::I64) => Some(HirBinaryOperation::MultiplyI64),
        (ResolvedBinaryOperator::Add, Type::U64) => Some(HirBinaryOperation::AddU64),
        (ResolvedBinaryOperator::Subtract, Type::U64) => Some(HirBinaryOperation::SubtractU64),
        (ResolvedBinaryOperator::Multiply, Type::U64) => Some(HirBinaryOperation::MultiplyU64),
        (ResolvedBinaryOperator::Add, Type::U8) => Some(HirBinaryOperation::AddU8),
        (ResolvedBinaryOperator::Subtract, Type::U8) => Some(HirBinaryOperation::SubtractU8),
        (ResolvedBinaryOperator::Multiply, Type::U8) => Some(HirBinaryOperation::MultiplyU8),
        (ResolvedBinaryOperator::Add, Type::F64) => Some(HirBinaryOperation::AddF64),
        (ResolvedBinaryOperator::Subtract, Type::F64) => Some(HirBinaryOperation::SubtractF64),
        (ResolvedBinaryOperator::Multiply, Type::F64) => Some(HirBinaryOperation::MultiplyF64),
        (_, Type::Bool | Type::Unit | Type::Class(_)) => None,
    }
}

pub(super) fn require_type(
    actual: Type,
    expected: Type,
    span: Span,
    context: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if actual == expected {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(
            TYPE_MISMATCH,
            format!(
                "{context} has type `{}` but `{}` is required",
                actual.name(),
                expected.name()
            ),
        )
        .with_primary_label(span, "type mismatch"),
    );
    false
}

pub(super) fn is_call_through_groups(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::DirectCall(_) | ResolvedExpression::MethodCall(_) => true,
        ResolvedExpression::Grouped(grouped) => is_call_through_groups(&grouped.expression),
        _ => false,
    }
}
