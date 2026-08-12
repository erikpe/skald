//! Structural template-type resolution under the definition module.

use super::*;

pub(super) struct TemplateTypeResolver<'parameters, 'diagnostics> {
    parameters: &'parameters ResolvedTypeParameters,
    lookup: ModuleLookup<'parameters>,
    diagnostics: &'diagnostics mut Diagnostics,
}

impl<'parameters, 'diagnostics> TemplateTypeResolver<'parameters, 'diagnostics> {
    pub(super) fn new(
        parameters: &'parameters ResolvedTypeParameters,
        lookup: ModuleLookup<'parameters>,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            parameters,
            lookup,
            diagnostics,
        }
    }

    pub(super) fn resolve(&mut self, syntax: &syntax::TypeSyntax) -> Option<ResolvedTemplateType> {
        let kind = match &syntax.kind {
            syntax::TypeKind::I64 => ResolvedTemplateTypeKind::I64,
            syntax::TypeKind::U64 => ResolvedTemplateTypeKind::U64,
            syntax::TypeKind::U8 => ResolvedTemplateTypeKind::U8,
            syntax::TypeKind::F64 => ResolvedTemplateTypeKind::F64,
            syntax::TypeKind::Bool => ResolvedTemplateTypeKind::Bool,
            syntax::TypeKind::Unit => ResolvedTemplateTypeKind::Unit,
            syntax::TypeKind::Named(named) => return self.resolve_named(named),
            syntax::TypeKind::Shared { target, .. } => {
                ResolvedTemplateTypeKind::Shared(Box::new(self.resolve(target)?))
            }
            syntax::TypeKind::Optional { payload, .. } => {
                ResolvedTemplateTypeKind::Optional(Box::new(self.resolve(payload)?))
            }
            syntax::TypeKind::Grouped { inner, .. } => {
                let mut resolved = self.resolve(inner)?;
                resolved.span = syntax.span;
                return Some(resolved);
            }
            syntax::TypeKind::Array { element, .. } => {
                ResolvedTemplateTypeKind::Array(Box::new(self.resolve(element)?))
            }
        };
        Some(ResolvedTemplateType {
            kind,
            span: syntax.span,
        })
    }

    pub(super) fn resolve_named(
        &mut self,
        named: &syntax::NamedTypeSyntax,
    ) -> Option<ResolvedTemplateType> {
        if !named.name.is_qualified() {
            if let Some(parameter) = self.parameters.get(named.name.text.as_str()) {
                if let Some(arguments) = &named.arguments {
                    self.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::INVALID_GENERIC_APPLICATION,
                            format!(
                                "type parameter `{}` is not a generic class",
                                named.name.text
                            ),
                        )
                        .with_primary_label(arguments.span, "type arguments are not allowed here")
                        .with_secondary_label(parameter.name_span, "parameter declared here"),
                    );
                    return None;
                }
                return Some(ResolvedTemplateType {
                    kind: ResolvedTemplateTypeKind::Parameter(parameter.id),
                    span: named.span,
                });
            }
            if named.name.text == "Obj" {
                if let Some(arguments) = &named.arguments {
                    self.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::INVALID_GENERIC_APPLICATION,
                            "`Obj` is not a generic class",
                        )
                        .with_primary_label(arguments.span, "type arguments are not allowed here"),
                    );
                    return None;
                }
                return Some(ResolvedTemplateType {
                    kind: ResolvedTemplateTypeKind::Obj,
                    span: named.span,
                });
            }
        }

        match self.lookup.select(&named.name, self.diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                name_span,
            }) => {
                self.resolve_non_generic(named, name_span, ResolvedTemplateTypeKind::Class(class))
            }
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(interface),
                name_span,
            }) => self.resolve_non_generic(
                named,
                name_span,
                ResolvedTemplateTypeKind::Interface(interface),
            ),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::ClassTemplate(template),
                name_span,
            }) => {
                let Some(arguments) = &named.arguments else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::RAW_GENERIC_TYPE,
                            format!(
                                "generic class `{}` requires type arguments",
                                named.name.text
                            ),
                        )
                        .with_primary_label(named.name.span, "type arguments cannot be omitted")
                        .with_secondary_label(name_span, "template declared here"),
                    );
                    return None;
                };
                let expected = self.lookup.template_arity(template);
                if arguments.arguments.len() != expected {
                    self.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::GENERIC_ARITY_MISMATCH,
                            format!(
                                "generic class `{}` expects {expected} type argument{}",
                                named.name.text,
                                if expected == 1 { "" } else { "s" },
                            ),
                        )
                        .with_primary_label(arguments.span, "wrong number of type arguments")
                        .with_secondary_label(name_span, "template declared here"),
                    );
                    return None;
                }
                let arguments = arguments
                    .arguments
                    .iter()
                    .filter_map(|argument| self.resolve(argument))
                    .collect::<Vec<_>>();
                (arguments.len() == expected).then_some(ResolvedTemplateType {
                    kind: ResolvedTemplateTypeKind::ClassTemplate {
                        template,
                        arguments,
                    },
                    span: named.span,
                })
            }
            TopLevelLookup::Found(symbol) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::UNKNOWN_TYPE,
                        format!("`{}` does not name a type", named.name.text),
                    )
                    .with_primary_label(named.name.span, "expected a type declaration")
                    .with_secondary_label(symbol.name_span, "function declared here"),
                );
                None
            }
            TopLevelLookup::Missing => {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::UNKNOWN_TYPE,
                        format!("unknown type `{}`", named.name.text),
                    )
                    .with_primary_label(
                        named.name.span,
                        "no type with this name is visible in the template's module",
                    ),
                );
                None
            }
            TopLevelLookup::Diagnosed => None,
        }
    }

    fn resolve_non_generic(
        &mut self,
        named: &syntax::NamedTypeSyntax,
        declaration_span: Span,
        kind: ResolvedTemplateTypeKind,
    ) -> Option<ResolvedTemplateType> {
        if let Some(arguments) = &named.arguments {
            self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_GENERIC_APPLICATION,
                    format!("`{}` is not a generic class", named.name.text),
                )
                .with_primary_label(arguments.span, "type arguments are not allowed here")
                .with_secondary_label(declaration_span, "declaration is non-generic"),
            );
            return None;
        }
        Some(ResolvedTemplateType {
            kind,
            span: named.span,
        })
    }
}
