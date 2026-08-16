//! Contextual field-modifier classification and recovery.

use super::*;

pub(super) struct ParsedClassField {
    pub(super) recognized: bool,
    pub(super) member: Option<ClassMember>,
}

impl ParsedClassField {
    fn not_field() -> Self {
        Self {
            recognized: false,
            member: None,
        }
    }

    fn parsed(member: Option<ClassMember>) -> Self {
        Self {
            recognized: true,
            member,
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_class_field(&mut self, visibility: MemberVisibility) -> ParsedClassField {
        if self.at(TokenKind::Identifier) && self.peek_ahead(1).kind == TokenKind::Colon {
            return ParsedClassField::parsed(
                self.parse_field(visibility, None, None)
                    .map(ClassMember::Field),
            );
        }
        if self.starts_cell_field_modifier() {
            return ParsedClassField::parsed(self.parse_cell_field(visibility));
        }
        if self.starts_final_field_modifier() {
            return ParsedClassField::parsed(self.parse_final_field(visibility));
        }
        if self.at_contextual("static") && self.peek_ahead(1).kind != TokenKind::Colon {
            return self.parse_static_field_modifier(visibility);
        }
        ParsedClassField::not_field()
    }

    pub(super) fn starts_cell_field_modifier(&self) -> bool {
        self.at_contextual("cell") && self.peek_ahead(1).kind != TokenKind::Colon
    }

    pub(super) fn starts_final_field_modifier(&self) -> bool {
        self.at_contextual("final") && self.peek_ahead(1).kind != TokenKind::Colon
    }

    fn parse_cell_field(&mut self, visibility: MemberVisibility) -> Option<ClassMember> {
        let cell_span = self.advance().span;
        let mut valid = matches!(visibility, MemberVisibility::Private { .. });

        if self.at_contextual("private")
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.peek_ahead(2).kind == TokenKind::Colon
        {
            let private_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "`private` must precede `cell`",
                private_span,
                "write `private cell name: type;`",
            );
            valid = false;
        } else if !valid {
            self.report(
                INVALID_CLASS_MEMBER,
                "cell fields must be private",
                cell_span,
                "write `private cell name: type;`",
            );
        }

        while self.starts_cell_field_modifier() {
            let duplicate = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "a field cannot repeat `cell`",
                duplicate.span,
                "remove this duplicate cell modifier",
            );
            valid = false;
        }

        if self.at_contextual("final")
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.peek_ahead(2).kind == TokenKind::Colon
        {
            let final_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "a field cannot be both `cell` and `final`",
                final_span,
                "choose interior mutability or final storage",
            );
            let _ = self.parse_field(visibility, Some(cell_span), Some(final_span));
            return None;
        }

        if self.at_contextual("static")
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.peek_ahead(2).kind == TokenKind::Colon
        {
            let static_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "cell fields cannot be static",
                static_span,
                "remove `static` or declare an ordinary static field",
            );
            let _ = self.parse_field(visibility, Some(cell_span), None);
            return None;
        }

        if !self.at(TokenKind::Identifier) || self.peek_ahead(1).kind != TokenKind::Colon {
            self.report(
                INVALID_CLASS_MEMBER,
                "`cell` modifies only an instance field",
                cell_span,
                "expected `private cell name: type;`",
            );
            return None;
        }

        let field = self.parse_field(visibility, Some(cell_span), None)?;
        valid.then_some(ClassMember::Field(field))
    }

    fn parse_final_field(&mut self, visibility: MemberVisibility) -> Option<ClassMember> {
        let final_span = self.advance().span;
        let mut valid = true;

        if self.at_contextual("private")
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && (self.peek_ahead(2).kind == TokenKind::Colon
                || (self.lexeme(self.peek_ahead(1)) == "static"
                    && self.peek_ahead(2).kind == TokenKind::Identifier
                    && self.peek_ahead(3).kind == TokenKind::Colon))
        {
            let private_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "`private` must precede `final`",
                private_span,
                "write `private final name: type;`",
            );
            valid = false;
        }

        while self.starts_final_field_modifier() {
            let duplicate = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "a field cannot repeat `final`",
                duplicate.span,
                "remove this duplicate final modifier",
            );
            valid = false;
        }

        if self.starts_cell_field_modifier()
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.peek_ahead(2).kind == TokenKind::Colon
        {
            let cell_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "a field cannot be both `final` and `cell`",
                cell_span,
                "choose final storage or interior mutability",
            );
            let _ = self.parse_field(visibility, Some(cell_span), Some(final_span));
            return None;
        }

        if self.at_contextual("static")
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.peek_ahead(2).kind == TokenKind::Colon
        {
            let static_span = self.advance().span;
            let field = self.parse_static_field(visibility, static_span, Some(final_span))?;
            return valid.then_some(ClassMember::StaticField(field));
        }

        if self.at_contextual("static")
            && self.lexeme(self.peek_ahead(1)) == "static"
            && self.peek_ahead(2).kind == TokenKind::Identifier
            && self.peek_ahead(3).kind == TokenKind::Colon
        {
            let static_span = self.advance().span;
            let duplicate = self.advance();
            self.report(
                INVALID_CLASS_MEMBER,
                "a field cannot repeat `static`",
                duplicate.span,
                "remove this duplicate static modifier",
            );
            let _ = self.parse_static_field(visibility, static_span, Some(final_span));
            return None;
        }

        if !self.at(TokenKind::Identifier) || self.peek_ahead(1).kind != TokenKind::Colon {
            self.report(
                INVALID_CLASS_MEMBER,
                "`final` modifies only an instance or static field",
                final_span,
                "expected `final name: type;` or `final static name: type = expression;`",
            );
            return None;
        }

        let field = self.parse_field(visibility, None, Some(final_span))?;
        valid.then_some(ClassMember::Field(field))
    }

    fn parse_static_field_modifier(&mut self, visibility: MemberVisibility) -> ParsedClassField {
        let next = self.peek_ahead(1);
        if next.kind == TokenKind::Identifier && self.peek_ahead(2).kind == TokenKind::Colon {
            let static_span = self.advance().span;
            return ParsedClassField::parsed(
                self.parse_static_field(visibility, static_span, None)
                    .map(ClassMember::StaticField),
            );
        }

        if self.lexeme(next) == "cell"
            && self.peek_ahead(2).kind == TokenKind::Identifier
            && self.peek_ahead(3).kind == TokenKind::Colon
        {
            self.advance();
            let cell_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "cell fields cannot be static",
                cell_span,
                "declare an instance field as `private cell name: type;`",
            );
            let _ = self.parse_field(visibility, Some(cell_span), None);
            return ParsedClassField::parsed(None);
        }

        if self.lexeme(next) == "final"
            && self.peek_ahead(2).kind == TokenKind::Identifier
            && self.peek_ahead(3).kind == TokenKind::Colon
        {
            let static_span = self.advance().span;
            let final_span = self.advance().span;
            self.report(
                INVALID_CLASS_MEMBER,
                "`final` must precede `static`",
                final_span,
                "write `final static name: type = expression;`",
            );
            let _ = self.parse_static_field(visibility, static_span, Some(final_span));
            return ParsedClassField::parsed(None);
        }

        ParsedClassField::not_field()
    }
}
