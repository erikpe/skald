//! Direct, constructor, method, and interface call/member selection.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_field_access(
        &mut self,
        member: &syntax::MemberAccessExpr,
    ) -> Option<ResolvedExpression> {
        let receiver = self.resolve_object_receiver(&member.receiver)?;
        let selected = self.select_member(receiver.class(), &member.member)?;
        let receiver =
            self.project_receiver_to_declaring_class(receiver, selected.declaring_class());
        match selected {
            OrdinaryMemberSymbolKind::Field(field) => {
                Some(ResolvedExpression::FieldAccess(ResolvedFieldAccessExpr {
                    receiver,
                    field,
                    member_span: member.member.span,
                    span: member.span,
                }))
            }
            OrdinaryMemberSymbolKind::Method(method) => {
                let declaration = self
                    .environment
                    .classes
                    .get(method.class())
                    .and_then(|class| class.method(method))
                    .expect("member symbols must reference declaration metadata");
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_MEMBER_SELECTION,
                        format!("method `{}` cannot be used as a value", declaration.name),
                    )
                    .with_primary_label(member.member.span, "call the method with `(...)`")
                    .with_secondary_label(declaration.name_span, "method declared here"),
                );
                None
            }
        }
    }

    pub(super) fn resolve_call(&mut self, call: &syntax::CallExpr) -> Option<ResolvedExpression> {
        let copy_mode = matches!(call.arguments, syntax::CallArguments::Copy { .. });
        let target = self.resolve_call_target(&call.callee, copy_mode)?;
        if let syntax::CallArguments::Copy { copy_span, source } = &call.arguments {
            let source = self.resolve_expression(source)?;
            return match target {
                CallTarget::Constructor { class } => {
                    Some(ResolvedExpression::Construct(ResolvedConstructExpr {
                        class,
                        callee_span: call.callee.span(),
                        mode: ResolvedConstructionMode::Copy {
                            copy_span: *copy_span,
                            source: Box::new(source),
                        },
                        span: call.span,
                    }))
                }
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CONSTRUCTION_TARGET,
                            "`copy` construction requires a concrete class",
                        )
                        .with_primary_label(*copy_span, "copy-construction marker used here")
                        .with_secondary_label(
                            call.callee.span(),
                            "this callee does not name a class",
                        ),
                    );
                    None
                }
            };
        }

        let syntax::CallArguments::Ordinary(syntax_arguments) = &call.arguments else {
            unreachable!("copy construction returned above");
        };
        let mut arguments = Vec::with_capacity(syntax_arguments.len());
        let mut valid = true;
        for argument in syntax_arguments {
            match self.resolve_expression(argument) {
                Some(argument) => arguments.push(argument),
                None => valid = false,
            }
        }
        if !valid {
            return None;
        }
        Some(match target {
            CallTarget::Function(function) => {
                ResolvedExpression::DirectCall(ResolvedDirectCallExpr {
                    function,
                    callee_span: call.callee.span(),
                    arguments,
                    span: call.span,
                })
            }
            CallTarget::Constructor { class } => {
                ResolvedExpression::Construct(ResolvedConstructExpr {
                    class,
                    callee_span: call.callee.span(),
                    mode: ResolvedConstructionMode::Initialize { arguments },
                    span: call.span,
                })
            }
            CallTarget::Method {
                receiver,
                method,
                member_span,
            } => ResolvedExpression::MethodCall(ResolvedMethodCallExpr {
                receiver,
                method,
                member_span,
                arguments,
                span: call.span,
            }),
            CallTarget::Interface {
                receiver,
                interface,
                requirement,
                receiver_span,
                member_span,
            } => ResolvedExpression::InterfaceCall(ResolvedInterfaceCallExpr {
                receiver,
                interface,
                requirement,
                receiver_span,
                member_span,
                arguments,
                span: call.span,
            }),
        })
    }

    fn resolve_call_target(
        &mut self,
        callee: &syntax::Expression,
        copy_mode: bool,
    ) -> Option<CallTarget> {
        match callee {
            syntax::Expression::Identifier(identifier) => {
                if let Some(binding) = self.lookup_binding(&identifier.name.text) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CALL_TARGET,
                            format!("binding `{}` is not callable", identifier.name.text),
                        )
                        .with_primary_label(identifier.span, "called here")
                        .with_secondary_label(binding.name_span, "binding declared here"),
                    );
                    return None;
                }
                match self
                    .environment
                    .top_levels
                    .get(&identifier.name.text)
                    .copied()
                {
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Function(function),
                        ..
                    }) => Some(CallTarget::Function(function)),
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Class(class),
                        ..
                    }) => {
                        if !copy_mode
                            && self
                                .environment
                                .classes
                                .get(class)
                                .is_none_or(|class| class.initializers.is_empty())
                        {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    INVALID_CONSTRUCTION_TARGET,
                                    format!("class `{}` has no initializer", identifier.name.text),
                                )
                                .with_primary_label(
                                    identifier.span,
                                    "construction requires an explicit `init` declaration",
                                ),
                            );
                            return None;
                        }
                        Some(CallTarget::Constructor { class })
                    }
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Interface(_),
                        ..
                    }) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_CALL_TARGET,
                                format!("interface `{}` is not callable", identifier.name.text),
                            )
                            .with_primary_label(
                                identifier.span,
                                "interfaces describe non-owning views and cannot be constructed",
                            ),
                        );
                        None
                    }
                    None => {
                        self.report_unknown(
                            &identifier.name.text,
                            identifier.span,
                            "unknown function or class",
                        );
                        None
                    }
                }
            }
            syntax::Expression::MemberAccess(member) => {
                if let Some((receiver, interface, receiver_span)) =
                    self.interface_receiver(&member.receiver)
                {
                    let declaration = self
                        .environment
                        .interfaces
                        .get(interface)
                        .expect("interface binding type must reference a declaration");
                    let Some(requirement) = declaration
                        .requirements
                        .iter()
                        .find(|requirement| requirement.name == member.member.text)
                    else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                UNKNOWN_MEMBER,
                                format!(
                                    "interface `{}` has no requirement `{}`",
                                    declaration.name, member.member.text
                                ),
                            )
                            .with_primary_label(member.member.span, "unknown requirement"),
                        );
                        return None;
                    };
                    return Some(CallTarget::Interface {
                        receiver,
                        interface,
                        requirement: requirement.id,
                        receiver_span,
                        member_span: member.member.span,
                    });
                }
                let receiver = self.resolve_object_receiver(&member.receiver)?;
                let selected = self.select_member(receiver.class(), &member.member)?;
                let receiver =
                    self.project_receiver_to_declaring_class(receiver, selected.declaring_class());
                match selected {
                    OrdinaryMemberSymbolKind::Method(method) => Some(CallTarget::Method {
                        receiver,
                        method,
                        member_span: member.member.span,
                    }),
                    OrdinaryMemberSymbolKind::Field(field) => {
                        let declaration = self
                            .environment
                            .classes
                            .get(field.class())
                            .and_then(|class| class.field(field))
                            .expect("member symbols must reference declaration metadata");
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_CALL_TARGET,
                                format!("field `{}` is not callable", declaration.name),
                            )
                            .with_primary_label(member.member.span, "called here")
                            .with_secondary_label(declaration.name_span, "field declared here"),
                        );
                        None
                    }
                }
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_CALL_TARGET, "invalid call target")
                        .with_primary_label(
                            callee.span(),
                            "expected a function, class, or ungrouped method selection",
                        ),
                );
                None
            }
        }
    }

    fn interface_receiver(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<(
        ResolvedInterfaceReceiver,
        crate::identity::InterfaceId,
        Span,
    )> {
        match expression {
            syntax::Expression::Identifier(identifier) => {
                let binding = self.lookup_binding(&identifier.name.text)?;
                let interface = match binding.ty {
                    ResolvedTypeKind::Interface(interface)
                    | ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::Interface(
                        interface,
                    )) => interface,
                    _ => return None,
                };
                Some((
                    ResolvedInterfaceReceiver::Binding {
                        binding: binding.id,
                        span: identifier.span,
                    },
                    interface,
                    identifier.span,
                ))
            }
            syntax::Expression::Grouped(grouped) => self
                .interface_receiver(&grouped.expression)
                .map(|(receiver, interface, _)| (receiver, interface, grouped.span)),
            syntax::Expression::ObjectCast(_) => {
                let resolved = self.resolve_expression(expression)?;
                let ResolvedExpression::ObjectCast(cast) = resolved else {
                    unreachable!("object cast syntax must resolve as an object cast")
                };
                let ResolvedTypeKind::Interface(interface) = cast.target.kind else {
                    return None;
                };
                let span = cast.span;
                if matches!(
                    cast.target_mode,
                    crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
                ) {
                    return Some((
                        ResolvedInterfaceReceiver::SharedExpression(Box::new(
                            ResolvedExpression::ObjectCast(cast),
                        )),
                        interface,
                        span,
                    ));
                }
                Some((
                    ResolvedInterfaceReceiver::Cast(Box::new(cast)),
                    interface,
                    span,
                ))
            }
            syntax::Expression::MemberAccess(_) | syntax::Expression::Call(_) => {
                let resolved = self.resolve_expression(expression)?;
                let interface = self.shared_expression_interface(&resolved)?;
                let span = expression.span();
                Some((
                    ResolvedInterfaceReceiver::SharedExpression(Box::new(resolved)),
                    interface,
                    span,
                ))
            }
            _ => None,
        }
    }

    fn shared_expression_interface(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<crate::identity::InterfaceId> {
        let kind = match expression {
            ResolvedExpression::FieldAccess(access) => {
                self.environment
                    .classes
                    .get(access.field.class())?
                    .field(access.field)?
                    .type_syntax
                    .kind
            }
            ResolvedExpression::DirectCall(call) => {
                self.environment
                    .functions
                    .get(call.function)?
                    .return_type
                    .kind
            }
            ResolvedExpression::MethodCall(call) => {
                self.environment
                    .classes
                    .get(call.method.class())?
                    .method(call.method)?
                    .return_type
                    .kind
            }
            ResolvedExpression::InterfaceCall(call) => {
                self.environment
                    .interfaces
                    .get(call.interface)?
                    .requirements
                    .get(call.requirement.index())?
                    .return_type
                    .kind
            }
            ResolvedExpression::ObjectCast(cast) => match cast.target_mode {
                crate::resolve::ResolvedObjectCastTargetMode::Shared { .. } => cast.target.kind,
                crate::resolve::ResolvedObjectCastTargetMode::Plain => return None,
            },
            ResolvedExpression::Grouped(grouped) => {
                return self.shared_expression_interface(&grouped.expression)
            }
            _ => return None,
        };
        match kind {
            ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::Interface(
                interface,
            )) => Some(interface),
            _ => None,
        }
    }

    pub(super) fn select_member(
        &mut self,
        class: ClassId,
        name: &syntax::Name,
    ) -> Option<OrdinaryMemberSymbolKind> {
        self.environment
            .hierarchy
            .member(class, &name.text)
            .map(|member| match member {
                ResolvedClassMember::Field(field) => OrdinaryMemberSymbolKind::Field(field),
                ResolvedClassMember::Method(method) => OrdinaryMemberSymbolKind::Method(method),
            })
            .or_else(|| {
                let class_name = &self
                    .environment
                    .classes
                    .get(class)
                    .expect("resolved object place must reference a class")
                    .name;
                self.diagnostics.push(
                    Diagnostic::error(
                        UNKNOWN_MEMBER,
                        format!("class `{class_name}` has no member `{}`", name.text),
                    )
                    .with_primary_label(name.span, "unknown member"),
                );
                None
            })
    }
}

enum CallTarget {
    Function(FunctionId),
    Constructor {
        class: ClassId,
    },
    Method {
        receiver: ResolvedObjectReceiver,
        method: MethodId,
        member_span: Span,
    },
    Interface {
        receiver: ResolvedInterfaceReceiver,
        interface: crate::identity::InterfaceId,
        requirement: crate::identity::InterfaceRequirementId,
        receiver_span: Span,
        member_span: Span,
    },
}
