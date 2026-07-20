//! Top-level declarations, parameters, and source type syntax.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TypeContext {
    Result,
    StoredValue,
}

impl TypeContext {
    const fn accepts_unit(self) -> bool {
        matches!(self, Self::Result)
    }

    const fn expected_label(self) -> &'static str {
        match self {
            Self::Result => "expected `i64`, `u64`, `u8`, `f64`, `bool`, or `unit`",
            Self::StoredValue => {
                "parameters and locals must have type `i64`, `u64`, `u8`, `f64`, or `bool`"
            }
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

    fn parse_parameter_list(&mut self) -> Option<Vec<Parameter>> {
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

            if self.at(TokenKind::Identifier) {
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
        let name = self.parse_name("expected a parameter name");
        self.expect(TokenKind::Colon, "`:` after the parameter name");
        let type_syntax = self.parse_type(
            TypeContext::StoredValue,
            "expected the parameter type `i64`, `u64`, `u8`, `f64`, or `bool`",
        );

        match (name, type_syntax) {
            (Some(name), Some(type_syntax)) => {
                let span = self.cover(name.span, type_syntax.span);
                Some(Parameter {
                    name,
                    type_syntax,
                    span,
                })
            }
            _ => None,
        }
    }

    pub(super) fn parse_type(
        &mut self,
        context: TypeContext,
        message: &'static str,
    ) -> Option<TypeSyntax> {
        let token = self.peek();
        if let Some(kind) = token_type_kind(token.kind) {
            if kind != TypeKind::Unit || context.accepts_unit() {
                self.advance();
                return Some(TypeSyntax {
                    kind,
                    span: token.span,
                });
            }
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
