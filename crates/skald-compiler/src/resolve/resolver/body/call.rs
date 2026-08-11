//! Direct, constructor, method, and interface call/member selection.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_field_access(
        &mut self,
        member: &syntax::MemberAccessExpr,
    ) -> Option<ResolvedExpression> {
        if matches!(member.operator, syntax::MemberAccessOperator::Dot { .. }) {
            match self.class_receiver(&member.receiver) {
                ClassReceiver::Class(class) => {
                    let selected = self.select_member(class, &member.member)?;
                    if let SelectedClassMember::StaticField(field) = selected {
                        return Some(ResolvedExpression::StaticFieldAccess(
                            ResolvedStaticFieldAccessExpr {
                                field,
                                member_span: member.member.span,
                                span: member.span,
                            },
                        ));
                    }
                    self.report_class_member_used_as_value(selected, &member.member);
                    return None;
                }
                ClassReceiver::Diagnosed => return None,
                ClassReceiver::NotClass => {}
            }
        }
        let receiver = self.resolve_member_object_receiver(member)?;
        let selected = self.select_member(receiver.class(), &member.member)?;
        let receiver =
            self.project_receiver_to_declaring_class(receiver, selected.declaring_class());
        match selected {
            SelectedClassMember::Field(field) => {
                Some(ResolvedExpression::FieldAccess(ResolvedFieldAccessExpr {
                    receiver,
                    field,
                    member_span: member.member.span,
                    span: member.span,
                }))
            }
            SelectedClassMember::Method(method) => {
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
            SelectedClassMember::StaticField(field) => {
                self.report_object_selected_static_field(
                    field,
                    &member.member,
                    INVALID_MEMBER_SELECTION,
                    "object-selected static field",
                );
                None
            }
        }
    }

    pub(super) fn resolve_call(&mut self, call: &syntax::CallExpr) -> Option<ResolvedExpression> {
        if let Some(length) = self.resolve_array_length_call(call) {
            return length;
        }
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
            CallTarget::Static {
                method,
                member_span,
            } => ResolvedExpression::StaticCall(ResolvedStaticCallExpr {
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

    fn resolve_array_length_call(
        &mut self,
        call: &syntax::CallExpr,
    ) -> Option<Option<ResolvedExpression>> {
        let syntax::Expression::MemberAccess(member) = &*call.callee else {
            return None;
        };
        if member.member.text != "len" {
            return None;
        }
        let receiver = self.resolve_expression(&member.receiver)?;
        let receiver_type = self.resolved_expression_type(&receiver);
        let operator = match (member.operator, receiver_type) {
            (syntax::MemberAccessOperator::Dot { span }, Some(ResolvedTypeKind::Array(_))) => {
                ResolvedArrayLengthOperator::Ordinary { dot_span: span }
            }
            (
                syntax::MemberAccessOperator::Arrow { span },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(_))),
            ) => ResolvedArrayLengthOperator::Shared { arrow_span: span },
            _ => return None,
        };
        let syntax::CallArguments::Ordinary(syntax_arguments) = &call.arguments else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION_TARGET,
                    "array `len()` does not support `copy` arguments",
                )
                .with_primary_label(call.span, "call `len()` without arguments"),
            );
            return Some(None);
        };
        let mut arguments = Vec::with_capacity(syntax_arguments.len());
        for argument in syntax_arguments {
            arguments.push(self.resolve_expression(argument)?);
        }
        Some(Some(ResolvedExpression::ArrayLength(Box::new(
            ResolvedArrayLengthExpr {
                receiver: Box::new(receiver),
                operator,
                member_span: member.member.span,
                arguments,
                span: call.span,
            },
        ))))
    }

    pub(super) fn resolved_expression_type(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ResolvedTypeKind> {
        match expression {
            ResolvedExpression::Binding(binding) => self
                .scopes
                .iter()
                .rev()
                .flat_map(|scope| scope.values())
                .find(|symbol| symbol.id == binding.binding)
                .map(|symbol| symbol.ty),
            ResolvedExpression::Dereference(dereference) => Some(match dereference.target {
                ResolvedSharedTarget::Obj => ResolvedTypeKind::Obj,
                ResolvedSharedTarget::Class(class) => ResolvedTypeKind::Class(class),
                ResolvedSharedTarget::Interface(interface) => {
                    ResolvedTypeKind::Interface(interface)
                }
                ResolvedSharedTarget::Array(array) => ResolvedTypeKind::Array(array),
                ResolvedSharedTarget::OptionalBox(target) => ResolvedTypeKind::Optional(
                    self.type_interner
                        .optional_box(target)
                        .expect("resolved optional-box target must be interned")
                        .optional?,
                ),
            }),
            ResolvedExpression::Unwrap(unwrap) => {
                if let Some(target) = self.resolved_optional_box_object_leaf(unwrap) {
                    return Some(match target {
                        ResolvedObjectTarget::Class(class) => ResolvedTypeKind::Class(class),
                        ResolvedObjectTarget::Interface(interface) => {
                            ResolvedTypeKind::Interface(interface)
                        }
                        ResolvedObjectTarget::Obj => ResolvedTypeKind::Obj,
                    });
                }
                match self.resolved_expression_type(&unwrap.source)? {
                    ResolvedTypeKind::Optional(optional) => self
                        .type_interner
                        .optional(optional)
                        .map(|entry| entry.payload.kind),
                    _ => None,
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                self.resolved_expression_type(&grouped.expression)
            }
            ResolvedExpression::ArrayConstruction(construction) => {
                let ResolvedTypeKind::Array(array) = construction.array_type.kind else {
                    return None;
                };
                Some(if construction.new_span.is_some() {
                    ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array))
                } else {
                    ResolvedTypeKind::Array(array)
                })
            }
            ResolvedExpression::ArrayProjection(projection) => {
                let receiver = self.resolved_expression_type(&projection.receiver)?;
                let array = match (projection.operator, receiver) {
                    (
                        ResolvedArrayProjectionOperator::Ordinary { .. },
                        ResolvedTypeKind::Array(array),
                    )
                    | (
                        ResolvedArrayProjectionOperator::Shared { .. },
                        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array)),
                    ) => array,
                    _ => return None,
                };
                match projection.bounds {
                    ResolvedArrayProjectionBounds::Index(_) => self
                        .type_interner
                        .array(array)
                        .map(|entry| entry.element.kind),
                    ResolvedArrayProjectionBounds::Slice { .. } => {
                        Some(ResolvedTypeKind::Array(array))
                    }
                }
            }
            ResolvedExpression::ArrayLength(_) => Some(ResolvedTypeKind::U64),
            ResolvedExpression::FieldAccess(access) => self
                .environment
                .classes
                .get(access.field.class())
                .and_then(|class| class.field(access.field))
                .map(|field| field.type_syntax.kind),
            ResolvedExpression::StaticFieldAccess(access) => self
                .environment
                .classes
                .get(access.field.class())
                .and_then(|class| class.static_field(access.field))
                .map(|field| field.type_syntax.kind),
            ResolvedExpression::DirectCall(call) => self
                .environment
                .functions
                .get(call.function)
                .map(|declaration| declaration.return_type.kind),
            ResolvedExpression::StaticCall(call) => self
                .environment
                .classes
                .get(call.method.class())
                .and_then(|class| class.method(call.method))
                .map(|method| method.return_type.kind),
            ResolvedExpression::MethodCall(call) => self
                .environment
                .classes
                .get(call.method.class())
                .and_then(|class| class.method(call.method))
                .map(|method| method.return_type.kind),
            ResolvedExpression::InterfaceCall(call) => self
                .environment
                .interfaces
                .get(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .map(|requirement| requirement.return_type.kind),
            ResolvedExpression::Allocation(allocation) => Some(ResolvedTypeKind::Shared(
                ResolvedSharedTarget::Class(allocation.class),
            )),
            ResolvedExpression::Construct(construction) => {
                Some(ResolvedTypeKind::Class(construction.class))
            }
            _ => None,
        }
    }

    fn resolve_call_target(
        &mut self,
        callee: &syntax::Expression,
        copy_mode: bool,
    ) -> Option<CallTarget> {
        match callee {
            syntax::Expression::Identifier(identifier) => {
                if !identifier.name.is_qualified() {
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
                }
                match self
                    .environment
                    .lookup
                    .select(&identifier.name, self.diagnostics)
                {
                    TopLevelLookup::Found(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Function(function),
                        ..
                    }) => Some(CallTarget::Function(function)),
                    TopLevelLookup::Found(TopLevelSymbol {
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
                    TopLevelLookup::Found(TopLevelSymbol {
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
                    TopLevelLookup::Missing => {
                        self.report_unknown(
                            &identifier.name.text,
                            identifier.span,
                            "unknown function or class",
                        );
                        None
                    }
                    TopLevelLookup::Diagnosed => None,
                }
            }
            syntax::Expression::MemberAccess(member) => {
                if matches!(member.operator, syntax::MemberAccessOperator::Dot { .. }) {
                    match self.class_receiver(&member.receiver) {
                        ClassReceiver::Class(class) => {
                            return self.select_static_call_target(class, member);
                        }
                        ClassReceiver::Diagnosed => return None,
                        ClassReceiver::NotClass => {}
                    }
                }
                let receiver = match member.operator {
                    syntax::MemberAccessOperator::Dot { .. } => {
                        if let Some((receiver, interface, receiver_span)) =
                            self.interface_receiver(&member.receiver)
                        {
                            return self.select_interface_call_target(
                                member,
                                receiver,
                                interface,
                                receiver_span,
                            );
                        }
                        self.resolve_object_receiver(&member.receiver)?
                    }
                    syntax::MemberAccessOperator::Arrow {
                        span: operator_span,
                    } => {
                        let span = self.cover(member.receiver.span(), operator_span);
                        let dereference = self.resolve_dereference(
                            &member.receiver,
                            ResolvedDereferenceOperator::Arrow,
                            operator_span,
                            span,
                        )?;
                        if let ResolvedSharedTarget::Interface(interface) = dereference.target {
                            return self.select_interface_call_target(
                                member,
                                ResolvedInterfaceReceiver::Dereference(Box::new(dereference)),
                                interface,
                                span,
                            );
                        }
                        self.object_receiver_from_dereference(dereference)?
                    }
                };
                let selected = self.select_member(receiver.class(), &member.member)?;
                let receiver =
                    self.project_receiver_to_declaring_class(receiver, selected.declaring_class());
                match selected {
                    SelectedClassMember::Method(method) => {
                        let declaration = self
                            .environment
                            .classes
                            .get(method.class())
                            .and_then(|class| class.method(method))
                            .expect("member symbols must reference declaration metadata");
                        if declaration.kind == ResolvedMethodKind::Static {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    INVALID_CALL_TARGET,
                                    format!(
                                        "static method `{}` must be called through a class",
                                        declaration.name
                                    ),
                                )
                                .with_primary_label(
                                    member.member.span,
                                    "object-selected static method",
                                )
                                .with_secondary_label(
                                    declaration.name_span,
                                    "static method declared here",
                                ),
                            );
                            None
                        } else {
                            Some(CallTarget::Method {
                                receiver,
                                method,
                                member_span: member.member.span,
                            })
                        }
                    }
                    SelectedClassMember::Field(field) => {
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
                    SelectedClassMember::StaticField(field) => {
                        self.report_object_selected_static_field(
                            field,
                            &member.member,
                            INVALID_CALL_TARGET,
                            "object-selected static field",
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

    pub(super) fn class_receiver(&mut self, expression: &syntax::Expression) -> ClassReceiver {
        let syntax::Expression::Identifier(identifier) = expression else {
            return ClassReceiver::NotClass;
        };
        if !identifier.name.is_qualified() && self.lookup_binding(&identifier.name.text).is_some() {
            return ClassReceiver::NotClass;
        }
        match self
            .environment
            .lookup
            .select(&identifier.name, self.diagnostics)
        {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => ClassReceiver::Class(class),
            TopLevelLookup::Diagnosed => ClassReceiver::Diagnosed,
            TopLevelLookup::Found(_) | TopLevelLookup::Missing => ClassReceiver::NotClass,
        }
    }

    fn select_static_call_target(
        &mut self,
        class: ClassId,
        member: &syntax::MemberAccessExpr,
    ) -> Option<CallTarget> {
        let selected = self.select_member(class, &member.member)?;
        match selected {
            SelectedClassMember::Method(method) => {
                let declaration = self
                    .environment
                    .classes
                    .get(method.class())
                    .and_then(|class| class.method(method))
                    .expect("member symbols must reference declaration metadata");
                if declaration.kind == ResolvedMethodKind::Static {
                    Some(CallTarget::Static {
                        method,
                        member_span: member.member.span,
                    })
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CALL_TARGET,
                            format!(
                                "instance method `{}` requires an object receiver",
                                declaration.name
                            ),
                        )
                        .with_primary_label(member.member.span, "class-selected instance method")
                        .with_secondary_label(
                            declaration.name_span,
                            "instance method declared here",
                        ),
                    );
                    None
                }
            }
            SelectedClassMember::Field(field) => {
                let declaration = self
                    .environment
                    .classes
                    .get(field.class())
                    .and_then(|class| class.field(field))
                    .expect("member symbols must reference declaration metadata");
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_TARGET,
                        format!("field `{}` requires an object receiver", declaration.name),
                    )
                    .with_primary_label(member.member.span, "class-selected field")
                    .with_secondary_label(declaration.name_span, "field declared here"),
                );
                None
            }
            SelectedClassMember::StaticField(field) => {
                let declaration = self
                    .environment
                    .classes
                    .get(field.class())
                    .and_then(|class| class.static_field(field))
                    .expect("member symbols must reference declaration metadata");
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_TARGET,
                        format!("static field `{}` is not callable", declaration.name),
                    )
                    .with_primary_label(member.member.span, "called here")
                    .with_secondary_label(declaration.name_span, "static field declared here"),
                );
                None
            }
        }
    }

    fn report_class_member_used_as_value(
        &mut self,
        selected: SelectedClassMember,
        name: &syntax::Name,
    ) {
        match selected {
            SelectedClassMember::Method(method) => {
                let declaration = self
                    .environment
                    .classes
                    .get(method.class())
                    .and_then(|class| class.method(method))
                    .expect("member symbols must reference declaration metadata");
                let message = if declaration.kind == ResolvedMethodKind::Static {
                    format!(
                        "static method `{}` cannot be used as a value",
                        declaration.name
                    )
                } else {
                    format!(
                        "instance method `{}` requires an object receiver",
                        declaration.name
                    )
                };
                self.diagnostics.push(
                    Diagnostic::error(INVALID_MEMBER_SELECTION, message)
                        .with_primary_label(name.span, "call the method with `(...)`")
                        .with_secondary_label(declaration.name_span, "method declared here"),
                );
            }
            SelectedClassMember::Field(field) => {
                let declaration = self
                    .environment
                    .classes
                    .get(field.class())
                    .and_then(|class| class.field(field))
                    .expect("member symbols must reference declaration metadata");
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_MEMBER_SELECTION,
                        format!("field `{}` requires an object receiver", declaration.name),
                    )
                    .with_primary_label(name.span, "class-selected field")
                    .with_secondary_label(declaration.name_span, "field declared here"),
                );
            }
            SelectedClassMember::StaticField(_) => {
                unreachable!("class-selected static fields are values")
            }
        }
    }

    fn select_interface_call_target(
        &mut self,
        member: &syntax::MemberAccessExpr,
        receiver: ResolvedInterfaceReceiver,
        interface: crate::identity::InterfaceId,
        receiver_span: Span,
    ) -> Option<CallTarget> {
        let declaration = self
            .environment
            .interfaces
            .get(interface)
            .expect("interface receiver type must reference a declaration");
        let Some(requirement) = declaration
            .requirements
            .iter()
            .find(|requirement| requirement.name == member.member.text.as_str())
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
        Some(CallTarget::Interface {
            receiver,
            interface,
            requirement: requirement.id,
            receiver_span,
            member_span: member.member.span,
        })
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
            syntax::Expression::Unary(unary)
                if unary.operator == syntax::UnaryOperator::Dereference =>
            {
                let dereference = self.resolve_dereference(
                    &unary.operand,
                    ResolvedDereferenceOperator::Star,
                    unary.operator_span,
                    unary.span,
                )?;
                self.interface_receiver_from_dereference(dereference)
            }
            syntax::Expression::Identifier(identifier) => {
                let binding = self.lookup_binding(&identifier.name.text)?;
                let interface = match binding.ty {
                    ResolvedTypeKind::Interface(interface) => interface,
                    ResolvedTypeKind::Shared(_) => return None,
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
                    return None;
                }
                Some((
                    ResolvedInterfaceReceiver::Cast(Box::new(cast)),
                    interface,
                    span,
                ))
            }
            syntax::Expression::Unwrap(_) => {
                let resolved = self.resolve_expression(expression)?;
                let ResolvedExpression::Unwrap(unwrap) = resolved else {
                    unreachable!("unwrap syntax must retain its resolved node")
                };
                let Some(ResolvedObjectTarget::Interface(interface)) =
                    self.resolved_optional_box_object_leaf(&unwrap)
                else {
                    return None;
                };
                let span = unwrap.span;
                Some((
                    ResolvedInterfaceReceiver::OptionalBoxPayload(Box::new(unwrap)),
                    interface,
                    span,
                ))
            }
            _ => None,
        }
    }

    fn interface_receiver_from_dereference(
        &self,
        dereference: ResolvedDereferenceExpr,
    ) -> Option<(
        ResolvedInterfaceReceiver,
        crate::identity::InterfaceId,
        Span,
    )> {
        let ResolvedSharedTarget::Interface(interface) = dereference.target else {
            return None;
        };
        let span = dereference.span;
        Some((
            ResolvedInterfaceReceiver::Dereference(Box::new(dereference)),
            interface,
            span,
        ))
    }
    pub(super) fn select_member(
        &mut self,
        class: ClassId,
        name: &syntax::Name,
    ) -> Option<SelectedClassMember> {
        let member = self
            .environment
            .hierarchy
            .member(class, &name.text)
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
            })?;

        let declaring_class = member.declaring_class();
        let private_span = match member {
            ResolvedClassMember::Field(field) => self
                .environment
                .classes
                .get(declaring_class)
                .and_then(|class| class.field(field))
                .expect("selected field must have declaration metadata")
                .visibility
                .private_span(),
            ResolvedClassMember::StaticField(field) => self
                .environment
                .classes
                .get(declaring_class)
                .and_then(|class| class.static_field(field))
                .expect("selected static field must have declaration metadata")
                .visibility
                .private_span(),
            ResolvedClassMember::Method(method) => self
                .environment
                .classes
                .get(declaring_class)
                .and_then(|class| class.method(method))
                .expect("selected method must have declaration metadata")
                .visibility
                .private_span(),
        };
        if let Some(private_span) =
            private_span.filter(|_| self.class_owner != Some(declaring_class))
        {
            let owner = self
                .environment
                .classes
                .get(declaring_class)
                .expect("selected member owner must exist");
            self.diagnostics.push(
                Diagnostic::error(
                    PRIVATE_MEMBER_ACCESS,
                    format!(
                        "member `{}` is private to class `{}`",
                        name.text, owner.name
                    ),
                )
                .with_primary_label(name.span, "private member is not accessible here")
                .with_secondary_label(private_span, "declared private here")
                .with_note("private access is granted only inside the declaring class"),
            );
            return None;
        }

        Some(match member {
            ResolvedClassMember::Field(field) => SelectedClassMember::Field(field),
            ResolvedClassMember::StaticField(field) => SelectedClassMember::StaticField(field),
            ResolvedClassMember::Method(method) => SelectedClassMember::Method(method),
        })
    }

    pub(super) fn report_object_selected_static_field(
        &mut self,
        field: StaticFieldId,
        name: &syntax::Name,
        code: &'static str,
        label: &'static str,
    ) {
        let declaration = self
            .environment
            .classes
            .get(field.class())
            .and_then(|class| class.static_field(field))
            .expect("selected static field must have declaration metadata");
        self.diagnostics.push(
            Diagnostic::error(
                code,
                format!(
                    "static field `{}` must be selected through a class",
                    declaration.name
                ),
            )
            .with_primary_label(name.span, label)
            .with_secondary_label(declaration.name_span, "static field declared here"),
        );
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
    Static {
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

pub(super) enum ClassReceiver {
    NotClass,
    Class(ClassId),
    Diagnosed,
}
