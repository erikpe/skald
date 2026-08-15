//! Static ordinary-initializer overload analysis and selection.

use super::*;
use crate::{
    hir::{HirAccess, Type},
    literal::NumericLiteralKind,
    resolve::{
        ResolvedExpression, ResolvedInitializerDeclaration, ResolvedObjectReceiver,
        ResolvedParameter, ResolvedParameterBindingMode, ResolvedTypeKind,
    },
    source::Span,
    typeck::{
        expression::class_provides_view,
        program::{
            lower_type, AMBIGUOUS_INITIALIZER, NO_MATCHING_INITIALIZER, PRIVATE_INITIALIZER_ACCESS,
        },
    },
};

#[derive(Clone, Copy)]
struct ArgumentAnalysis {
    ty: Type,
    absent: bool,
    contextual_optional: Option<ContextualOptionalArgument>,
    object: Option<ObjectArgument>,
    optional_place_access: Option<HirAccess>,
}

#[derive(Clone, Copy)]
struct ContextualOptionalArgument {
    present_layers: usize,
    terminal: ContextualOptionalTerminal,
}

#[derive(Clone, Copy)]
enum ContextualOptionalTerminal {
    Absent,
    Typed(Type),
}

#[derive(Clone, Copy)]
struct ObjectArgument {
    access: HirAccess,
    source: ObjectArgumentSource,
}

#[derive(Clone, Copy)]
enum ObjectArgumentSource {
    ExistingPlace,
    CheckedPlace,
    Produced,
}

#[derive(Clone, Copy)]
enum InitializerCallSite {
    DirectConstruction,
    BaseInitialization,
    Allocation,
}

