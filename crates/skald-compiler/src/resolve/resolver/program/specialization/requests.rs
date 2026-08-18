//! Canonical source-order discovery of explicit closed applications.

use super::super::resolver::{ModuleUnit, ProgramLookupTables};
use super::{closed_types::object_target, *};

pub(crate) struct SpecializationDiscoveryInput<'program, 'ast> {
    units: &'program [ModuleUnit<'ast>],
    modules: &'program crate::module::ProgramModuleTable,
    lookups: ProgramLookupTables<'program>,
    templates: GenericTemplateDiscoveryInput<'program>,
    ordinary_class_count: usize,
}

pub(crate) struct GenericTemplateDiscoveryInput<'program> {
    class_semantics: &'program ResolvedClassTemplateSemanticTable,
    interface_semantics: &'program ResolvedInterfaceTemplateSemanticTable,
    classes: &'program ResolvedClassTemplateTable,
    interfaces: &'program ResolvedInterfaceTemplateTable,
}

impl<'program> GenericTemplateDiscoveryInput<'program> {
    pub(crate) const fn new(
        class_semantics: &'program ResolvedClassTemplateSemanticTable,
        interface_semantics: &'program ResolvedInterfaceTemplateSemanticTable,
        classes: &'program ResolvedClassTemplateTable,
        interfaces: &'program ResolvedInterfaceTemplateTable,
    ) -> Self {
        Self {
            class_semantics,
            interface_semantics,
            classes,
            interfaces,
        }
    }
}

impl<'program, 'ast> SpecializationDiscoveryInput<'program, 'ast> {
    pub(crate) fn new(
        units: &'program [ModuleUnit<'ast>],
        modules: &'program crate::module::ProgramModuleTable,
        lookups: ProgramLookupTables<'program>,
        templates: GenericTemplateDiscoveryInput<'program>,
        ordinary_class_count: usize,
    ) -> Self {
        Self {
            units,
            modules,
            lookups,
            templates,
            ordinary_class_count,
        }
    }
}

pub(crate) struct GenericApplicationDiscovery {
    pub(crate) class_specializations: GenericSpecializationTable,
    pub(crate) interface_applications: ResolvedGenericInterfaceApplicationTable,
}

pub(crate) fn discover_specializations(
    input: SpecializationDiscoveryInput<'_, '_>,
    interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> GenericApplicationDiscovery {
    let mut owner = SpecializationOwner::new(
        input.templates.class_semantics,
        input.templates.classes,
        interner,
        diagnostics,
        input.ordinary_class_count,
    );
    record_template_interface_applications(&mut owner, &input);
    for unit in input.units {
        let lookup = input.lookups.for_unit(unit, input.modules);
        SourceRequestScanner {
            resolver: SyntaxTypeCloser {
                owner: &mut owner,
                lookup,
                module: unit.module,
            },
        }
        .visit_unit(unit.ast);
    }
    owner.finish()
}

fn record_template_interface_applications(
    owner: &mut SpecializationOwner<'_, '_, '_>,
    input: &SpecializationDiscoveryInput<'_, '_>,
) {
    for semantics in input.templates.class_semantics.iter() {
        let module = input
            .templates
            .classes
            .get(semantics.template)
            .expect("class semantics reference a collected template")
            .module;
        for type_use in &semantics.type_uses {
            owner
                .interface_applications
                .record_type(module, &type_use.type_term);
        }
        for claim in &semantics.implemented_interfaces {
            owner.interface_applications.record_interface(
                &claim.interface,
                GenericInterfaceApplicationOrigin {
                    module,
                    span: claim.span,
                },
            );
        }
        for bound in &semantics.bounds {
            owner.interface_applications.record_interface(
                &bound.interface,
                GenericInterfaceApplicationOrigin {
                    module,
                    span: bound.interface_span,
                },
            );
        }
    }
    for semantics in input.templates.interface_semantics.iter() {
        let module = input
            .templates
            .interfaces
            .get(semantics.template)
            .expect("interface semantics reference a collected template")
            .module;
        for type_use in &semantics.type_uses {
            owner
                .interface_applications
                .record_type(module, &type_use.type_term);
        }
    }
}

struct SyntaxTypeCloser<'owner, 'semantic, 'interner, 'diagnostics, 'lookup> {
    owner: &'owner mut SpecializationOwner<'semantic, 'interner, 'diagnostics>,
    lookup: ModuleLookup<'lookup>,
    module: ModuleId,
}

