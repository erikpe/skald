//! Top-level declarations, parameters, and source type syntax.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TypeContext {
    Result,
    ValueParameter,
    AliasParameter,
    ArrayElement,
    LocalValue,
    Field,
    StaticField,
}

impl TypeContext {
    const fn accepts_unit(self) -> bool {
        matches!(self, Self::Result | Self::StaticField)
    }

    const fn accepts_named(self) -> bool {
        matches!(
            self,
            Self::Result
                | Self::ValueParameter
                | Self::AliasParameter
                | Self::ArrayElement
                | Self::LocalValue
                | Self::Field
                | Self::StaticField
        )
    }

    const fn accepts_primitive(self) -> bool {
        true
    }

    const fn accepts_shared(self) -> bool {
        !matches!(self, Self::AliasParameter)
    }

    fn expected_label(self) -> String {
        match self {
            Self::Result => format!(
                "expected {}, a named class type, or a shared object type",
                format_type_list(RESULT_TYPE_NAMES)
            ),
            Self::ValueParameter => format!(
                "value parameters must have type {}, a named class type, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
            Self::AliasParameter => {
                "alias parameters must name an object view or supported inline optional type"
                    .to_owned()
            }
            Self::LocalValue => format!(
                "locals must have type {}, a named class type, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
            Self::Field | Self::StaticField => format!(
                "fields must have type {}, a named class type, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
            Self::ArrayElement => "expected an array element type".to_owned(),
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_function(&mut self, visibility: Visibility) -> Option<FunctionDecl> {
        let fn_token = self.advance();
        let name = self.parse_name("expected a function name after `fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type(TypeContext::Result, "expected a return type after `->`");
        let body = self.parse_block();

        let (name, parameters, return_type, body) = match (name, parameters, return_type, body) {
            (Some(name), Some(parameters), Some(return_type), Some(body)) => {
                (name, parameters, return_type, body)
            }
            _ => return None,
        };
        let span = self.cover(visibility.start_span(fn_token.span), body.span);

        Some(FunctionDecl {
            visibility,
            name,
            parameters,
            return_type,
            body,
            span,
        })
    }

    pub(super) fn parse_external_function(
        &mut self,
        visibility: Visibility,
    ) -> Option<ExternalFunctionDecl> {
        let extern_token = self.advance();
        self.expect(TokenKind::Fn, "`fn` after `extern`")?;
        let name = self.parse_name("expected a function name after `extern fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type(TypeContext::Result, "expected a return type after `->`");
        let semicolon = self.expect(
            TokenKind::Semicolon,
            "`;` after the external function declaration",
        );

        let (name, parameters, return_type) = match (name, parameters, return_type) {
            (Some(name), Some(parameters), Some(return_type)) => (name, parameters, return_type),
            _ => return None,
        };
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or(return_type.span);
        Some(ExternalFunctionDecl {
            visibility,
            name,
            parameters,
            return_type,
            span: self.cover(visibility.start_span(extern_token.span), end_span),
        })
    }

    pub(super) fn parse_intrinsic_function(
        &mut self,
        visibility: Visibility,
    ) -> Option<IntrinsicFunctionDecl> {
        let intrinsic_token = self.advance();
        self.expect(TokenKind::Fn, "`fn` after `intrinsic`")?;
        let name = self.parse_name("expected a function name after `intrinsic fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type(TypeContext::Result, "expected a return type after `->`");
        let semicolon = self.expect(
            TokenKind::Semicolon,
            "`;` after the intrinsic function declaration",
        );

        let (name, parameters, return_type) = match (name, parameters, return_type) {
            (Some(name), Some(parameters), Some(return_type)) => (name, parameters, return_type),
            _ => return None,
        };
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or(return_type.span);
        Some(IntrinsicFunctionDecl {
            visibility,
            intrinsic_span: intrinsic_token.span,
            name,
            parameters,
            return_type,
            span: self.cover(visibility.start_span(intrinsic_token.span), end_span),
        })
    }

    pub(super) fn parse_parameter_list(&mut self) -> Option<Vec<Parameter>> {
        self.expect(TokenKind::LeftParen, "`(` after the function name");
        let mut parameters = Vec::new();
        let mut valid = true;

        if self.consume(TokenKind::RightParen).is_some() {
            return Some(parameters);
        }

        loop {
            if self.at_any(&[
                TokenKind::RightParen,
                TokenKind::Arrow,
                TokenKind::LeftBrace,
                TokenKind::Semicolon,
                TokenKind::Fn,
                TokenKind::Extern,
                TokenKind::Eof,
            ]) {
                break;
            }

            if let Some(parameter) = self.parse_parameter() {
                parameters.push(parameter);
            } else {
                valid = false;
                self.synchronize_parameter();
            }

            if self.consume(TokenKind::Comma).is_some() {
                if self.at(TokenKind::RightParen) {
                    self.report(
                        EXPECTED_TOKEN,
                        "expected a parameter after `,`",
                        self.peek().span,
                        "trailing commas are not supported",
                    );
                    valid = false;
                    break;
                }
                continue;
            }

            if self.starts_parameter() {
                self.report(
                    EXPECTED_TOKEN,
                    "expected `,` between parameters",
                    self.peek().span,
                    "insert `,` before this parameter",
                );
                continue;
            }

            break;
        }

        self.expect(TokenKind::RightParen, "`)` after the parameters");
        valid.then_some(parameters)
    }

    fn parse_parameter(&mut self) -> Option<Parameter> {
        let binding_mode = self.parse_parameter_binding_mode()?;
        let name = self.parse_name("expected a parameter name");
        self.expect(TokenKind::Colon, "`:` after the parameter name");
        let type_context = if binding_mode == ParameterBindingMode::Value {
            TypeContext::ValueParameter
        } else {
            TypeContext::AliasParameter
        };
        let type_syntax = self.parse_type(
            type_context,
            if type_context == TypeContext::AliasParameter {
                "expected an object view or inline optional alias parameter type".to_owned()
            } else {
                format!(
                    "expected the parameter type {}, a named class type, or a shared object type",
                    format_type_list(STORED_TYPE_NAMES)
                )
            },
        );

        match (name, type_syntax) {
            (Some(name), Some(type_syntax)) => {
                let span = self.cover(binding_mode.start_span(name.span), type_syntax.span);
                Some(Parameter {
                    binding_mode,
                    name,
                    type_syntax,
                    span,
                })
            }
            _ => None,
        }
    }

    fn parse_parameter_binding_mode(&mut self) -> Option<ParameterBindingMode> {
        if let Some(mut_token) = self.consume(TokenKind::Mut) {
            let Some(ref_token) = self.consume(TokenKind::Ref) else {
                self.report(
                    EXPECTED_TOKEN,
                    "expected `ref` after `mut` in a parameter",
                    self.peek().span,
                    "mutable alias parameters use `mut ref name: Class`",
                );
                return None;
            };
            if self.at(TokenKind::Question) {
                self.report_optional_reference();
                return None;
            }
            if self.at_any(&[TokenKind::Mut, TokenKind::Ref]) {
                self.report_repeated_parameter_modifier();
                return None;
            }
            return Some(ParameterBindingMode::MutableAlias {
                mut_span: mut_token.span,
                ref_span: ref_token.span,
            });
        }

        if let Some(ref_token) = self.consume(TokenKind::Ref) {
            if self.at(TokenKind::Question) {
                self.report_optional_reference();
                return None;
            }
            if self.at(TokenKind::Mut) {
                self.report(
                    EXPECTED_TOKEN,
                    "`mut` must precede `ref` in a mutable alias parameter",
                    self.peek().span,
                    "use `mut ref name: Class`",
                );
                return None;
            }
            if self.at(TokenKind::Ref) {
                self.report_repeated_parameter_modifier();
                return None;
            }
            return Some(ParameterBindingMode::ReadOnlyAlias {
                ref_span: ref_token.span,
            });
        }

        Some(ParameterBindingMode::Value)
    }

    fn report_optional_reference(&mut self) {
        let question = self.advance();
        while self.consume(TokenKind::Question).is_some() {}
        self.report(
            INVALID_OPTIONAL_TYPE,
            "optional references are not supported",
            question.span,
            "place `?` on the designated value type, not on `ref`",
        );
    }

    fn report_repeated_parameter_modifier(&mut self) {
        self.report(
            EXPECTED_TOKEN,
            "repeated alias parameter modifier",
            self.peek().span,
            "use exactly `ref` or `mut ref`",
        );
    }

    fn starts_parameter(&self) -> bool {
        self.at_any(&[TokenKind::Identifier, TokenKind::Mut, TokenKind::Ref])
    }

    pub(super) fn parse_type(
        &mut self,
        context: TypeContext,
        message: impl Into<String>,
    ) -> Option<TypeSyntax> {
        self.parse_type_inner(context, message.into())
    }

    fn parse_type_inner(&mut self, context: TypeContext, message: String) -> Option<TypeSyntax> {
        let token = self.peek();
        if context == TypeContext::AliasParameter
            && self.at_contextual("shared")
            && self.at_any_ahead(1, &[TokenKind::Identifier, TokenKind::Question])
        {
            let shared = self.advance();
            self.consume(TokenKind::Question);
            let target = self.parse_name_path("a class, interface, or `Obj` after `shared`")?;
            let end = self
                .consume(TokenKind::Question)
                .map_or(target.span, |question| question.span);
            self.report(
                INVALID_OPTIONAL_TYPE,
                "aliases to shared owners are not supported",
                self.cover(shared.span, end),
                "pass the shared owner by value; aliases may designate inline optional containers",
            );
            return None;
        }

        let mut type_syntax = if context.accepts_shared() && self.at_contextual("shared") {
            let shared = self.advance();
            if let Some(question) = self.consume(TokenKind::Question) {
                if self.at(TokenKind::Question) {
                    let _ = self.parse_name_path("a class, interface, or `Obj` after `shared?`")?;
                }
                let target = self.with_syntax_nesting(shared.span, |parser| {
                    parser.parse_type_inner(
                        TypeContext::ArrayElement,
                        "expected a type after `shared?`".to_owned(),
                    )
                })?;
                if let TypeKind::Optional { question_span, .. } = &target.kind {
                    self.report(
                        INVALID_OPTIONAL_TYPE,
                        "optional shared boxes are not supported",
                        *question_span,
                        "`shared? T?` is reserved for a future boxed-optional design",
                    );
                    return None;
                }
                TypeSyntax {
                    span: self.cover(shared.span, target.span),
                    kind: TypeKind::OptionalShared {
                        shared_span: shared.span,
                        question_span: question.span,
                        target: Box::new(target),
                    },
                }
            } else {
                let target = self.with_syntax_nesting(shared.span, |parser| {
                    parser.parse_type_inner(
                        TypeContext::ArrayElement,
                        "expected a type after `shared`".to_owned(),
                    )
                })?;
                if let TypeKind::Optional { question_span, .. } = &target.kind {
                    self.report(
                        INVALID_OPTIONAL_TYPE,
                        "shared boxes containing optional payloads are not supported",
                        *question_span,
                        "`shared T?` is reserved for a future boxed-optional design",
                    );
                    return None;
                }
                TypeSyntax {
                    span: self.cover(shared.span, target.span),
                    kind: TypeKind::Shared {
                        shared_span: shared.span,
                        target: Box::new(target),
                    },
                }
            }
        } else if let Some(kind) = token_type_kind(token.kind) {
            if kind == TypeKind::Unit && self.peek_ahead(1).kind == TokenKind::Question {
                self.advance();
                let question = self.advance();
                self.report(
                    INVALID_OPTIONAL_TYPE,
                    "`unit?` is not a valid optional type",
                    question.span,
                    "`unit` has no value payload to make optional",
                );
                self.consume_repeated_questions();
                return None;
            }
            if context.accepts_primitive()
                && (kind != TypeKind::Unit
                    || context.accepts_unit()
                    || self.peek_ahead(1).kind == TokenKind::LeftBracket)
            {
                self.advance();
                if let Some(question) = self.consume(TokenKind::Question) {
                    if self.at(TokenKind::Question) {
                        self.reject_optional_suffix(
                            "nested optional types are not supported",
                            "use exactly one `?` marker",
                        );
                        return None;
                    }
                    TypeSyntax {
                        kind: TypeKind::Optional {
                            payload: optional_payload_kind(kind)
                                .expect("validated primitive optional payload"),
                            payload_span: token.span,
                            question_span: question.span,
                        },
                        span: self.cover(token.span, question.span),
                    }
                } else {
                    TypeSyntax {
                        kind,
                        span: token.span,
                    }
                }
            } else {
                self.report(
                    EXPECTED_TOKEN,
                    message,
                    token.span,
                    context.expected_label(),
                );
                self.advance();
                return None;
            }
        } else if context.accepts_named() && token.kind == TokenKind::Identifier {
            let name = self.parse_name_path("expected a named type")?;
            let name_span = name.span;
            if let Some(question) = self.consume(TokenKind::Question) {
                if name.text == "Obj" {
                    self.report(
                        INVALID_OPTIONAL_TYPE,
                        "`Obj?` is not a valid inline optional type",
                        question.span,
                        "use `shared? Obj` for an optional owning object view",
                    );
                    self.consume_repeated_questions();
                    return None;
                }
                if self.at(TokenKind::Question) {
                    self.reject_optional_suffix(
                        "nested optional types are not supported",
                        "use exactly one `?` marker",
                    );
                    return None;
                }
                TypeSyntax {
                    kind: TypeKind::Optional {
                        payload: OptionalPayloadKind::Named(name),
                        payload_span: name_span,
                        question_span: question.span,
                    },
                    span: self.cover(name_span, question.span),
                }
            } else {
                TypeSyntax {
                    kind: TypeKind::Named(name),
                    span: name_span,
                }
            }
        } else if let Some(left_paren) = self.consume(TokenKind::LeftParen) {
            let inner = self.with_syntax_nesting(left_paren.span, |parser| {
                parser.parse_type_inner(
                    TypeContext::ArrayElement,
                    "expected a type inside the grouping".to_owned(),
                )
            })?;
            let right_paren = self.expect(TokenKind::RightParen, "`)` after the grouped type")?;
            TypeSyntax {
                span: self.cover(left_paren.span, right_paren.span),
                kind: TypeKind::Grouped {
                    left_paren_span: left_paren.span,
                    inner: Box::new(inner),
                    right_paren_span: right_paren.span,
                },
            }
        } else {
            self.report(
                EXPECTED_TOKEN,
                message,
                token.span,
                context.expected_label(),
            );
            if token.kind == TokenKind::Identifier || token_type_kind(token.kind).is_some() {
                self.advance();
            }
            return None;
        };

        let mut array_depth = 0usize;
        while self.at(TokenKind::LeftBracket) {
            if self.peek_ahead(1).kind != TokenKind::RightBracket {
                self.report(
                    EXPECTED_TOKEN,
                    "expected `]` in the array type suffix",
                    self.peek_ahead(1).span,
                    "array types use the exact postfix spelling `[]`",
                );
                self.advance();
                return None;
            }
            if self.nesting_depth + array_depth + 1 >= MAX_SYNTAX_NESTING {
                self.report_excessive_nesting(self.peek().span);
                self.recover_from_excessive_nesting();
                return None;
            }
            array_depth += 1;
            let left_bracket = self.advance();
            let right_bracket = self.advance();
            type_syntax = TypeSyntax {
                span: self.cover(type_syntax.span, right_bracket.span),
                kind: TypeKind::Array {
                    element: Box::new(type_syntax),
                    left_bracket_span: left_bracket.span,
                    right_bracket_span: right_bracket.span,
                },
            };
        }

        if matches!(type_syntax.kind, TypeKind::Grouped { .. }) {
            self.report(
                EXPECTED_TOKEN,
                "grouped types are supported only as array element types",
                type_syntax.span,
                "follow the grouping with `[]`",
            );
            return None;
        }
        if self.at(TokenKind::Question) && matches!(type_syntax.kind, TypeKind::Array { .. }) {
            self.reject_optional_suffix(
                "inline optional array payloads are not supported",
                "use `shared? T[]` for an optional shared array owner",
            );
            return None;
        }
        Some(type_syntax)
    }

    fn at_any_ahead(&self, distance: usize, kinds: &[TokenKind]) -> bool {
        kinds
            .iter()
            .any(|kind| self.peek_ahead(distance).kind == *kind)
    }

    fn reject_optional_suffix(&mut self, message: &'static str, label: &'static str) {
        let question = self.advance();
        self.consume_repeated_questions();
        self.report(INVALID_OPTIONAL_TYPE, message, question.span, label);
    }

    fn consume_repeated_questions(&mut self) {
        while self.consume(TokenKind::Question).is_some() {}
    }
}

fn optional_payload_kind(kind: TypeKind) -> Option<OptionalPayloadKind> {
    match kind {
        TypeKind::I64 => Some(OptionalPayloadKind::I64),
        TypeKind::U64 => Some(OptionalPayloadKind::U64),
        TypeKind::U8 => Some(OptionalPayloadKind::U8),
        TypeKind::F64 => Some(OptionalPayloadKind::F64),
        TypeKind::Bool => Some(OptionalPayloadKind::Bool),
        TypeKind::Unit
        | TypeKind::Named(_)
        | TypeKind::Shared { .. }
        | TypeKind::Optional { .. }
        | TypeKind::OptionalShared { .. }
        | TypeKind::Grouped { .. }
        | TypeKind::Array { .. } => None,
    }
}

fn token_type_kind(kind: TokenKind) -> Option<TypeKind> {
    match kind {
        TokenKind::I64 => Some(TypeKind::I64),
        TokenKind::U64 => Some(TypeKind::U64),
        TokenKind::U8 => Some(TypeKind::U8),
        TokenKind::F64 => Some(TypeKind::F64),
        TokenKind::Bool => Some(TypeKind::Bool),
        TokenKind::Unit => Some(TypeKind::Unit),
        _ => None,
    }
}