impl ObjectArgumentSource {
    const fn can_bind_alias(self, required: HirAccess) -> bool {
        matches!(self, Self::ExistingPlace | Self::CheckedPlace)
            || matches!((self, required), (Self::Produced, HirAccess::ReadOnly))
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn select_construction_initializer(
        &mut self,
        construction: &crate::resolve::ResolvedConstructExpr,
    ) -> Option<crate::identity::InitializerId> {
        let crate::resolve::ResolvedConstructionMode::Initialize { arguments } = &construction.mode
        else {
            unreachable!("copy construction does not select an ordinary initializer");
        };
        self.select_initializer(
            construction.class,
            arguments,
            construction.callee_span,
            InitializerCallSite::DirectConstruction,
        )
    }

    pub(super) fn select_base_initializer(
        &mut self,
        initialization: &crate::resolve::ResolvedBaseInitialization,
    ) -> Option<crate::identity::InitializerId> {
        self.select_initializer(
            initialization.base,
            &initialization.arguments,
            initialization.super_span,
            InitializerCallSite::BaseInitialization,
        )
    }

    pub(in crate::typeck) fn select_allocation_initializer(
        &mut self,
        allocation: &crate::resolve::ResolvedAllocationExpr,
    ) -> Option<crate::identity::InitializerId> {
        let crate::resolve::ResolvedConstructionMode::Initialize { arguments } = &allocation.mode
        else {
            unreachable!("copy allocation does not select an ordinary initializer");
        };
        self.select_initializer(
            allocation.class,
            arguments,
            allocation.target_span,
            InitializerCallSite::Allocation,
        )
    }

    fn select_initializer(
        &mut self,
        class_id: ClassId,
        source_arguments: &[ResolvedExpression],
        callee_span: Span,
        call_site: InitializerCallSite,
    ) -> Option<crate::identity::InitializerId> {
        let class = self
            .program
            .class(class_id)
            .expect("resolved initializer owner class must exist");
        let arguments: Vec<_> = source_arguments
            .iter()
            .map(|argument| self.analyze_argument(argument))
            .collect();
        let applicable: Vec<_> = class
            .initializers
            .iter()
            .filter(|candidate| self.initializer_is_applicable(candidate, &arguments))
            .collect();

        let selected = if applicable.len() == 1 {
            applicable.first().copied()
        } else {
            let maximal: Vec<_> = applicable
                .iter()
                .copied()
                .filter(|candidate| {
                    !applicable.iter().copied().any(|other| {
                        other.id != candidate.id
                            && self.initializer_is_more_specific(other, candidate)
                    })
                })
                .collect();
            (maximal.len() == 1).then(|| maximal[0])
        };
        if let Some(selected) = selected {
            return self
                .check_initializer_access(selected.id, callee_span)
                .then_some(selected.id);
        }

        if applicable.is_empty() {
            self.report_no_matching_initializer(
                class_id,
                callee_span,
                call_site,
                &arguments,
                &class.initializers,
            );
        } else {
            self.report_ambiguous_initializer(
                class_id,
                callee_span,
                call_site,
                &arguments,
                &applicable,
            );
        }
        None
    }

    pub(in crate::typeck) fn check_initializer_access(
        &mut self,
        initializer: crate::identity::InitializerId,
        call_span: Span,
    ) -> bool {
        let declaration = self
            .program
            .initializer(initializer)
            .expect("selected initializer must have declaration metadata");
        let Some(private_span) = declaration
            .visibility
            .private_span()
            .filter(|_| self.class_owner != Some(initializer.class()))
        else {
            return true;
        };
        let class = self
            .program
            .class(initializer.class())
            .expect("initializer owner class must exist");
        self.diagnostics.push(
            Diagnostic::error(
                PRIVATE_INITIALIZER_ACCESS,
                format!("initializer of class `{}` is private", class.name),
            )
            .with_primary_label(call_span, "private initializer is not accessible here")
            .with_secondary_label(private_span, "declared private here")
            .with_note("private access is granted only inside the declaring class"),
        );
        false
    }

    fn analyze_argument(&self, expression: &ResolvedExpression) -> ArgumentAnalysis {
        if matches!(expression, ResolvedExpression::Absent(_)) {
            return ArgumentAnalysis {
                ty: Type::Unit,
                absent: true,
                contextual_optional: None,
                object: None,
                optional_place_access: None,
            };
        }
        if let Some(contextual_optional) = self.contextual_optional_argument(expression) {
            return ArgumentAnalysis {
                ty: Type::Unit,
                absent: false,
                contextual_optional: Some(contextual_optional),
                object: None,
                optional_place_access: None,
            };
        }
        let ty = self.static_expression_type(expression);
        let object = matches!(ty, Type::Class(_) | Type::Interface(_) | Type::Obj)
            .then(|| self.object_argument(expression))
            .flatten();
        ArgumentAnalysis {
            ty,
            absent: false,
            contextual_optional: None,
            object,
            optional_place_access: matches!(ty, Type::Optional(_))
                .then(|| self.static_place_access(expression))
                .flatten(),
        }
    }

    fn contextual_optional_argument(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ContextualOptionalArgument> {
        let mut expression = expression;
        let mut present_layers = 0;
        loop {
            match expression {
                ResolvedExpression::Grouped(grouped) => expression = &grouped.expression,
                ResolvedExpression::Present(present) => {
                    present_layers += 1;
                    expression = &present.value;
                }
                ResolvedExpression::Absent(_) if present_layers > 0 => {
                    return Some(ContextualOptionalArgument {
                        present_layers,
                        terminal: ContextualOptionalTerminal::Absent,
                    });
                }
                _ if present_layers > 0 => {
                    return Some(ContextualOptionalArgument {
                        present_layers,
                        terminal: ContextualOptionalTerminal::Typed(
                            self.static_expression_type(expression),
                        ),
                    });
                }
                _ => return None,
            }
        }
    }

    fn static_place_access(&self, expression: &ResolvedExpression) -> Option<HirAccess> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                Some(self.static_binding_access(binding.binding))
            }
            ResolvedExpression::Grouped(grouped) => self.static_place_access(&grouped.expression),
            ResolvedExpression::FieldAccess(access) => {
                Some(self.static_receiver_access(&access.receiver))
            }
            ResolvedExpression::StaticFieldAccess(_) => Some(HirAccess::Mutable),
            ResolvedExpression::ArrayProjection(projection) => {
                self.static_place_access(&projection.receiver)
            }
            _ => None,
        }
    }

    pub(in crate::typeck) fn static_expression_type(
        &self,
        expression: &ResolvedExpression,
    ) -> Type {
        match expression {
            // `none` has no standalone type. `unit` is the diagnostic-only
            // sentinel already used for malformed projection shapes; callers
            // that select optional arguments handle `none` before this helper.
            ResolvedExpression::Absent(_) | ResolvedExpression::Present(_) => Type::Unit,
            ResolvedExpression::PresenceTest(_) => Type::Bool,
            ResolvedExpression::Unwrap(unwrap) => {
                let source = self.static_expression_type(&unwrap.source);
                super::super::optional_types::optional_id(source)
                    .map(|optional| {
                        super::super::optional_types::payload_type(self.program, optional)
                    })
                    .unwrap_or(Type::Unit)
            }
            ResolvedExpression::Binding(binding) => self.binding_type(binding.binding),
            ResolvedExpression::FunctionReference(reference) => {
                Type::Function(reference.function_type)
            }
            ResolvedExpression::NumericLiteral(literal) => match literal.kind {
                NumericLiteralKind::I64(_) => Type::I64,
                NumericLiteralKind::U64(_) => Type::U64,
                NumericLiteralKind::U8(_) => Type::U8,
                NumericLiteralKind::F64 => Type::F64,
            },
            ResolvedExpression::ByteLiteral(_) => Type::U8,
            ResolvedExpression::StringLiteral(literal) => Type::Class(literal.class),
            ResolvedExpression::Boolean(_) | ResolvedExpression::TypeTest(_) => Type::Bool,
            ResolvedExpression::Unary(unary) => self.static_expression_type(&unary.operand),
            ResolvedExpression::Dereference(dereference) => match dereference.target {
                crate::resolve::ResolvedSharedTarget::Obj => Type::Obj,
                crate::resolve::ResolvedSharedTarget::Class(class) => Type::Class(class),
                crate::resolve::ResolvedSharedTarget::Interface(interface) => {
                    Type::Interface(interface)
                }
                crate::resolve::ResolvedSharedTarget::Array(array) => Type::Array(array),
                crate::resolve::ResolvedSharedTarget::OptionalBox(target) => self
                    .program
                    .optional_box_types
                    .get(target)
                    .and_then(|metadata| metadata.optional)
                    .map_or(Type::Unit, Type::Optional),
            },
            ResolvedExpression::Binary(binary) => match binary.operator {
                crate::resolve::ResolvedBinaryOperator::Equal
                | crate::resolve::ResolvedBinaryOperator::NotEqual
                | crate::resolve::ResolvedBinaryOperator::LessThan
                | crate::resolve::ResolvedBinaryOperator::LessEqual
                | crate::resolve::ResolvedBinaryOperator::GreaterThan
                | crate::resolve::ResolvedBinaryOperator::GreaterEqual => Type::Bool,
                crate::resolve::ResolvedBinaryOperator::Add
                | crate::resolve::ResolvedBinaryOperator::Subtract
                | crate::resolve::ResolvedBinaryOperator::Multiply
                | crate::resolve::ResolvedBinaryOperator::Divide
                | crate::resolve::ResolvedBinaryOperator::Remainder
                | crate::resolve::ResolvedBinaryOperator::ShiftLeft
                | crate::resolve::ResolvedBinaryOperator::ShiftRight
                | crate::resolve::ResolvedBinaryOperator::BitwiseAnd
                | crate::resolve::ResolvedBinaryOperator::BitwiseOr
                | crate::resolve::ResolvedBinaryOperator::BitwiseXor => {
                    self.static_expression_type(&binary.left)
                }
            },
            ResolvedExpression::Logical(_) => Type::Bool,
            ResolvedExpression::PrimitiveCast(cast) => match cast.target {
                crate::resolve::ResolvedPrimitiveType::I64 => Type::I64,
                crate::resolve::ResolvedPrimitiveType::U64 => Type::U64,
                crate::resolve::ResolvedPrimitiveType::U8 => Type::U8,
                crate::resolve::ResolvedPrimitiveType::F64 => Type::F64,
                crate::resolve::ResolvedPrimitiveType::Bool => Type::Bool,
            },
            ResolvedExpression::ObjectCast(cast) => lower_type(self.program, &cast.target),
            ResolvedExpression::DirectCall(call) => self
                .program
                .declarations
                .get(call.function)
                .map(|declaration| lower_type(self.program, &declaration.return_type))
                .expect("resolved direct call must reference a declaration"),
            ResolvedExpression::IndirectCall(call) => self
                .program
                .function_types
                .get(call.function_type)
                .map(|signature| lower_type(self.program, &signature.result))
                .expect("resolved indirect call must reference a canonical function type"),
            ResolvedExpression::StaticCall(call) => self
                .program
                .method(call.method)
                .map(|method| lower_type(self.program, &method.return_type))
                .expect("resolved static call must reference a declaration"),
            ResolvedExpression::Grouped(grouped) => {
                self.static_expression_type(&grouped.expression)
            }
            ResolvedExpression::FieldAccess(access) => self
                .program
                .field(access.field)
                .map(|field| lower_type(self.program, &field.type_syntax))
                .expect("resolved field access must reference a declaration"),
            ResolvedExpression::StaticFieldAccess(access) => self
                .program
                .static_field(access.field)
                .map(|field| lower_type(self.program, &field.type_syntax))
                .expect("resolved static-field access must reference a declaration"),
            ResolvedExpression::MethodCall(call) => self
                .program
                .method(call.method)
                .map(|method| lower_type(self.program, &method.return_type))
                .expect("resolved method call must reference a declaration"),
            ResolvedExpression::InterfaceCall(call) => self
                .program
                .interface(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .map(|requirement| lower_type(self.program, &requirement.return_type))
                .expect("resolved interface call must reference a requirement"),
            ResolvedExpression::Allocation(allocation) => {
                Type::Shared(crate::hir::HirSharedTarget::Class(allocation.class))
            }
            ResolvedExpression::OptionalBoxAllocation(allocation) => {
                Type::Shared(crate::hir::HirSharedTarget::OptionalBox(allocation.target))
            }
            ResolvedExpression::Construct(construction) => Type::Class(construction.class),
            ResolvedExpression::ArrayConstruction(construction) => {
                let ResolvedTypeKind::Array(array) = construction.array_type.kind else {
                    unreachable!("resolved array construction must retain its exact identity")
                };
                if construction.new_span.is_some() {
                    Type::Shared(crate::hir::HirSharedTarget::Array(array))
                } else {
                    Type::Array(array)
                }
            }
            ResolvedExpression::ArrayLength(_) => Type::U64,
            ResolvedExpression::ArrayProjection(projection) => {
                let receiver = self.static_expression_type(&projection.receiver);
                let array = match (projection.operator, receiver) {
                    (
                        crate::resolve::ResolvedArrayProjectionOperator::Ordinary { .. },
                        Type::Array(array),
                    )
                    | (
                        crate::resolve::ResolvedArrayProjectionOperator::Shared { .. },
                        Type::Shared(crate::hir::HirSharedTarget::Array(array)),
                    ) => array,
                    _ => return Type::Unit,
                };
                match projection.bounds {
                    crate::resolve::ResolvedArrayProjectionBounds::Index(_) => {
                        self.copy_capabilities.array(array).element
                    }
                    crate::resolve::ResolvedArrayProjectionBounds::Slice { .. } => {
                        Type::Array(array)
                    }
                }
            }
        }
    }

    fn object_argument(&self, expression: &ResolvedExpression) -> Option<ObjectArgument> {
        match expression {
            ResolvedExpression::Binding(binding) => Some(ObjectArgument {
                access: self.static_binding_access(binding.binding),
                source: ObjectArgumentSource::ExistingPlace,
            }),
            ResolvedExpression::Grouped(grouped) => self.object_argument(&grouped.expression),
            ResolvedExpression::FieldAccess(access) => Some(ObjectArgument {
                access: self.static_receiver_access(&access.receiver),
                source: if access.receiver.cast().is_some() {
                    ObjectArgumentSource::CheckedPlace
                } else {
                    ObjectArgumentSource::ExistingPlace
                },
            }),
            ResolvedExpression::ArrayProjection(projection)
                if matches!(
                    projection.bounds,
                    crate::resolve::ResolvedArrayProjectionBounds::Index(_)
                ) =>
            {
                matches!(
                    self.static_expression_type(expression),
                    Type::Class(_) | Type::Array(_)
                )
                .then(|| ObjectArgument {
                    access: self
                        .static_place_access(&projection.receiver)
                        .unwrap_or(HirAccess::Mutable),
                    source: ObjectArgumentSource::ExistingPlace,
                })
            }
            ResolvedExpression::ObjectCast(cast) => {
                let source = self.object_argument(&cast.source).map_or(
                    ObjectArgumentSource::CheckedPlace,
                    |argument| match argument.source {
                        ObjectArgumentSource::Produced => ObjectArgumentSource::Produced,
                        ObjectArgumentSource::ExistingPlace
                        | ObjectArgumentSource::CheckedPlace => ObjectArgumentSource::CheckedPlace,
                    },
                );
                Some(ObjectArgument {
                    access: self.static_cast_access(&cast.source),
                    source,
                })
            }
            ResolvedExpression::Construct(_)
            | ResolvedExpression::StringLiteral(_)
            | ResolvedExpression::DirectCall(_)
            | ResolvedExpression::IndirectCall(_)
            | ResolvedExpression::StaticCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_) => Some(ObjectArgument {
                access: HirAccess::Mutable,
                source: ObjectArgumentSource::Produced,
            }),
            _ => None,
        }
    }

    fn static_receiver_access(&self, receiver: &ResolvedObjectReceiver) -> HirAccess {
        match receiver {
            ResolvedObjectReceiver::BindingPath(path) => self.static_binding_access(path.root),
            ResolvedObjectReceiver::StaticField { .. } => HirAccess::Mutable,
            ResolvedObjectReceiver::CastRelative { cast, .. } => {
                self.static_cast_access(&cast.source)
            }
            ResolvedObjectReceiver::Dereference { .. } => HirAccess::Mutable,
            ResolvedObjectReceiver::OptionalPayload { unwrap, .. } => {
                self.static_cast_access(&unwrap.source)
            }
            ResolvedObjectReceiver::ArrayElement { projection, .. } => self
                .static_place_access(&projection.receiver)
                .unwrap_or(HirAccess::Mutable),
            ResolvedObjectReceiver::Produced { .. } => HirAccess::ReadOnly,
        }
    }

    fn static_cast_access(&self, source: &ResolvedExpression) -> HirAccess {
        match source {
            ResolvedExpression::Binding(binding) => self.static_binding_access(binding.binding),
            ResolvedExpression::Grouped(grouped) => self.static_cast_access(&grouped.expression),
            ResolvedExpression::FieldAccess(access) => {
                self.static_receiver_access(&access.receiver)
            }
            _ => HirAccess::Mutable,
        }
    }

    fn static_binding_access(&self, binding: BindingId) -> HirAccess {
        match binding {
            BindingId::Receiver(_) => {
                self.receiver
                    .expect("resolved receiver binding must occur in a member")
                    .access
            }
            BindingId::Local(_) => HirAccess::Mutable,
            BindingId::Parameter(id) => match self.parameter(id).binding_mode {
                ResolvedParameterBindingMode::ReadOnlyAlias { .. } => HirAccess::ReadOnly,
                ResolvedParameterBindingMode::Value
                | ResolvedParameterBindingMode::MutableAlias { .. } => HirAccess::Mutable,
            },
        }
    }

    fn initializer_is_applicable(
        &self,
        candidate: &ResolvedInitializerDeclaration,
        arguments: &[ArgumentAnalysis],
    ) -> bool {
        candidate.parameters.len() == arguments.len()
            && candidate
                .parameters
                .iter()
                .zip(arguments)
                .all(|(parameter, argument)| self.parameter_accepts(parameter, argument))
    }

    fn parameter_accepts(
        &self,
        parameter: &ResolvedParameter,
        argument: &ArgumentAnalysis,
    ) -> bool {
        let expected = lower_type(self.program, &parameter.type_syntax);
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value => {
                if let Some(contextual) = argument.contextual_optional {
                    return self.contextual_optional_accepts(expected, contextual);
                }
                match expected {
                    Type::Optional(optional) => {
                        argument.absent || self.optional_parameter_accepts(optional, argument.ty)
                    }
                    Type::Class(target) => {
                        let Type::Class(actual) = argument.ty else {
                            return false;
                        };
                        self.program
                            .hierarchy
                            .is_subtype(actual, target)
                            .unwrap_or(false)
                            && self
                                .copy_capabilities
                                .constructor(target)
                                .selected()
                                .is_some()
                    }
                    Type::Obj | Type::Interface(_) | Type::Unit => false,
                    Type::Shared(expected) => {
                        let Type::Shared(actual) = argument.ty else {
                            return false;
                        };
                        crate::typeck::shared::target_accepts(self.program, expected, actual)
                    }
                    primitive => !argument.absent && argument.ty == primitive,
                }
            }
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. } => {
                let required = match parameter.binding_mode {
                    ResolvedParameterBindingMode::ReadOnlyAlias { .. } => HirAccess::ReadOnly,
                    ResolvedParameterBindingMode::MutableAlias { .. } => HirAccess::Mutable,
                    ResolvedParameterBindingMode::Value => unreachable!(),
                };
                if matches!(expected, Type::Optional(_)) {
                    return argument.ty == expected
                        && argument
                            .optional_place_access
                            .is_some_and(|access| access.permits(required));
                }
                let Some(object) = argument
                    .object
                    .filter(|object| object.source.can_bind_alias(required))
                else {
                    return false;
                };
                object.access.permits(required)
                    && self.parameter_type_accepts(argument.ty, expected)
            }
        }
    }

    fn contextual_optional_accepts(
        &self,
        mut expected: Type,
        argument: ContextualOptionalArgument,
    ) -> bool {
        for _ in 0..argument.present_layers {
            let Type::Optional(optional) = expected else {
                return false;
            };
            expected = super::super::optional_types::payload_type(self.program, optional);
        }
        match argument.terminal {
            ContextualOptionalTerminal::Absent => matches!(expected, Type::Optional(_)),
            ContextualOptionalTerminal::Typed(actual) => {
                self.parameter_type_accepts(actual, expected)
            }
        }
    }

    fn parameter_type_accepts(&self, actual: Type, expected: Type) -> bool {
        match (actual, expected) {
            (actual, expected) if actual == expected => true,
            (Type::Class(actual), Type::Class(expected)) => self
                .program
                .hierarchy
                .is_subtype(actual, expected)
                .unwrap_or(false),
            (Type::Class(actual), Type::Interface(expected)) => class_provides_view(
                self.program,
                actual,
                crate::hir::HirViewTarget::Interface(expected),
            ),
            (Type::Class(_), Type::Obj) | (Type::Interface(_), Type::Obj) => true,
            (Type::Shared(actual), Type::Shared(expected)) => {
                crate::typeck::shared::target_accepts(self.program, expected, actual)
            }
            (actual, Type::Optional(optional)) => self.optional_parameter_accepts(optional, actual),
            _ => false,
        }
    }

    fn initializer_is_more_specific(
        &self,
        candidate: &ResolvedInitializerDeclaration,
        other: &ResolvedInitializerDeclaration,
    ) -> bool {
        let mut strict = false;
        candidate
            .parameters
            .iter()
            .zip(&other.parameters)
            .all(|(candidate, other)| {
                let candidate = lower_type(self.program, &candidate.type_syntax);
                let other = lower_type(self.program, &other.type_syntax);
                let compatible = self.parameter_type_accepts(candidate, other);
                strict |= compatible && candidate != other;
                compatible
            })
            && strict
    }

    fn optional_parameter_accepts(
        &self,
        expected: crate::identity::OptionalTypeId,
        actual: Type,
    ) -> bool {
        if actual == Type::Optional(expected)
            || actual == super::super::optional_types::payload_type(self.program, expected)
        {
            return true;
        }
        let Some(super::super::optional_types::OptionalPayloadKind::Shared(expected_target)) =
            super::super::optional_types::classify_payload(self.program, expected)
        else {
            return false;
        };
        let actual_target = match actual {
            Type::Shared(target) => Some(target),
            Type::Optional(actual) => {
                match super::super::optional_types::classify_payload(self.program, actual) {
                    Some(super::super::optional_types::OptionalPayloadKind::Shared(target)) => {
                        Some(target)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        actual_target.is_some_and(|actual_target| {
            crate::typeck::shared::target_accepts(self.program, expected_target, actual_target)
        })
    }

    fn report_no_matching_initializer(
        &mut self,
        class_id: ClassId,
        callee_span: Span,
        call_site: InitializerCallSite,
        arguments: &[ArgumentAnalysis],
        candidates: &[ResolvedInitializerDeclaration],
    ) {
        let class = self
            .program
            .class(class_id)
            .expect("initializer owner class must exist");
        let owner = match call_site {
            InitializerCallSite::DirectConstruction => "class",
            InitializerCallSite::BaseInitialization => "base class",
            InitializerCallSite::Allocation => "allocation class",
        };
        let mut diagnostic = Diagnostic::error(
            NO_MATCHING_INITIALIZER,
            format!(
                "no initializer of {owner} `{}` matches these arguments",
                class.name,
            ),
        )
        .with_primary_label(
            callee_span,
            format!("supplied ({})", self.argument_type_list(arguments)),
        );
        for candidate in candidates {
            diagnostic = diagnostic.with_secondary_label(
                candidate.span,
                format!("candidate {}", self.initializer_signature(candidate)),
            );
        }
        self.diagnostics.push(diagnostic);
    }

    fn report_ambiguous_initializer(
        &mut self,
        class_id: ClassId,
        callee_span: Span,
        call_site: InitializerCallSite,
        arguments: &[ArgumentAnalysis],
        candidates: &[&ResolvedInitializerDeclaration],
    ) {
        let class = self
            .program
            .class(class_id)
            .expect("initializer owner class must exist");
        let call = match call_site {
            InitializerCallSite::DirectConstruction => "initializer call",
            InitializerCallSite::BaseInitialization => "base initializer call",
            InitializerCallSite::Allocation => "allocation initializer call",
        };
        let mut diagnostic = Diagnostic::error(
            AMBIGUOUS_INITIALIZER,
            format!("{call} for class `{}` is ambiguous", class.name),
        )
        .with_primary_label(
            callee_span,
            format!("supplied ({})", self.argument_type_list(arguments)),
        );
        for candidate in candidates {
            diagnostic = diagnostic.with_secondary_label(
                candidate.span,
                format!(
                    "applicable candidate {}",
                    self.initializer_signature(candidate)
                ),
            );
        }
        self.diagnostics.push(diagnostic);
    }

    fn argument_type_list(&self, arguments: &[ArgumentAnalysis]) -> String {
        arguments
            .iter()
            .map(|argument| {
                if argument.absent {
                    "none".to_owned()
                } else if argument.contextual_optional.is_some() {
                    "some(...)".to_owned()
                } else {
                    self.type_name(argument.ty)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn initializer_signature(&self, initializer: &ResolvedInitializerDeclaration) -> String {
        let parameters = initializer
            .parameters
            .iter()
            .map(|parameter| {
                let mode = match parameter.binding_mode {
                    ResolvedParameterBindingMode::Value => "",
                    ResolvedParameterBindingMode::ReadOnlyAlias { .. } => "ref ",
                    ResolvedParameterBindingMode::MutableAlias { .. } => "mut ref ",
                };
                format!(
                    "{mode}{}",
                    self.type_name(lower_type(self.program, &parameter.type_syntax))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("init({parameters})")
    }

    pub(in crate::typeck) fn type_name(&self, ty: Type) -> String {
        match ty {
            Type::Optional(optional) => format!(
                "{}?",
                self.type_name(super::super::optional_types::payload_type(
                    self.program,
                    optional
                ))
            ),
            Type::Class(class) => self
                .program
                .class(class)
                .map(|class| class.name.clone())
                .unwrap_or_else(|| format!("{class}")),
            Type::Interface(interface) => self
                .program
                .interface(interface)
                .map(|interface| interface.name.clone())
                .unwrap_or_else(|| format!("{interface}")),
            other => other.name().into_owned(),
        }
    }
}
