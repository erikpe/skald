//! Static ordinary-initializer overload analysis and selection.

use super::*;
use crate::{
    hir::{HirAccess, Type},
    literal::NumericLiteralKind,
    resolve::{
        ResolvedExpression, ResolvedInitializerDeclaration, ResolvedObjectReceiver,
        ResolvedParameter, ResolvedParameterBindingMode,
    },
    source::Span,
    typeck::{
        expression::class_provides_view,
        program::{lower_type, AMBIGUOUS_INITIALIZER, NO_MATCHING_INITIALIZER},
    },
};

#[derive(Clone, Copy)]
struct ArgumentAnalysis {
    ty: Type,
    absent: bool,
    object: Option<ObjectArgument>,
    optional_place_access: Option<HirAccess>,
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
    const fn can_bind_alias(self) -> bool {
        matches!(self, Self::ExistingPlace | Self::CheckedPlace)
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
            return Some(selected.id);
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

    fn analyze_argument(&self, expression: &ResolvedExpression) -> ArgumentAnalysis {
        if matches!(expression, ResolvedExpression::Absent(_)) {
            return ArgumentAnalysis {
                ty: Type::Unit,
                absent: true,
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
            object,
            optional_place_access: matches!(
                ty,
                Type::OptionalPrimitive(_) | Type::OptionalClass(_)
            )
            .then(|| self.static_place_access(expression))
            .flatten(),
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
            _ => None,
        }
    }

    fn static_expression_type(&self, expression: &ResolvedExpression) -> Type {
        match expression {
            ResolvedExpression::Absent(_) => {
                unreachable!("absent arguments are handled before static type analysis")
            }
            ResolvedExpression::PresenceTest(_) => Type::Bool,
            ResolvedExpression::Unwrap(unwrap) => {
                match self.static_expression_type(&unwrap.source) {
                    Type::OptionalPrimitive(payload) => payload.payload_type(),
                    Type::OptionalClass(class) => Type::Class(class),
                    _ => unreachable!("resolved unwrap source must have an optional type"),
                }
            }
            ResolvedExpression::Binding(binding) => self.binding_type(binding.binding),
            ResolvedExpression::NumericLiteral(literal) => match literal.kind {
                NumericLiteralKind::I64 => Type::I64,
                NumericLiteralKind::U64 => Type::U64,
                NumericLiteralKind::U8 => Type::U8,
                NumericLiteralKind::F64 => Type::F64,
            },
            ResolvedExpression::Boolean(_) | ResolvedExpression::TypeTest(_) => Type::Bool,
            ResolvedExpression::Unary(unary) => self.static_expression_type(&unary.operand),
            ResolvedExpression::Dereference(dereference) => match dereference.target {
                crate::resolve::ResolvedSharedTarget::Obj => Type::Obj,
                crate::resolve::ResolvedSharedTarget::Class(class) => Type::Class(class),
                crate::resolve::ResolvedSharedTarget::Interface(interface) => {
                    Type::Interface(interface)
                }
                crate::resolve::ResolvedSharedTarget::Array(_) => {
                    panic!("array targets are rejected by the type-checking array gate")
                }
            },
            ResolvedExpression::Binary(binary) => self.static_expression_type(&binary.left),
            ResolvedExpression::ObjectCast(cast) => lower_type(&cast.target),
            ResolvedExpression::DirectCall(call) => self
                .program
                .declarations
                .get(call.function)
                .map(|declaration| lower_type(&declaration.return_type))
                .expect("resolved direct call must reference a declaration"),
            ResolvedExpression::Grouped(grouped) => {
                self.static_expression_type(&grouped.expression)
            }
            ResolvedExpression::FieldAccess(access) => self
                .program
                .field(access.field)
                .map(|field| lower_type(&field.type_syntax))
                .expect("resolved field access must reference a declaration"),
            ResolvedExpression::MethodCall(call) => self
                .program
                .method(call.method)
                .map(|method| lower_type(&method.return_type))
                .expect("resolved method call must reference a declaration"),
            ResolvedExpression::InterfaceCall(call) => self
                .program
                .interface(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .map(|requirement| lower_type(&requirement.return_type))
                .expect("resolved interface call must reference a requirement"),
            ResolvedExpression::Allocation(allocation) => {
                Type::Shared(crate::hir::HirSharedTarget::Class(allocation.class))
            }
            ResolvedExpression::Construct(construction) => Type::Class(construction.class),
            ResolvedExpression::ArrayConstruction(_) | ResolvedExpression::ArrayProjection(_) => {
                panic!("array expressions are rejected by the type-checking array gate")
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
            ResolvedExpression::ObjectCast(cast) => Some(ObjectArgument {
                access: self.static_cast_access(&cast.source),
                source: ObjectArgumentSource::CheckedPlace,
            }),
            ResolvedExpression::Construct(_)
            | ResolvedExpression::DirectCall(_)
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
            ResolvedObjectReceiver::CastRelative { cast, .. } => {
                self.static_cast_access(&cast.source)
            }
            ResolvedObjectReceiver::Dereference { .. } => HirAccess::Mutable,
            ResolvedObjectReceiver::OptionalPayload { unwrap, .. } => {
                self.static_cast_access(&unwrap.source)
            }
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
                .all(|(parameter, argument)| self.parameter_accepts(parameter, *argument))
    }

    fn parameter_accepts(&self, parameter: &ResolvedParameter, argument: ArgumentAnalysis) -> bool {
        let expected = lower_type(&parameter.type_syntax);
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value => match expected {
                Type::OptionalPrimitive(payload) => {
                    argument.absent
                        || argument.ty == Type::OptionalPrimitive(payload)
                        || argument.ty == payload.payload_type()
                }
                Type::OptionalClass(class) => {
                    argument.absent
                        || argument.ty == Type::OptionalClass(class)
                        || argument.ty == Type::Class(class)
                }
                Type::OptionalShared(expected) => {
                    argument.absent
                        || match argument.ty {
                            Type::OptionalShared(actual) | Type::Shared(actual) => {
                                crate::typeck::shared::target_accepts(
                                    self.program,
                                    expected,
                                    actual,
                                )
                            }
                            _ => false,
                        }
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
            },
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. } => {
                let required = match parameter.binding_mode {
                    ResolvedParameterBindingMode::ReadOnlyAlias { .. } => HirAccess::ReadOnly,
                    ResolvedParameterBindingMode::MutableAlias { .. } => HirAccess::Mutable,
                    ResolvedParameterBindingMode::Value => unreachable!(),
                };
                if matches!(
                    expected,
                    Type::OptionalPrimitive(_) | Type::OptionalClass(_)
                ) {
                    return argument.ty == expected
                        && argument
                            .optional_place_access
                            .is_some_and(|access| access.permits(required));
                }
                let Some(object) = argument
                    .object
                    .filter(|object| object.source.can_bind_alias())
                else {
                    return false;
                };
                object.access.permits(required)
                    && self.parameter_type_accepts(argument.ty, expected)
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
            (Type::OptionalShared(actual), Type::OptionalShared(expected))
            | (Type::Shared(actual), Type::OptionalShared(expected)) => {
                crate::typeck::shared::target_accepts(self.program, expected, actual)
            }
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
                let candidate = lower_type(&candidate.type_syntax);
                let other = lower_type(&other.type_syntax);
                let compatible = self.parameter_type_accepts(candidate, other)
                    || matches!(
                        (candidate, other),
                        (candidate, Type::OptionalPrimitive(payload))
                            if candidate == payload.payload_type()
                    )
                    || matches!(
                        (candidate, other),
                        (Type::Class(candidate), Type::OptionalClass(payload))
                            if candidate == payload
                    )
                    || matches!(
                        (candidate, other),
                        (
                            Type::Shared(candidate) | Type::OptionalShared(candidate),
                            Type::OptionalShared(expected)
                        ) if crate::typeck::shared::target_accepts(
                            self.program,
                            expected,
                            candidate
                        )
                    );
                strict |= compatible && candidate != other;
                compatible
            })
            && strict
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
                    self.type_name(lower_type(&parameter.type_syntax))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("init({parameters})")
    }

    fn type_name(&self, ty: Type) -> String {
        match ty {
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
