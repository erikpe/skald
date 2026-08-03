//! File-scope import and visibility grammar.

use super::*;

impl Parser<'_> {
    pub(super) fn starts_import(&self) -> bool {
        self.at_contextual("import") || self.at_contextual("from")
    }

    pub(super) fn parse_import(&mut self) -> Option<ImportDeclaration> {
        if self.at_contextual("from") {
            self.parse_selective_import()
                .map(ImportDeclaration::Selective)
        } else {
            self.parse_module_import().map(ImportDeclaration::Module)
        }
    }

    fn parse_module_import(&mut self) -> Option<ModuleImport> {
        let import = self.advance();
        let module = self.parse_import_path("expected a module path after `import`")?;
        let (as_span, alias) = self.parse_alias("module")?;
        self.reject_qualified_alias(&alias)?;
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the import")?;
        Some(ModuleImport {
            import_span: import.span,
            span: self.cover(import.span, semicolon.span),
            module,
            as_span,
            alias,
            semicolon_span: semicolon.span,
        })
    }

    fn parse_selective_import(&mut self) -> Option<SelectiveImport> {
        let from = self.advance();
        let module = self.parse_import_path("expected a module path after `from`")?;
        let import = if self.at_contextual("import") {
            self.advance()
        } else {
            self.report(
                INVALID_IMPORT,
                "expected `import` after the source module",
                self.peek().span,
                "selective imports use `from path import Name;`",
            );
            return None;
        };
        if self.at(TokenKind::Star) {
            let star_span = self.advance().span;
            self.report(
                INVALID_IMPORT,
                "wildcard imports are not supported",
                star_span,
                "name each imported declaration explicitly",
            );
            return None;
        }

        let mut items = Vec::new();
        let mut comma_spans = Vec::new();
        loop {
            let name = self.parse_name("expected a declaration name to import")?;
            let (as_span, alias) = self.parse_alias("declaration")?;
            self.reject_qualified_alias(&alias)?;
            let end = alias.as_ref().map_or(name.span, |alias| alias.span);
            items.push(SelectiveImportItem {
                span: self.cover(name.span, end),
                name,
                as_span,
                alias,
            });
            let Some(comma) = self.consume(TokenKind::Comma) else {
                break;
            };
            comma_spans.push(comma.span);
            if self.at(TokenKind::Semicolon) {
                self.report(
                    INVALID_IMPORT,
                    "selective imports do not allow a trailing comma",
                    comma.span,
                    "remove this comma",
                );
                return None;
            }
        }
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the selective import")?;
        Some(SelectiveImport {
            from_span: from.span,
            span: self.cover(from.span, semicolon.span),
            module,
            import_span: import.span,
            items,
            comma_spans,
            semicolon_span: semicolon.span,
        })
    }

    fn parse_import_path(&mut self, message: &'static str) -> Option<Name> {
        if self.at_any(&[TokenKind::Dot, TokenKind::DoubleColon]) {
            self.report(
                INVALID_IMPORT,
                "module paths must be absolute logical paths",
                self.peek().span,
                "relative and empty module paths are not supported",
            );
            return None;
        }
        self.parse_module_name_path(message)
    }

    fn parse_alias(&mut self, kind: &'static str) -> Option<(Option<Span>, Option<Name>)> {
        if !self.at_contextual("as") {
            return Some((None, None));
        }
        let as_token = self.advance();
        let alias = self.parse_name(format_alias_expectation(kind))?;
        Some((Some(as_token.span), Some(alias)))
    }

    fn reject_qualified_alias(&mut self, alias: &Option<Name>) -> Option<()> {
        if alias.is_some() && self.at(TokenKind::DoubleColon) {
            self.report(
                INVALID_IMPORT,
                "import aliases must be one identifier",
                self.peek().span,
                "remove the additional alias components",
            );
            return None;
        }
        Some(())
    }
}

fn format_alias_expectation(kind: &str) -> &'static str {
    if kind == "module" {
        "a one-identifier module alias after `as`"
    } else {
        "a one-identifier declaration alias after `as`"
    }
}
