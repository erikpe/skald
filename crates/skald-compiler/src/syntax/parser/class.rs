//! Restricted class and instance-member declarations.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_class(&mut self) -> Option<ClassDecl> {
        let class_token = self.advance();
        let name = self.parse_name("expected a class name after `class`");
        let left_brace = self.expect(TokenKind::LeftBrace, "`{` after the class name")?;
        self.brace_depth += 1;
        self.class_depth += 1;
        let body = self.with_syntax_nesting(left_brace.span, |parser| parser.parse_class_body());
        self.class_depth -= 1;
        self.brace_depth -= 1;
        let (members, right_brace) = body?;
        let name = name?;

        Some(ClassDecl {
            name,
            members,
            span: self.cover(class_token.span, right_brace.span),
        })
    }

    fn parse_class_body(&mut self) -> Option<(Vec<ClassMember>, Token)> {
        let mut members = Vec::new();
        while !self.at_any(&[TokenKind::RightBrace, TokenKind::Eof]) {
            if self.recovering_from_excessive_nesting {
                return None;
            }
            if self.at_any(&[TokenKind::Class, TokenKind::Extern]) {
                break;
            }
            if self.at(TokenKind::Invalid) {
                self.advance();
                continue;
            }

            let before = self.current;
            if let Some(member) = self.parse_class_member() {
                members.push(member);
            } else if self.current == before || !self.starts_class_member() {
                self.synchronize_class_member();
            }
            if self.current == before {
                self.advance();
            }
        }

        let right_brace = self.expect(TokenKind::RightBrace, "`}` after the class body")?;
        Some((members, right_brace))
    }

    fn starts_class_member(&self) -> bool {
        self.at_any(&[TokenKind::Fn, TokenKind::Mut, TokenKind::Ref])
            || (self.at(TokenKind::Identifier)
                && (self.peek_ahead(1).kind == TokenKind::Colon
                    || self.lexeme(self.peek()) == "destroy"
                    || (self.lexeme(self.peek()) == "assign"
                        && self.peek_ahead(1).kind == TokenKind::LeftParen)
                    || (self.lexeme(self.peek()) == "init"
                        && self.peek_ahead(1).kind == TokenKind::LeftParen)))
    }

    fn parse_class_member(&mut self) -> Option<ClassMember> {
        if self.at(TokenKind::Ref)
            || (self.at(TokenKind::Mut) && self.peek_ahead(1).kind == TokenKind::Ref)
        {
            self.report(
                INVALID_CLASS_MEMBER,
                "alias bindings are not valid class fields",
                self.peek().span,
                "`ref` and `mut ref` are supported only on parameters",
            );
            return None;
        }

        if self.at(TokenKind::Mut)
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && matches!(self.lexeme(self.peek_ahead(1)), "assign" | "destroy")
        {
            let modifier = self.advance();
            let lifecycle = self.lexeme(self.peek()).to_owned();
            self.report(
                INVALID_CLASS_MEMBER,
                format!("{lifecycle} members do not use `mut`"),
                modifier.span,
                format!("`{lifecycle}` already has an implicit mutable receiver"),
            );
            if lifecycle == "assign" {
                self.parse_copy_assignment();
            } else {
                self.parse_destructor();
            }
            return None;
        }

        if self.at(TokenKind::Fn) || self.at(TokenKind::Mut) {
            return self.parse_method().map(ClassMember::Method);
        }

        if self.at(TokenKind::Identifier) {
            let text = self.lexeme(self.peek());
            if text == "init" && self.peek_ahead(1).kind == TokenKind::LeftParen {
                return self.parse_initializer().map(ClassMember::Initializer);
            }
            if self.peek_ahead(1).kind == TokenKind::Colon {
                return self.parse_field().map(ClassMember::Field);
            }
            if text == "assign" && self.peek_ahead(1).kind == TokenKind::LeftParen {
                return self
                    .parse_copy_assignment()
                    .map(ClassMember::CopyAssignment);
            }
            if text == "destroy" {
                return self.parse_destructor().map(ClassMember::Destructor);
            }

            let span = self.peek().span;
            let message = match text {
                "assign" => "malformed copy-assignment declaration",
                "init" => "malformed initializer declaration",
                _ => "expected a field, initializer, destructor, or method declaration",
            };
            self.report(
                INVALID_CLASS_MEMBER,
                message,
                span,
                "class members use `name: type;`, `init(...) { ... }`, `assign(ref name: Class) { ... }`, `destroy { ... }`, or `[mut] fn name(...) -> type { ... }`",
            );
            return None;
        }

        self.report(
            INVALID_CLASS_MEMBER,
            "expected a class member declaration",
            self.peek().span,
            "expected a field, initializer, destructor, or method",
        );
        None
    }

    fn parse_field(&mut self) -> Option<FieldDecl> {
        let name = self.parse_name("expected a field name")?;
        self.expect(TokenKind::Colon, "`:` after the field name");
        let type_syntax = self.parse_type(
            TypeContext::Field,
            format!(
                "expected a field type {} or a class name",
                format_type_list(STORED_TYPE_NAMES)
            ),
        )?;
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the field declaration");
        let end_span = semicolon.map_or(type_syntax.span, |token| token.span);
        Some(FieldDecl {
            span: self.cover(name.span, end_span),
            name,
            type_syntax,
        })
    }

    fn parse_initializer(&mut self) -> Option<InitializerDecl> {
        let introducer = self.advance();
        debug_assert_eq!(self.lexeme(introducer), "init");
        let parameters = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Some(InitializerDecl {
            introducer_span: introducer.span,
            parameters,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_copy_assignment(&mut self) -> Option<CopyAssignmentDecl> {
        let introducer = self.advance();
        debug_assert_eq!(self.lexeme(introducer), "assign");
        let parameters = self.parse_parameter_list()?;
        let mut valid = true;

        if let Some(arrow) = self.consume(TokenKind::Arrow) {
            self.report(
                INVALID_CLASS_MEMBER,
                "copy-assignment members do not declare a result type",
                arrow.span,
                "`assign` returns `unit` implicitly",
            );
            self.parse_type(
                TypeContext::Result,
                "expected a type after the invalid copy-assignment result arrow",
            );
            valid = false;
        }

        if let Some(semicolon) = self.consume(TokenKind::Semicolon) {
            self.report(
                INVALID_CLASS_MEMBER,
                "copy-assignment members require a body",
                semicolon.span,
                "replace `;` with `{ ... }`",
            );
            return None;
        }

        let body = self.parse_block()?;
        valid.then_some(CopyAssignmentDecl {
            introducer_span: introducer.span,
            parameters,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_destructor(&mut self) -> Option<DestructorDecl> {
        let introducer = self.advance();
        debug_assert_eq!(self.lexeme(introducer), "destroy");
        let mut valid = true;

        if self.at(TokenKind::LeftParen) {
            let parameters_span = self.peek().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "destruction members do not have a parameter list",
                parameters_span,
                "remove the parentheses and parameters",
            );
            self.parse_parameter_list();
            valid = false;
        }

        if let Some(arrow) = self.consume(TokenKind::Arrow) {
            self.report(
                INVALID_CLASS_MEMBER,
                "destruction members do not declare a result type",
                arrow.span,
                "`destroy` returns `unit` implicitly",
            );
            self.parse_type(
                TypeContext::Result,
                "expected a type after the invalid destruction result arrow",
            );
            valid = false;
        }

        if let Some(semicolon) = self.consume(TokenKind::Semicolon) {
            self.report(
                INVALID_CLASS_MEMBER,
                "destruction members require a body",
                semicolon.span,
                "replace `;` with `{ ... }`",
            );
            return None;
        }

        if !self.at(TokenKind::LeftBrace) {
            self.report(
                INVALID_CLASS_MEMBER,
                "malformed destruction declaration",
                self.peek().span,
                "expected `{` directly after `destroy`",
            );
            return None;
        }

        let body = self.parse_block()?;
        valid.then_some(DestructorDecl {
            introducer_span: introducer.span,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_method(&mut self) -> Option<MethodDecl> {
        let mut_token = self.consume(TokenKind::Mut);
        let start_span = mut_token.map_or_else(|| self.peek().span, |token| token.span);
        self.expect(TokenKind::Fn, "`fn` after `mut` in a method declaration")?;
        let name = self.parse_name("expected a method name after `fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the method parameter list");
        let return_type = self.parse_type(
            TypeContext::Result,
            "expected a method return type after `->`",
        );
        let body = self.parse_block();
        let (name, parameters, return_type, body) = match (name, parameters, return_type, body) {
            (Some(name), Some(parameters), Some(return_type), Some(body)) => {
                (name, parameters, return_type, body)
            }
            _ => return None,
        };
        Some(MethodDecl {
            mut_span: mut_token.map(|token| token.span),
            name,
            parameters,
            return_type,
            span: self.cover(start_span, body.span),
            body,
        })
    }
}
