//! Top-level declarations, parameters, and source type syntax.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TypeContext {
    Result,
    ValueParameter,
    AliasParameter,
    LocalValue,
    Field,
}

impl TypeContext {
    const fn accepts_unit(self) -> bool {
        matches!(self, Self::Result)
    }

    const fn accepts_named(self) -> bool {
        matches!(
            self,
            Self::ValueParameter | Self::AliasParameter | Self::LocalValue | Self::Field
        )
    }

    const fn accepts_primitive(self) -> bool {
        !matches!(self, Self::AliasParameter)
    }

    fn expected_label(self) -> String {
        match self {
            Self::Result => format!("expected {}", format_type_list(RESULT_TYPE_NAMES)),
            Self::ValueParameter => format!(
                "value parameters must have type {} or a named class type",
                format_type_list(STORED_TYPE_NAMES)
            ),
            Self::AliasParameter => "alias parameters must name an inline class type".to_owned(),
            Self::LocalValue => format!(
                "locals must have type {} or a named class type",
                format_type_list(STORED_TYPE_NAMES)
            ),
            Self::Field => format!(
                "fields must have type {} or a named class type",
                format_type_list(STORED_TYPE_NAMES)
            ),
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_function(&mut self) -> Option<FunctionDecl> {
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
        let span = self.cover(fn_token.span, body.span);

        Some(FunctionDecl {
            name,
            parameters,
            return_type,
            body,
            span,
        })
    }

    pub(super) fn parse_external_function(&mut self) -> Option<ExternalFunctionDecl> {
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
            name,
            parameters,
            return_type,
            span: self.cover(extern_token.span, end_span),
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
                "expected a class name as the alias parameter type".to_owned()
            } else {
                format!(
                    "expected the parameter type {} or a named class type",
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
        let token = self.peek();
        if let Some(kind) = token_type_kind(token.kind) {
            if context.accepts_primitive() && (kind != TypeKind::Unit || context.accepts_unit()) {
                self.advance();
                return Some(TypeSyntax {
                    kind,
                    span: token.span,
                });
            }
        }

        if context.accepts_named() && token.kind == TokenKind::Identifier {
            self.advance();
            return Some(TypeSyntax {
                kind: TypeKind::Named(Name {
                    text: self.lexeme(token).to_owned(),
                    span: token.span,
                }),
                span: token.span,
            });
        }

        self.report(
            EXPECTED_TOKEN,
            message,
            token.span,
            context.expected_label(),
        );
        if token.kind == TokenKind::Identifier || token_type_kind(token.kind).is_some() {
            self.advance();
        }
        None
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
