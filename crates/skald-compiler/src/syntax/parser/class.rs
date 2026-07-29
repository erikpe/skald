//! Class and instance-member declaration grammar.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_class(&mut self, visibility: Visibility) -> Option<ClassDecl> {
        let class_token = self.advance();
        let name = self.parse_name("expected a class name after `class`");
        let direct_base = self.parse_direct_base();
        self.discard_duplicate_base_clauses();
        let implemented_interfaces = self.parse_implemented_interfaces();
        let left_brace = self.expect(TokenKind::LeftBrace, "`{` after the class header")?;
        self.brace_depth += 1;
        self.class_depth += 1;
        let body = self.with_syntax_nesting(left_brace.span, |parser| parser.parse_class_body());
        self.class_depth -= 1;
        self.brace_depth -= 1;
        let (members, right_brace) = body?;
        let name = name?;

        Some(ClassDecl {
            visibility,
            name,
            direct_base,
            implemented_interfaces,
            members,
            span: self.cover(visibility.start_span(class_token.span), right_brace.span),
        })
    }

    fn parse_implemented_interfaces(&mut self) -> Vec<Name> {
        if !self.at_contextual("implements") {
            return Vec::new();
        }
        self.advance();
        let mut interfaces = Vec::new();
        while let Some(name) = self.parse_name_path("expected an interface name after `implements`")
        {
            interfaces.push(name);
            if self.consume(TokenKind::Comma).is_none() {
                break;
            }
        }
        interfaces
    }

    fn parse_direct_base(&mut self) -> Option<Name> {
        if !self.at_contextual("extends") {
            return None;
        }

        self.advance();
        self.parse_name_path("expected a base class name after `extends`")
    }

    fn discard_duplicate_base_clauses(&mut self) {
        while self.at_contextual("extends") {
            let extends = self.advance();
            self.report(
                INVALID_CLASS_HEADER,
                "a class cannot declare more than one direct base",
                extends.span,
                "remove this duplicate `extends` clause",
            );
            if self.at(TokenKind::Identifier) {
                self.advance();
            } else {
                self.report(
                    INVALID_CLASS_HEADER,
                    "expected a base class name after `extends`",
                    self.peek().span,
                    "a base clause requires a class name",
                );
                break;
            }
        }
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
                && (self.starts_member_visibility()
                    || self.starts_static_member()
                    || self.starts_method_modifier()
                    || self.peek_ahead(1).kind == TokenKind::Colon
                    || self.lexeme(self.peek()) == "destroy"
                    || (matches!(self.lexeme(self.peek()), "copy" | "assign")
                        && self.peek_ahead(1).kind == TokenKind::LeftParen)
                    || (self.lexeme(self.peek()) == "init"
                        && self.peek_ahead(1).kind == TokenKind::LeftParen)))
    }

    fn parse_class_member(&mut self) -> Option<ClassMember> {
        let visibility = self.parse_member_visibility();

        if self.at_contextual("static") && self.peek_ahead(1).kind != TokenKind::Colon {
            let static_token = self.advance();
            if self.peek_ahead(1).kind == TokenKind::Colon {
                self.report(
                    INVALID_CLASS_MEMBER,
                    "static fields are not supported",
                    static_token.span,
                    "static storage is a separate future feature",
                );
                self.parse_field(MemberVisibility::Public);
                return None;
            }
            if self.at(TokenKind::Identifier)
                && matches!(
                    self.lexeme(self.peek()),
                    "init" | "copy" | "assign" | "destroy"
                )
            {
                let lifecycle = self.lexeme(self.peek()).to_owned();
                self.report(
                    INVALID_CLASS_MEMBER,
                    format!("{lifecycle} members cannot be static"),
                    static_token.span,
                    "lifecycle members always operate on an object receiver",
                );
                match lifecycle.as_str() {
                    "init" => {
                        self.parse_initializer();
                    }
                    "copy" => {
                        self.parse_copy_constructor();
                    }
                    "assign" => {
                        self.parse_copy_assignment();
                    }
                    "destroy" => {
                        self.parse_destructor();
                    }
                    _ => unreachable!("guarded lifecycle spelling"),
                }
                return None;
            }
            return self
                .parse_method(visibility, Some(static_token.span))
                .map(ClassMember::Method);
        }

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
            && matches!(
                self.lexeme(self.peek_ahead(1)),
                "copy" | "assign" | "destroy"
            )
        {
            let modifier = self.advance();
            let lifecycle = self.lexeme(self.peek()).to_owned();
            self.report(
                INVALID_CLASS_MEMBER,
                format!("{lifecycle} members do not use `mut`"),
                modifier.span,
                format!("`{lifecycle}` already has an implicit mutable receiver"),
            );
            match lifecycle.as_str() {
                "copy" => {
                    self.parse_copy_constructor();
                }
                "assign" => {
                    self.parse_copy_assignment();
                }
                "destroy" => {
                    self.parse_destructor();
                }
                _ => unreachable!("guarded lifecycle spelling"),
            }
            return None;
        }

        if self.at(TokenKind::Fn) || self.at(TokenKind::Mut) || self.starts_method_modifier() {
            return self.parse_method(visibility, None).map(ClassMember::Method);
        }

        if self.at(TokenKind::Identifier) {
            let text = self.lexeme(self.peek());
            if text == "init" && self.peek_ahead(1).kind == TokenKind::LeftParen {
                if let MemberVisibility::Private { span } = visibility {
                    self.report_private_lifecycle(span, "initializers");
                    self.parse_initializer();
                    return None;
                }
                return self.parse_initializer().map(ClassMember::Initializer);
            }
            if text == "copy" && self.peek_ahead(1).kind == TokenKind::LeftParen {
                if let MemberVisibility::Private { span } = visibility {
                    self.report_private_lifecycle(span, "copy constructors");
                    self.parse_copy_constructor();
                    return None;
                }
                return self
                    .parse_copy_constructor()
                    .map(ClassMember::CopyConstructor);
            }
            if self.peek_ahead(1).kind == TokenKind::Colon {
                return self.parse_field(visibility).map(ClassMember::Field);
            }
            if text == "assign" && self.peek_ahead(1).kind == TokenKind::LeftParen {
                if let MemberVisibility::Private { span } = visibility {
                    self.report_private_lifecycle(span, "copy assignments");
                    self.parse_copy_assignment();
                    return None;
                }
                return self
                    .parse_copy_assignment()
                    .map(ClassMember::CopyAssignment);
            }
            if text == "destroy" {
                if let MemberVisibility::Private { span } = visibility {
                    self.report_private_lifecycle(span, "destructors");
                    self.parse_destructor();
                    return None;
                }
                return self.parse_destructor().map(ClassMember::Destructor);
            }

            let span = self.peek().span;
            let message = match text {
                "assign" => "malformed copy-assignment declaration",
                "copy" => "malformed copy-constructor declaration",
                "init" => "malformed initializer declaration",
                _ => "expected a field, initializer, destructor, or method declaration",
            };
            if matches!(text, "assign" | "copy" | "init") {
                self.advance();
            }
            self.report(
                INVALID_CLASS_MEMBER,
                message,
                span,
                "class members use `name: type;`, `init(...) { ... }`, `copy(ref name: Class) { ... }`, `assign(ref name: Class) { ... }`, `destroy { ... }`, or `[virtual|override] [mut] fn name(...) -> type { ... }`",
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

    fn parse_member_visibility(&mut self) -> MemberVisibility {
        if !self.starts_member_visibility() {
            return MemberVisibility::Public;
        }

        let span = self.advance().span;
        while self.starts_member_visibility() && self.at_contextual("private") {
            let duplicate = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "a class member cannot repeat `private`",
                duplicate.span,
                "remove this duplicate visibility modifier",
            );
        }
        MemberVisibility::Private { span }
    }

    fn starts_member_visibility(&self) -> bool {
        if !self.at_contextual("private") {
            return false;
        }

        let next = self.peek_ahead(1);
        matches!(next.kind, TokenKind::Fn | TokenKind::Mut)
            || (next.kind == TokenKind::Identifier
                && (matches!(
                    self.lexeme(next),
                    "private"
                        | "static"
                        | "virtual"
                        | "override"
                        | "init"
                        | "copy"
                        | "assign"
                        | "destroy"
                ) || self.peek_ahead(2).kind == TokenKind::Colon))
    }

    fn starts_static_member(&self) -> bool {
        if !self.at_contextual("static") {
            return false;
        }
        let next = self.peek_ahead(1);
        matches!(next.kind, TokenKind::Fn | TokenKind::Mut)
            || (next.kind == TokenKind::Identifier
                && (matches!(
                    self.lexeme(next),
                    "static" | "virtual" | "override" | "init" | "copy" | "assign" | "destroy"
                ) || self.peek_ahead(2).kind == TokenKind::Colon))
    }

    fn report_private_lifecycle(&mut self, span: Span, member: &str) {
        self.report(
            INVALID_CLASS_MEMBER,
            format!("{member} cannot be private"),
            span,
            "lifecycle members do not declare visibility",
        );
    }

    fn parse_field(&mut self, visibility: MemberVisibility) -> Option<FieldDecl> {
        let name = self.parse_name("expected a field name")?;
        self.expect(TokenKind::Colon, "`:` after the field name");
        let type_syntax = self.parse_type(
            TypeContext::Field,
            format!(
                "expected a field type {}, a class name, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
        )?;
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the field declaration");
        let end_span = semicolon.map_or(type_syntax.span, |token| token.span);
        Some(FieldDecl {
            span: self.cover(visibility.start_span(name.span), end_span),
            visibility,
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
            visibility: MemberVisibility::Public,
            introducer_span: introducer.span,
            parameters,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_copy_assignment(&mut self) -> Option<CopyAssignmentDecl> {
        let (introducer, parameters, body) =
            self.parse_parameterized_lifecycle_member("assign", "copy-assignment")?;
        Some(CopyAssignmentDecl {
            introducer_span: introducer.span,
            parameters,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_copy_constructor(&mut self) -> Option<CopyConstructorDecl> {
        let (introducer, parameters, body) =
            self.parse_parameterized_lifecycle_member("copy", "copy-constructor")?;
        Some(CopyConstructorDecl {
            introducer_span: introducer.span,
            parameters,
            span: self.cover(introducer.span, body.span),
            body,
        })
    }

    fn parse_parameterized_lifecycle_member(
        &mut self,
        introducer_name: &str,
        description: &str,
    ) -> Option<(Token, Vec<Parameter>, Block)> {
        let introducer = self.advance();
        debug_assert_eq!(self.lexeme(introducer), introducer_name);
        let parameters = self.parse_parameter_list()?;
        let mut valid = true;

        if let Some(arrow) = self.consume(TokenKind::Arrow) {
            self.report(
                INVALID_CLASS_MEMBER,
                format!("{description} members do not declare a result type"),
                arrow.span,
                format!("`{introducer_name}` returns `unit` implicitly"),
            );
            self.parse_type(
                TypeContext::Result,
                format!("expected a type after the invalid {description} result arrow"),
            );
            valid = false;
        }

        if let Some(semicolon) = self.consume(TokenKind::Semicolon) {
            self.report(
                INVALID_CLASS_MEMBER,
                format!("{description} members require a body"),
                semicolon.span,
                "replace `;` with `{ ... }`",
            );
            return None;
        }

        let body = self.parse_block()?;
        valid.then_some((introducer, parameters, body))
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

    fn parse_method(
        &mut self,
        visibility: MemberVisibility,
        static_span: Option<Span>,
    ) -> Option<MethodDecl> {
        let modifier = self.parse_method_modifier();
        let mut valid = true;
        if let (Some(static_span), Some(modifier)) = (static_span, modifier) {
            self.report(
                INVALID_CLASS_MEMBER,
                "static methods cannot be `virtual` or `override`",
                modifier.span(),
                "static methods use receiverless direct dispatch",
            );
            let _ = static_span;
            valid = false;
        }
        if let (MemberVisibility::Private { .. }, Some(modifier)) = (visibility, modifier) {
            self.report(
                INVALID_CLASS_MEMBER,
                "private methods cannot be `virtual` or `override`",
                modifier.span(),
                "private methods use direct dispatch",
            );
        }
        let mut_token = self.consume(TokenKind::Mut);
        if let (Some(static_span), Some(mut_token)) = (static_span, mut_token) {
            self.report(
                INVALID_CLASS_MEMBER,
                "static methods cannot use `mut`",
                mut_token.span,
                "static methods have no receiver to mutate",
            );
            let _ = static_span;
            valid = false;
        }
        if self.at_contextual("static") {
            let misplaced = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                if static_span.is_some() {
                    "a static method cannot repeat `static`"
                } else {
                    "`static` must precede method dispatch modifiers and `mut`"
                },
                misplaced.span,
                if static_span.is_some() {
                    "remove this duplicate method modifier"
                } else {
                    "write `static fn`"
                },
            );
            valid = false;
        }
        if self.at_contextual("private") {
            let misplaced = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "`private` must precede method modifiers and `mut`",
                misplaced.span,
                "write `private mut fn`",
            );
        }
        if mut_token.is_some() && (self.at_contextual("virtual") || self.at_contextual("override"))
        {
            let misplaced = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "method dispatch modifiers must precede `mut`",
                misplaced.span,
                "write `virtual mut fn` or `override mut fn`",
            );
        }
        let method_start = modifier
            .map(MethodModifier::span)
            .or_else(|| mut_token.map(|token| token.span))
            .or(static_span)
            .unwrap_or_else(|| self.peek().span);
        let start_span = visibility.start_span(method_start);
        let expectation = if modifier.is_some() || mut_token.is_some() {
            "`fn` after method modifiers"
        } else {
            "`fn` in a method declaration"
        };
        self.expect(TokenKind::Fn, expectation)?;
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
        valid.then_some(MethodDecl {
            visibility,
            static_span,
            modifier,
            mut_span: mut_token.map(|token| token.span),
            name,
            parameters,
            return_type,
            span: self.cover(start_span, body.span),
            body,
        })
    }

    fn parse_method_modifier(&mut self) -> Option<MethodModifier> {
        let modifier = if self.at_contextual("virtual") {
            Some(MethodModifier::Virtual {
                span: self.advance().span,
            })
        } else if self.at_contextual("override") {
            Some(MethodModifier::Override {
                span: self.advance().span,
            })
        } else {
            None
        };

        while self.at_contextual("virtual") || self.at_contextual("override") {
            let duplicate = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "a method cannot combine or repeat dispatch modifiers",
                duplicate.span,
                "use exactly one of `virtual` or `override`",
            );
        }
        modifier
    }

    fn starts_method_modifier(&self) -> bool {
        if !(self.at_contextual("virtual") || self.at_contextual("override")) {
            return false;
        }
        let next = self.peek_ahead(1);
        matches!(next.kind, TokenKind::Fn | TokenKind::Mut)
            || (next.kind == TokenKind::Identifier
                && matches!(
                    self.lexeme(next),
                    "private" | "static" | "virtual" | "override"
                ))
    }
}
