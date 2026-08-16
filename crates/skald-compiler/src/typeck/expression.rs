//! Expression, call, binding, and primitive-operation checking.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirExpression, HirExpressionKind, Type},
    resolve::{
        ResolvedExpression, ResolvedFunctionTypeParameter, ResolvedFunctionTypeParameterMode,
        ResolvedInterfaceParameter, ResolvedParameter, ResolvedParameterBindingMode, ResolvedType,
    },
    source::Span,
};

use super::{
    function::CallableChecker,
    program::{INVALID_OBJECT_CAST, INVALID_OBJECT_CONTEXT, TYPE_MISMATCH},
};

mod alias;
mod alias_access;
mod bit_reinterpretation;
mod bitwise;
mod call;
mod comparison;
mod division;
mod indirect_call;
mod io;
mod logical;
mod object_view_relation;
mod optional_box_view;
mod place;
mod primitive;
mod receiver;
mod shared_pointee;
mod shift;
mod static_field;
mod type_operations;

pub(in crate::typeck) use object_view_relation::{
    class_provides_view, classify_object_view_relation, ObjectViewRelation, ObjectViewSource,
};
pub(in crate::typeck) use place::{CheckedReceiverCarrier, ObjectPlaceUse};

pub(in crate::typeck) trait CallParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode;
    fn type_syntax(&self) -> &ResolvedType;
    fn span(&self) -> Span;
}

impl CallParameter for ResolvedParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode {
        self.binding_mode
    }
    fn type_syntax(&self) -> &ResolvedType {
        &self.type_syntax
    }
    fn span(&self) -> Span {
        self.span
    }
}

impl CallParameter for ResolvedInterfaceParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode {
        self.binding_mode
    }
    fn type_syntax(&self) -> &ResolvedType {
        &self.type_syntax
    }
    fn span(&self) -> Span {
        self.span
    }
}

impl CallParameter for ResolvedFunctionTypeParameter {
    fn binding_mode(&self) -> ResolvedParameterBindingMode {
        match self.mode {
            ResolvedFunctionTypeParameterMode::Value => ResolvedParameterBindingMode::Value,
            ResolvedFunctionTypeParameterMode::ReadOnlyAlias => {
                ResolvedParameterBindingMode::ReadOnlyAlias {
                    ref_span: self.span,
                }
            }
            ResolvedFunctionTypeParameterMode::MutableAlias => {
                ResolvedParameterBindingMode::MutableAlias {
                    mut_span: self.span,
                    ref_span: self.span,
                }
            }
        }
    }

    fn type_syntax(&self) -> &ResolvedType {
        &self.type_syntax
    }

