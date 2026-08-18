//! Interface requirement declaration grammar.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_interface(&mut self, visibility: Visibility) -> Option<InterfaceDecl> {
        let interface_token = self.advance();
        let name = self.parse_name("expected an interface name after `interface`");
        let type_parameters = if self.at(TokenKind::Less) {
            self.parse_generic_parameter_list()
        } else {
            None
        };
        let where_clause = if self.at_contextual("where") {
            let where_span = self.peek().span;
            let clause = self.parse_generic_where_clause();
            if type_parameters.is_none() {
                self.report(
                    INVALID_GENERIC_SYNTAX,
                    "a `where` clause requires generic interface parameters",
                    where_span,
                    "add type parameters to the interface or remove this clause",
                );
            }
            clause
        } else {
            None
        };
        if self.at_contextual("where") {
            let duplicate = self.advance();
            self.report(
                INVALID_GENERIC_SYNTAX,
                "an interface cannot declare more than one `where` clause",
                duplicate.span,
                "merge the requirements into the first clause",
            );
            self.synchronize_declaration();
            return None;
        }
        let left = self.expect(TokenKind::LeftBrace, "`{` after the interface header")?;
        self.brace_depth += 1;
        let requirements = self.with_syntax_nesting(left.span, |parser| {
            let mut requirements = Vec::new();
            while !parser.at_any(&[TokenKind::RightBrace, TokenKind::Eof]) {
                let start = parser.current;
                if let Some(requirement) = parser.parse_interface_requirement() {
                    requirements.push(requirement);
                } else {
                    parser.synchronize_interface_member();
                }
                if parser.current == start {
                    parser.advance();
                }
            }
            Some(requirements)
        });
        self.brace_depth -= 1;
        let right = self.expect(TokenKind::RightBrace, "`}` after the interface body")?;
        Some(InterfaceDecl {
            visibility,
            name: name?,
            type_parameters,
            where_clause,
            requirements: requirements?,
            span: self.cover(visibility.start_span(interface_token.span), right.span),
        })
    }

    fn parse_interface_requirement(&mut self) -> Option<InterfaceRequirementDecl> {
        let mut_span = self.consume(TokenKind::Mut).map(|token| token.span);
        let Some(fn_token) = self.consume(TokenKind::Fn) else {
            self.report(
                INVALID_INTERFACE_MEMBER,
                "interfaces contain only method requirements",
                self.peek().span,
                "expected `fn` or `mut fn`",
            );
            return None;
        };
        let name = self.parse_name("expected a requirement name after `fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type(TypeContext::Result, "expected a return type after `->`");
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the interface requirement");
        Some(InterfaceRequirementDecl {
            mut_span,
            name: name?,
            parameters: parameters?,
            return_type: return_type?,
            span: self.cover(mut_span.unwrap_or(fn_token.span), semicolon?.span),
        })
    }

    pub(super) fn synchronize_interface_member(&mut self) {
        while !self.at_any(&[TokenKind::Semicolon, TokenKind::RightBrace, TokenKind::Eof]) {
            self.advance();
        }
        self.consume(TokenKind::Semicolon);
    }
}
