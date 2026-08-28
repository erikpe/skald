//! Closing source syntax types into canonical specialization requests.

use super::super::closed_types::object_target;
use super::*;

pub(super) struct SyntaxTypeCloser<'owner, 'semantic, 'interner, 'diagnostics, 'lookup> {
    owner: &'owner mut SpecializationCoordinator<'semantic, 'interner, 'diagnostics>,
    lookup: ModuleLookup<'lookup>,
    module: ModuleId,
}

impl<'owner, 'semantic, 'interner, 'diagnostics, 'lookup>
    SyntaxTypeCloser<'owner, 'semantic, 'interner, 'diagnostics, 'lookup>
{
    pub(super) fn new(
        owner: &'owner mut SpecializationCoordinator<'semantic, 'interner, 'diagnostics>,
        lookup: ModuleLookup<'lookup>,
        module: ModuleId,
    ) -> Self {
        Self {
            owner,
            lookup,
            module,
        }
    }

    pub(super) fn close(&mut self, syntax: &syntax::TypeSyntax) -> Option<ResolvedTypeKind> {
        self.close_with_lookup_diagnostics(syntax, false)
    }

    pub(super) fn request_range(
        &mut self,
        template: ClassTemplateId,
        endpoint: ResolvedTypeKind,
        span: Span,
    ) -> Option<ClassId> {
        self.owner.request_class(
            template,
            vec![endpoint],
            GenericApplicationOrigin {
                module: self.module,
                span,
            },
        )
    }

    pub(super) fn constructor_type(
        &mut self,
        callee: &syntax::Expression,
    ) -> Option<ResolvedTypeKind> {
        match callee {
            syntax::Expression::Identifier(identifier) => {
                let symbol = self.select(&identifier.name, false)?;
                match symbol.kind {
                    TopLevelSymbolKind::Class(class) => Some(ResolvedTypeKind::Class(class)),
                    _ => None,
                }
            }
            syntax::Expression::GenericTypeApplication(application) => {
                self.close_named(&application.target, false)
            }
            _ => None,
        }
    }

    pub(super) fn function(&mut self, name: &syntax::Name) -> Option<FunctionId> {
        let symbol = self.select(name, false)?;
        match symbol.kind {
            TopLevelSymbolKind::Function(function) => Some(function),
            _ => None,
        }
    }

    fn close_with_lookup_diagnostics(
        &mut self,
        syntax: &syntax::TypeSyntax,
        report_lookup_errors: bool,
    ) -> Option<ResolvedTypeKind> {
        Some(match &syntax.kind {
            syntax::TypeKind::I64 => ResolvedTypeKind::I64,
            syntax::TypeKind::U64 => ResolvedTypeKind::U64,
            syntax::TypeKind::U8 => ResolvedTypeKind::U8,
            syntax::TypeKind::F64 => ResolvedTypeKind::F64,
            syntax::TypeKind::Bool => ResolvedTypeKind::Bool,
            syntax::TypeKind::Unit => ResolvedTypeKind::Unit,
            syntax::TypeKind::Function(function) => {
                let mut parameters = Vec::with_capacity(function.parameters.len());
                for parameter in &function.parameters {
                    let mode = match parameter.mode {
                        syntax::FunctionTypeParameterMode::Value => {
                            ResolvedFunctionTypeParameterMode::Value
                        }
                        syntax::FunctionTypeParameterMode::ReadOnlyAlias { .. } => {
                            ResolvedFunctionTypeParameterMode::ReadOnlyAlias
                        }
                        syntax::FunctionTypeParameterMode::MutableAlias { .. } => {
                            ResolvedFunctionTypeParameterMode::MutableAlias
                        }
                    };
                    parameters.push(ResolvedFunctionTypeParameter {
                        mode,
                        type_syntax: ResolvedType {
                            kind: self.close_with_lookup_diagnostics(
                                &parameter.type_syntax,
                                report_lookup_errors,
                            )?,
                            span: parameter.type_syntax.span,
                        },
                        span: parameter.span,
                    });
                }
                let result = ResolvedType {
                    kind: self
                        .close_with_lookup_diagnostics(&function.result, report_lookup_errors)?,
                    span: function.result.span,
                };
                let id = self
                    .owner
                    .interner
                    .intern_function(parameters, result, function.span);
                ResolvedTypeKind::Function(id)
            }
            syntax::TypeKind::Named(named) => return self.close_named(named, report_lookup_errors),
            syntax::TypeKind::Shared { target, .. } => {
                ResolvedTypeKind::Shared(self.close_shared_target(target, report_lookup_errors)?)
            }
            syntax::TypeKind::Optional { payload, .. } => {
                let payload = ResolvedType {
                    kind: self.close_with_lookup_diagnostics(payload, report_lookup_errors)?,
                    span: payload.span,
                };
                ResolvedTypeKind::Optional(self.owner.interner.intern_optional(payload))
            }
            syntax::TypeKind::Grouped { inner, .. } => {
                return self.close_with_lookup_diagnostics(inner, report_lookup_errors)
            }
            syntax::TypeKind::Array { element, .. } => {
                let element = ResolvedType {
                    kind: self.close_with_lookup_diagnostics(element, report_lookup_errors)?,
                    span: element.span,
                };
                ResolvedTypeKind::Array(self.owner.interner.intern_array(element))
            }
        })
    }

    pub(super) fn close_named(
        &mut self,
        named: &syntax::NamedTypeSyntax,
        report_lookup_errors: bool,
    ) -> Option<ResolvedTypeKind> {
        if !named.name.is_qualified() && named.name.text == "Obj" {
            if named.arguments.is_none() {
                return Some(ResolvedTypeKind::Obj);
            }
            if report_lookup_errors {
                self.owner.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::INVALID_GENERIC_APPLICATION,
                        "`Obj` is not a generic class",
                    )
                    .with_primary_label(named.span, "type arguments are not allowed here"),
                );
            }
            return None;
        }
        let symbol = self.select(&named.name, report_lookup_errors)?;
        match (symbol.kind, &named.arguments) {
            (TopLevelSymbolKind::Class(class), None) => Some(ResolvedTypeKind::Class(class)),
            (TopLevelSymbolKind::Interface(interface), None) => {
                Some(ResolvedTypeKind::Interface(interface))
            }
            (TopLevelSymbolKind::ClassTemplate(template), Some(arguments))
                if arguments.arguments.len() == self.lookup.template_arity(template) =>
            {
                let mut closed = Vec::with_capacity(arguments.arguments.len());
                let mut valid = true;
                for argument in &arguments.arguments {
                    match self.close_with_lookup_diagnostics(argument, true) {
                        Some(argument) => closed.push(argument),
                        None => valid = false,
                    }
                }
                if !valid {
                    return None;
                }
                self.owner
                    .request_class(
                        template,
                        closed,
                        GenericApplicationOrigin {
                            module: self.module,
                            span: named.span,
                        },
                    )
                    .map(ResolvedTypeKind::Class)
            }
            (TopLevelSymbolKind::ClassTemplate(template), Some(arguments)) => {
                if report_lookup_errors {
                    let expected = self.lookup.template_arity(template);
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::GENERIC_ARITY_MISMATCH,
                            format!(
                                "generic class `{}` expects {expected} type argument{}",
                                named.name.text,
                                if expected == 1 { "" } else { "s" },
                            ),
                        )
                        .with_primary_label(arguments.span, "wrong number of type arguments")
                        .with_secondary_label(symbol.name_span, "template declared here"),
                    );
                }
                None
            }
            (TopLevelSymbolKind::ClassTemplate(_), None) => {
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::RAW_GENERIC_TYPE,
                            format!(
                                "generic class `{}` requires type arguments",
                                named.name.text
                            ),
                        )
                        .with_primary_label(named.name.span, "type arguments cannot be omitted")
                        .with_secondary_label(symbol.name_span, "template declared here"),
                    );
                }
                None
            }
            (TopLevelSymbolKind::InterfaceTemplate(template), Some(arguments))
                if arguments.arguments.len() == self.lookup.interface_template_arity(template) =>
            {
                let mut closed = Vec::with_capacity(arguments.arguments.len());
                let mut valid = true;
                for argument in &arguments.arguments {
                    match self.close_with_lookup_diagnostics(argument, true) {
                        Some(argument) => closed.push(argument),
                        None => valid = false,
                    }
                }
                if !valid {
                    return None;
                }
                let interface = self.owner.request_interface(
                    template,
                    closed,
                    GenericInterfaceApplicationOrigin {
                        module: self.module,
                        span: named.span,
                    },
                );
                // The identity is usable while closing an enclosing request;
                // declaration materialization decides whether ordinary
                // semantic resolution may consume it.
                interface.map(ResolvedTypeKind::Interface)
            }
            (TopLevelSymbolKind::InterfaceTemplate(template), Some(arguments)) => {
                if report_lookup_errors {
                    let expected = self.lookup.interface_template_arity(template);
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::GENERIC_ARITY_MISMATCH,
                            format!(
                                "generic interface `{}` expects {expected} type argument{}",
                                named.name.text,
                                if expected == 1 { "" } else { "s" }
                            ),
                        )
                        .with_primary_label(arguments.span, "wrong number of type arguments")
                        .with_secondary_label(symbol.name_span, "template declared here"),
                    );
                }
                None
            }
            (TopLevelSymbolKind::InterfaceTemplate(_), None) => {
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::RAW_GENERIC_TYPE,
                            format!(
                                "generic interface `{}` requires type arguments",
                                named.name.text
                            ),
                        )
                        .with_primary_label(named.name.span, "type arguments cannot be omitted")
                        .with_secondary_label(symbol.name_span, "template declared here"),
                    );
                }
                None
            }
            (TopLevelSymbolKind::Class(_), Some(arguments))
            | (TopLevelSymbolKind::Interface(_), Some(arguments))
            | (TopLevelSymbolKind::Function(_), Some(arguments)) => {
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::INVALID_GENERIC_APPLICATION,
                            format!("`{}` is not a generic class", named.name.text),
                        )
                        .with_primary_label(arguments.span, "type arguments are not allowed here")
                        .with_secondary_label(symbol.name_span, "declaration is non-generic"),
                    );
                }
                None
            }
            (TopLevelSymbolKind::Function(_), None) => {
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::UNKNOWN_TYPE,
                            format!("`{}` does not name a type", named.name.text),
                        )
                        .with_primary_label(named.name.span, "expected a class or interface type")
                        .with_secondary_label(symbol.name_span, "function declared here"),
                    );
                }
                None
            }
        }
    }

    fn close_shared_target(
        &mut self,
        target: &syntax::TypeSyntax,
        report_lookup_errors: bool,
    ) -> Option<ResolvedSharedTarget> {
        let (optional_depth, leaf) = syntax_optional_leaf(target);
        if optional_depth > 0 {
            let leaf = self.close_with_lookup_diagnostics(leaf, report_lookup_errors)?;
            if let Some(object) = object_target(leaf) {
                if matches!(
                    object,
                    ResolvedObjectTarget::Obj | ResolvedObjectTarget::Interface(_)
                ) {
                    return Some(ResolvedSharedTarget::OptionalBox(
                        self.owner.interner.intern_optional_object_box_view(
                            optional_depth,
                            object,
                            target.span,
                        ),
                    ));
                }
            }
        }
        let kind = self.close_with_lookup_diagnostics(target, report_lookup_errors)?;
        match kind {
            ResolvedTypeKind::Optional(optional) => Some(ResolvedSharedTarget::OptionalBox(
                self.owner
                    .interner
                    .intern_optional_box(optional, target.span),
            )),
            kind => match ResolvedSharedTarget::from_direct_type(kind) {
                Some(target) => Some(target),
                None => {
                    if report_lookup_errors {
                        self.owner.diagnostics.push(
                            Diagnostic::error(
                                super::super::super::UNKNOWN_TYPE,
                                "shared ownership requires an object target",
                            )
                            .with_primary_label(
                                target.span,
                                "expected a class, interface, `Obj`, or array type",
                            ),
                        );
                    }
                    None
                }
            },
        }
    }

    fn select(
        &mut self,
        name: &syntax::Name,
        report_lookup_errors: bool,
    ) -> Option<TopLevelSymbol> {
        let lookup = if report_lookup_errors {
            self.lookup.select(name, self.owner.diagnostics)
        } else {
            // Ordinary resolution already diagnosed the outer spelling.
            let mut diagnostics = Diagnostics::new();
            self.lookup.select(name, &mut diagnostics)
        };
        match lookup {
            TopLevelLookup::Found(symbol) => Some(symbol),
            TopLevelLookup::Missing => {
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::UNKNOWN_TYPE,
                            format!("unknown type `{}`", name.text),
                        )
                        .with_primary_label(name.span, "no type with this name is declared"),
                    );
                }
                None
            }
            TopLevelLookup::Diagnosed => None,
        }
    }
}

fn syntax_optional_leaf(mut syntax: &syntax::TypeSyntax) -> (usize, &syntax::TypeSyntax) {
    let mut depth = 0;
    loop {
        match &syntax.kind {
            syntax::TypeKind::Grouped { inner, .. } => syntax = inner,
            syntax::TypeKind::Optional { payload, .. } => {
                depth += 1;
                syntax = payload;
            }
            _ => return (depth, syntax),
        }
    }
}