    fn span(&self) -> Span {
        self.span
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_expression(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirExpression> {
        match expression {
            ResolvedExpression::Absent(absent) => {
                self.diagnostics.push(
                    Diagnostic::error(TYPE_MISMATCH, "`none` requires an expected optional type")
                        .with_primary_label(
                            absent.span,
                            "use `none` to initialize or assign a declared optional",
                        ),
                );
                None
            }
            ResolvedExpression::Present(present) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        "`some(expression)` requires an expected optional type",
                    )
                    .with_primary_label(
                        present.span,
                        "use `some` where a declared optional type supplies its payload type",
                    ),
                );
                None
            }
            ResolvedExpression::PresenceTest(test) => self.check_presence_test(test),
            ResolvedExpression::Unwrap(unwrap) => self.check_optional_unwrap(unwrap),
            ResolvedExpression::Binding(binding) => self.check_binding_expression(binding),
            ResolvedExpression::FunctionReference(reference) => Some(HirExpression {
                kind: HirExpressionKind::FunctionReference(crate::hir::HirFunctionReference {
                    target: reference.target,
                    function_type: reference.function_type,
                }),
                ty: Type::Function(reference.function_type),
                span: reference.span,
            }),
            ResolvedExpression::IndirectCall(call) => self.check_indirect_call(call),
            ResolvedExpression::NumericLiteral(literal) => self.check_numeric_literal(literal),
            ResolvedExpression::ByteLiteral(literal) => Some(HirExpression {
                kind: HirExpressionKind::U8(literal.value),
                ty: Type::U8,
                span: literal.span,
            }),
            ResolvedExpression::StringLiteral(literal) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "a string literal must be consumed as an object value",
                    )
                    .with_primary_label(
                        literal.span,
                        "store, pass, return, or otherwise consume this produced `Str`",
                    ),
                );
                None
            }
            ResolvedExpression::Boolean(boolean) => self.check_boolean_expression(boolean),
            ResolvedExpression::Unary(unary) => self.check_unary_expression(unary),
            ResolvedExpression::Dereference(dereference) => {
                if matches!(
                    dereference.target,
                    crate::resolve::ResolvedSharedTarget::OptionalBox(_)
                ) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "a boxed optional wrapper needs an explicit operation",
                        )
                        .with_primary_label(
                            dereference.operator_span,
                            "test presence, copy it, unwrap it, or pass it by read-only alias",
                        ),
                    );
                    return None;
                }
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "a dereferenced shared owner must be consumed as an object place",
                    )
                    .with_primary_label(
                        dereference.span,
                        "use this place for member access or a type test",
                    ),
                );
                None
            }
            ResolvedExpression::Binary(binary) => self.check_binary_expression(binary),
            ResolvedExpression::Logical(logical) => self.check_logical_expression(logical),
            ResolvedExpression::TypeTest(test) => self.check_type_test(test),
            ResolvedExpression::PrimitiveCast(cast) => self.check_primitive_cast(cast),
            ResolvedExpression::ObjectCast(cast) => {
                if self.check_object_cast(cast).is_some() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CAST,
                            "an object cast must be consumed as a non-owning place",
                        )
                        .with_primary_label(
                            cast.span,
                            "use this cast as a receiver, field place, or alias argument",
                        ),
                    );
                }
                None
            }
            ResolvedExpression::Allocation(allocation) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "shared allocation must be consumed as a shared owner",
                    )
                    .with_primary_label(
                        allocation.new_span,
                        "store, pass, return, or otherwise consume this produced owner",
                    ),
                );
                None
            }
            ResolvedExpression::OptionalBoxAllocation(allocation) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "optional-box allocation must be consumed as a shared owner",
                    )
                    .with_primary_label(
                        allocation.new_span,
                        "store, pass, return, or otherwise consume this produced box owner",
                    ),
                );
                None
            }
            ResolvedExpression::ArrayConstruction(construction) => {
                self.check_array_construction(construction)
            }
            ResolvedExpression::ArrayLength(length) => self.check_array_length(length),
            ResolvedExpression::DirectCall(call) => self.check_direct_call(call),
            ResolvedExpression::StaticCall(call) => self.check_static_call(call),
            ResolvedExpression::Grouped(grouped) => self.check_grouped_expression(grouped),
            ResolvedExpression::FieldAccess(access) => self.check_field_read(access),
            ResolvedExpression::StaticFieldAccess(access) => self.check_static_field_read(access),
            ResolvedExpression::ArrayProjection(projection) => {
                self.check_array_projection(projection)
            }
            ResolvedExpression::MethodCall(call) => self.check_method_call(call),
            ResolvedExpression::InterfaceCall(call) => self.check_interface_call(call),
            ResolvedExpression::Construct(construction) => {
                self.check_excluded_construction_expression(construction)
            }
        }
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
        ResolvedExpression::DirectCall(_)
        | ResolvedExpression::IndirectCall(_)
        | ResolvedExpression::StaticCall(_)
        | ResolvedExpression::MethodCall(_)
        | ResolvedExpression::InterfaceCall(_) => true,
        ResolvedExpression::Grouped(grouped) => is_call_through_groups(&grouped.expression),
        _ => false,
    }
}

pub(super) fn direct_call_through_groups(
    expression: &ResolvedExpression,
) -> Option<&crate::resolve::ResolvedDirectCallExpr> {
    match expression {
        ResolvedExpression::DirectCall(call) => Some(call),
        ResolvedExpression::Grouped(grouped) => direct_call_through_groups(&grouped.expression),
        _ => None,
    }
}