impl SyntaxTypeCloser<'_, '_, '_, '_, '_> {
    fn close(&mut self, syntax: &syntax::TypeSyntax) -> Option<ResolvedTypeKind> {
        self.close_with_lookup_diagnostics(syntax, false)
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

    fn close_named(
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
                    .request(
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
            (TopLevelSymbolKind::InterfaceTemplate(_), Some(_)) => {
                let syntax = syntax::TypeSyntax {
                    kind: syntax::TypeKind::Named(named.clone()),
                    span: named.span,
                };
                let term = if report_lookup_errors {
                    super::super::generic_templates::TemplateTypeResolver::for_application_site(
                        self.lookup,
                        self.owner.diagnostics,
                    )
                    .resolve(&syntax)?
                } else {
                    // Ordinary resolution owns diagnostics for the outer site.
                    // Discovery repeats structural resolution silently so it
                    // can retain a request without allocating an InterfaceId.
                    let mut scratch = Diagnostics::new();
                    super::super::generic_templates::TemplateTypeResolver::for_application_site(
                        self.lookup,
                        &mut scratch,
                    )
                    .resolve(&syntax)?
                };
                let interface = ResolvedInterfaceType::from_type(&term)
                    .expect("an interface-template application resolves as an interface");
                self.owner.interface_applications.record_interface(
                    &interface,
                    GenericInterfaceApplicationOrigin {
                        module: self.module,
                        span: named.span,
                    },
                );
                if report_lookup_errors {
                    self.owner.diagnostics.push(
                        Diagnostic::error(
                            super::super::super::UNSUPPORTED_GENERIC_INTERFACE,
                            format!(
                                "generic interface application `{}` is resolved but not yet specialized",
                                named.name.text
                            ),
                        )
                        .with_primary_label(
                            named.span,
                            "closed interface specialization is implemented by the next roadmap stage",
                        )
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

struct SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup> {
    resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
}

impl SourceRequestScanner<'_, '_, '_, '_, '_> {
    fn visit_unit(&mut self, unit: &syntax::CompilationUnit) {
        for declaration in &unit.declarations {
            self.visit_declaration(declaration);
        }
    }

    fn visit_declaration(&mut self, declaration: &syntax::TopLevelDeclaration) {
        match declaration {
            syntax::TopLevelDeclaration::Function(function) => {
                self.visit_parameters(&function.parameters);
                self.visit_type(&function.return_type);
                self.visit_block(&function.body);
            }
            syntax::TopLevelDeclaration::ExternalFunction(function) => {
                self.visit_parameters(&function.parameters);
                self.visit_type(&function.return_type);
            }
            syntax::TopLevelDeclaration::IntrinsicFunction(function) => {
                self.visit_parameters(&function.parameters);
                self.visit_type(&function.return_type);
            }
            syntax::TopLevelDeclaration::Class(class) if class.type_parameters.is_none() => {
                if let Some(base) = &class.direct_base {
                    self.visit_named_type(base);
                }
                for member in &class.members {
                    self.visit_member(member);
                }
            }
            syntax::TopLevelDeclaration::Interface(interface)
                if interface.type_parameters.is_none() =>
            {
                for requirement in &interface.requirements {
                    self.visit_parameters(&requirement.parameters);
                    self.visit_type(&requirement.return_type);
                }
            }
            syntax::TopLevelDeclaration::Class(_) | syntax::TopLevelDeclaration::Interface(_) => {
                // Applications in an unrequested template are discovered only
                // after substitution closes that template.
            }
        }
    }

    fn visit_member(&mut self, member: &syntax::ClassMember) {
        match member {
            syntax::ClassMember::Field(field) => self.visit_type(&field.type_syntax),
            syntax::ClassMember::StaticField(field) => {
                self.visit_type(&field.type_syntax);
                if let Some(initializer) = &field.initializer {
                    self.visit_expression(&initializer.expression);
                }
            }
            syntax::ClassMember::Initializer(declaration) => {
                self.visit_parameters(&declaration.parameters);
                self.visit_block(&declaration.body);
            }
            syntax::ClassMember::CopyConstructor(declaration) => {
                self.visit_parameters(&declaration.parameters);
                self.visit_block(&declaration.body);
            }
            syntax::ClassMember::CopyAssignment(declaration) => {
                self.visit_parameters(&declaration.parameters);
                self.visit_block(&declaration.body);
            }
            syntax::ClassMember::Destructor(declaration) => self.visit_block(&declaration.body),
            syntax::ClassMember::Method(declaration) => {
                self.visit_parameters(&declaration.parameters);
                self.visit_type(&declaration.return_type);
                self.visit_block(&declaration.body);
            }
        }
    }

    fn visit_parameters(&mut self, parameters: &[syntax::Parameter]) {
        for parameter in parameters {
            self.visit_type(&parameter.type_syntax);
        }
    }

    fn visit_type(&mut self, syntax: &syntax::TypeSyntax) {
        let _ = self.resolver.close(syntax);
    }

    fn visit_named_type(&mut self, syntax: &syntax::NamedTypeSyntax) {
        let _ = self.resolver.close_named(syntax, false);
    }

    fn visit_block(&mut self, block: &syntax::Block) {
        for statement in &block.statements {
            self.visit_statement(statement);
        }
    }

    fn visit_statement(&mut self, statement: &syntax::Statement) {
        match statement {
            syntax::Statement::BaseInitialization(statement) => {
                self.visit_expressions(&statement.arguments)
            }
            syntax::Statement::Local(statement) => {
                self.visit_type(&statement.type_syntax);
                self.visit_expression(&statement.initializer);
            }
            syntax::Statement::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.visit_expression(value);
                }
            }
            syntax::Statement::Break(_) | syntax::Statement::Continue(_) => {}
            syntax::Statement::Expression(statement) => {
                self.visit_expression(&statement.expression)
            }
            syntax::Statement::Conditional(statement) => {
                self.visit_expression(&statement.if_arm.condition);
                self.visit_block(&statement.if_arm.body);
                for arm in &statement.elif_arms {
                    self.visit_expression(&arm.condition);
                    self.visit_block(&arm.body);
                }
                if let Some(body) = &statement.else_block {
                    self.visit_block(body);
                }
            }
            syntax::Statement::While(statement) => {
                self.visit_expression(&statement.condition);
                self.visit_block(&statement.body);
            }
            syntax::Statement::Block(block) => self.visit_block(block),
            syntax::Statement::FieldAssignment(statement) => {
                self.visit_expression(&statement.place.receiver);
                self.visit_expression(&statement.value);
            }
            syntax::Statement::ObjectAssignment(statement) => {
                self.visit_expression(&statement.place);
                self.visit_expression(&statement.value);
            }
        }
    }

    fn visit_expression(&mut self, expression: &syntax::Expression) {
        match expression {
            syntax::Expression::Absent(_)
            | syntax::Expression::Identifier(_)
            | syntax::Expression::NumericLiteral(_)
            | syntax::Expression::ByteLiteral(_)
            | syntax::Expression::StringLiteral(_)
            | syntax::Expression::Boolean(_)
            | syntax::Expression::SelfValue(_) => {}
            syntax::Expression::Present(expression) => self.visit_expression(&expression.value),
            syntax::Expression::GenericTypeApplication(application) => {
                self.visit_named_type(&application.target)
            }
            syntax::Expression::GenericStaticSelection(selection) => {
                self.visit_named_type(&selection.target)
            }
            syntax::Expression::Unary(expression) => self.visit_expression(&expression.operand),
            syntax::Expression::Binary(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
            }
            syntax::Expression::Logical(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
            }
            syntax::Expression::TypeTest(expression) => {
                self.visit_expression(&expression.source);
                self.visit_named_type(&expression.target);
            }
            syntax::Expression::PresenceTest(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::Unwrap(expression) => self.visit_expression(&expression.source),
            syntax::Expression::PrimitiveCast(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::ObjectCast(expression) => {
                self.visit_named_type(&expression.target);
                self.visit_expression(&expression.source);
            }
            syntax::Expression::Allocation(expression) => {
                self.visit_named_type(&expression.target);
                self.visit_call_arguments(&expression.arguments);
            }
            syntax::Expression::OptionalBoxAllocation(expression) => {
                self.visit_type(&expression.target);
                if let syntax::OptionalBoxInitializer::Value { value, .. } = &expression.initializer
                {
                    self.visit_expression(value);
                }
            }
            syntax::Expression::ArrayConstruction(expression) => {
                self.visit_type(&expression.array_type);
                match &expression.arguments {
                    syntax::ArrayConstructionArguments::Empty { .. } => {}
                    syntax::ArrayConstructionArguments::Length { length, .. } => {
                        self.visit_expression(length)
                    }
                    syntax::ArrayConstructionArguments::Copy { source, .. } => {
                        self.visit_expression(source)
                    }
                    syntax::ArrayConstructionArguments::Elements(elements) => {
                        self.visit_expressions(&elements.elements)
                    }
                }
            }
            syntax::Expression::Call(expression) => {
                self.visit_expression(&expression.callee);
                self.visit_call_arguments(&expression.arguments);
            }
            syntax::Expression::Grouped(expression) => {
                self.visit_expression(&expression.expression)
            }
            syntax::Expression::MemberAccess(expression) => {
                self.visit_expression(&expression.receiver)
            }
            syntax::Expression::BracketProjection(expression) => {
                self.visit_expression(&expression.receiver);
                match &expression.bounds {
                    syntax::BracketProjectionBounds::Index(index) => self.visit_expression(index),
                    syntax::BracketProjectionBounds::Slice { start, end, .. } => {
                        if let Some(start) = start {
                            self.visit_expression(start);
                        }
                        if let Some(end) = end {
                            self.visit_expression(end);
                        }
                    }
                }
            }
        }
    }

    fn visit_call_arguments(&mut self, arguments: &syntax::CallArguments) {
        match arguments {
            syntax::CallArguments::Ordinary(arguments) => self.visit_expressions(arguments),
            syntax::CallArguments::Copy { source, .. } => self.visit_expression(source),
        }
    }

    fn visit_expressions(&mut self, expressions: &[syntax::Expression]) {
        for expression in expressions {
            self.visit_expression(expression);
        }
    }
}
