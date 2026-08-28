//! Body scanning for parameter-bearing types and delayed semantic selections.

use std::collections::HashMap;

use super::requirements::{
    infer_type_construction, push, push_destruction, stored_initialization_copy_term,
};
use super::*;
use crate::identity::TypeParameterId;

mod operator;
mod provenance;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_template_body(
    member: &syntax::ClassMember,
    member_index: usize,
    parameters: &ResolvedTypeParameters,
    bounds: &[ResolvedTemplateBound],
    interfaces: &ResolvedInterfaceDeclarationTable,
    interface_semantics: &ResolvedInterfaceTemplateSemanticTable,
    iterable_language_item: Option<&ResolvedIterableLanguageItem>,
    operator_language_item: Option<&ResolvedOperatorLanguageItem>,
    lookup: ModuleLookup<'_>,
    fields: &HashMap<String, ResolvedTemplateType>,
    member_names: &HashMap<String, usize>,
    member_results: &HashMap<String, ResolvedTemplateType>,
    has_direct_base: bool,
    callable_parameters: &HashMap<String, ResolvedTemplateType>,
    callable_result: Option<&ResolvedTemplateType>,
    type_uses: &mut Vec<ResolvedTemplateTypeUse>,
    requirements: &mut Vec<GenericRequirement>,
    selections: &mut Vec<ResolvedTemplateSelection>,
    diagnostics: &mut Diagnostics,
) {
    let (body, initializer) = match member {
        syntax::ClassMember::Field(_) => (None, None),
        syntax::ClassMember::StaticField(field) => (
            None,
            field
                .initializer
                .as_ref()
                .map(|initializer| &initializer.expression),
        ),
        syntax::ClassMember::Initializer(declaration) => (Some(&declaration.body), None),
        syntax::ClassMember::CopyConstructor(declaration) => (Some(&declaration.body), None),
        syntax::ClassMember::CopyAssignment(declaration) => (Some(&declaration.body), None),
        syntax::ClassMember::Destructor(declaration) => (Some(&declaration.body), None),
        syntax::ClassMember::Method(declaration) => (Some(&declaration.body), None),
    };
    let field_writes_assign = matches!(
        member,
        syntax::ClassMember::CopyAssignment(_)
            | syntax::ClassMember::Destructor(_)
            | syntax::ClassMember::Method(_)
    );
    let mut resolver = TemplateBodyResolver {
        member: member_index,
        parameters,
        bounds,
        interfaces,
        interface_semantics,
        iterable_language_item,
        operator_language_item,
        lookup,
        fields,
        member_names,
        member_results,
        has_direct_base,
        field_writes_assign,
        scopes: vec![callable_parameters
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    TemplateBinding {
                        ty: ty.clone(),
                        depends_on_parameter: ty.depends_on_parameter(),
                    },
                )
            })
            .collect()],
        callable_result: callable_result.cloned(),
        type_uses,
        requirements,
        selections,
        diagnostics,
    };
    if let Some(expression) = initializer {
        resolver.visit_expression(expression);
    }
    if let Some(body) = body {
        resolver.visit_block(body);
    }
}

struct TemplateBodyResolver<'semantic, 'lookup, 'diagnostics> {
    member: usize,
    parameters: &'semantic ResolvedTypeParameters,
    bounds: &'semantic [ResolvedTemplateBound],
    interfaces: &'semantic ResolvedInterfaceDeclarationTable,
    interface_semantics: &'semantic ResolvedInterfaceTemplateSemanticTable,
    iterable_language_item: Option<&'semantic ResolvedIterableLanguageItem>,
    operator_language_item: Option<&'semantic ResolvedOperatorLanguageItem>,
    lookup: ModuleLookup<'lookup>,
    fields: &'semantic HashMap<String, ResolvedTemplateType>,
    member_names: &'semantic HashMap<String, usize>,
    member_results: &'semantic HashMap<String, ResolvedTemplateType>,
    has_direct_base: bool,
    field_writes_assign: bool,
    scopes: Vec<HashMap<String, TemplateBinding>>,
    callable_result: Option<ResolvedTemplateType>,
    type_uses: &'semantic mut Vec<ResolvedTemplateTypeUse>,
    requirements: &'semantic mut Vec<GenericRequirement>,
    selections: &'semantic mut Vec<ResolvedTemplateSelection>,
    diagnostics: &'diagnostics mut Diagnostics,
}

#[derive(Clone)]
struct TemplateBinding {
    ty: ResolvedTemplateType,
    depends_on_parameter: bool,
}

impl TemplateBodyResolver<'_, '_, '_> {
    fn visit_block(&mut self, block: &syntax::Block) {
        self.scopes.push(HashMap::new());
        for statement in &block.statements {
            self.visit_statement(statement);
        }
        self.scopes.pop();
    }

    fn visit_statement(&mut self, statement: &syntax::Statement) {
        match statement {
            syntax::Statement::BaseInitialization(statement) => {
                self.visit_expressions(&statement.arguments)
            }
            syntax::Statement::Local(local) => {
                self.visit_expression(&local.initializer);
                if let Some(term) = self.resolve_type_use(
                    &local.type_syntax,
                    ResolvedTemplateTypeUseContext::Local {
                        member: self.member,
                    },
                ) {
                    if let Some(copy_term) =
                        stored_initialization_copy_term(&term, &local.initializer)
                    {
                        self.record_requirement(
                            copy_term,
                            GenericCapability::CopyConstructible,
                            local.initializer.span(),
                            GenericRequirementReason::StoredInitializationCopy {
                                member: self.member,
                            },
                        );
                    }
                    push_destruction(self.requirements, &term, self.member);
                    let depends_on_parameter = term.depends_on_parameter()
                        || self.expression_depends_on_parameter(&local.initializer);
                    self.declare_binding(&local.name, term, depends_on_parameter, "local binding");
                }
            }
            syntax::Statement::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.visit_expression(value);
                    if let Some(result) = self.callable_result.clone() {
                        if let Some(copy_term) = stored_initialization_copy_term(&result, value) {
                            self.record_requirement(
                                copy_term,
                                GenericCapability::CopyConstructible,
                                value.span(),
                                GenericRequirementReason::StoredInitializationCopy {
                                    member: self.member,
                                },
                            );
                        }
                    }
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
                if let Some(block) = &statement.else_block {
                    self.visit_block(block);
                }
            }
            syntax::Statement::While(statement) => {
                self.visit_expression(&statement.condition);
                self.visit_block(&statement.body);
            }
            syntax::Statement::ForIn(statement) => {
                self.visit_iteration(statement);
            }
            syntax::Statement::Block(block) => self.visit_block(block),
            syntax::Statement::FieldAssignment(statement) => {
                self.visit_member_access(&statement.place);
                self.visit_expression(&statement.value);
                if let Some(term) = self.field_assignment_type(&statement.place) {
                    if self.member_assigns_fields() {
                        self.record_requirement(
                            &term,
                            GenericCapability::Assignable,
                            statement.equal_span,
                            GenericRequirementReason::Assignment {
                                member: self.member,
                            },
                        );
                    } else if let Some(copy_term) =
                        stored_initialization_copy_term(&term, &statement.value)
                    {
                        self.record_requirement(
                            copy_term,
                            GenericCapability::CopyConstructible,
                            statement.value.span(),
                            GenericRequirementReason::StoredInitializationCopy {
                                member: self.member,
                            },
                        );
                    }
                }
            }
            syntax::Statement::ObjectAssignment(statement) => {
                self.visit_expression(&statement.place);
                self.visit_expression(&statement.value);
                if self.expression_depends_on_parameter(&statement.value) {
                    if let syntax::Expression::Identifier(identifier) = &statement.place {
                        if !identifier.name.is_qualified() {
                            self.mark_binding_parameter_dependent(identifier.name.text.as_str());
                        }
                    }
                }
                if let Some(term) = self.type_of_expression(&statement.place) {
                    self.record_requirement(
                        &term,
                        GenericCapability::Assignable,
                        statement.equal_span,
                        GenericRequirementReason::Assignment {
                            member: self.member,
                        },
                    );
                }
            }
        }
    }

    fn visit_expression(&mut self, expression: &syntax::Expression) {
        match expression {
            syntax::Expression::Absent(_)
            | syntax::Expression::NumericLiteral(_)
            | syntax::Expression::ByteLiteral(_)
            | syntax::Expression::StringLiteral(_)
            | syntax::Expression::Boolean(_)
            | syntax::Expression::SelfValue(_) => {}
            syntax::Expression::Identifier(identifier) => self.resolve_identifier_value(identifier),
            syntax::Expression::Present(expression) => self.visit_expression(&expression.value),
            syntax::Expression::GenericTypeApplication(application) => {
                if let Some(target) = self.resolve_named_type_use(
                    &application.target,
                    ResolvedTemplateTypeUseContext::ConstructionTarget {
                        member: self.member,
                    },
                ) {
                    self.record_operation(
                        ResolvedTemplateDependentSelectionKind::Construction(
                            ResolvedTemplateConstructionMode::Inline,
                        ),
                        target,
                        None,
                        application.span,
                    );
                }
            }
            syntax::Expression::GenericStaticSelection(selection) => {
                if let Some(target) = self.resolve_named_type_use(
                    &selection.target,
                    ResolvedTemplateTypeUseContext::StaticSelectionTarget {
                        member: self.member,
                    },
                ) {
                    if let Some(parameter) = target.parameter() {
                        self.report_parameter_member(parameter, &selection.member, selection.span);
                    } else {
                        self.record_operation(
                            ResolvedTemplateDependentSelectionKind::StaticMember,
                            target,
                            Some(selection.member.text.to_string()),
                            selection.span,
                        );
                    }
                }
            }
            syntax::Expression::Unary(expression) => {
                self.visit_expression(&expression.operand);
                self.select_unary_operator(expression);
            }
            syntax::Expression::Binary(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
                self.select_binary_operator(expression);
            }
            syntax::Expression::Logical(expression) => {
                self.visit_expression(&expression.left);
                self.visit_expression(&expression.right);
            }
            syntax::Expression::Range(expression) => {
                self.visit_expression(&expression.lower);
                self.visit_expression(&expression.upper);
                let lower = self.type_of_expression(&expression.lower);
                let upper = self.type_of_expression(&expression.upper);
                if let (Some(lower), Some(upper)) = (lower, upper) {
                    if lower.semantically_eq(&upper) {
                        self.selections.push(ResolvedTemplateSelection::Range {
                            endpoint: lower,
                            endpoint_provenance: [
                                self.range_endpoint_provenance(&expression.lower),
                                self.range_endpoint_provenance(&expression.upper),
                            ],
                            span: expression.operator_span,
                        });
                    }
                }
            }
            syntax::Expression::TypeTest(expression) => {
                self.visit_expression(&expression.source);
                if let Some(target) = self.resolve_named_type_use(
                    &expression.target,
                    ResolvedTemplateTypeUseContext::TypeTestTarget {
                        member: self.member,
                    },
                ) {
                    self.record_operation(
                        ResolvedTemplateDependentSelectionKind::TypeTest,
                        target,
                        None,
                        expression.span,
                    );
                }
            }
            syntax::Expression::PresenceTest(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::Unwrap(expression) => self.visit_expression(&expression.source),
            syntax::Expression::PrimitiveCast(expression) => {
                self.visit_expression(&expression.source)
            }
            syntax::Expression::ObjectCast(expression) => {
                self.visit_expression(&expression.source);
                if let Some(mut target) = self.resolve_named_type_use(
                    &expression.target,
                    ResolvedTemplateTypeUseContext::CastTarget {
                        member: self.member,
                    },
                ) {
                    if matches!(
                        expression.target_mode,
                        syntax::ObjectCastTargetMode::Shared { .. }
                    ) {
                        target = ResolvedTemplateType {
                            span: target.span,
                            kind: ResolvedTemplateTypeKind::Shared(Box::new(target)),
                        };
                    }
                    self.record_operation(
                        ResolvedTemplateDependentSelectionKind::Cast,
                        target,
                        None,
                        expression.span,
                    );
                }
            }
            syntax::Expression::Allocation(expression) => {
                self.visit_call_arguments(&expression.arguments);
                if let Some(target) = self.resolve_named_type_use(
                    &expression.target,
                    ResolvedTemplateTypeUseContext::ConstructionTarget {
                        member: self.member,
                    },
                ) {
                    if let Some(parameter) = target.parameter() {
                        self.report_parameter_construction(parameter, expression.target.span);
                    } else {
                        if let syntax::CallArguments::Copy { copy_span, .. } = &expression.arguments
                        {
                            self.record_requirement(
                                &target,
                                GenericCapability::CopyConstructible,
                                *copy_span,
                                GenericRequirementReason::ExplicitCopyConstruction {
                                    member: self.member,
                                },
                            );
                        }
                        self.record_operation(
                            ResolvedTemplateDependentSelectionKind::Construction(
                                ResolvedTemplateConstructionMode::Shared,
                            ),
                            target,
                            None,
                            expression.span,
                        );
                    }
                }
            }
            syntax::Expression::OptionalBoxAllocation(expression) => {
                if let syntax::OptionalBoxInitializer::Value { value, .. } = &expression.initializer
                {
                    self.visit_expression(value);
                }
                self.resolve_type_use(
                    &expression.target,
                    ResolvedTemplateTypeUseContext::OptionalBoxTarget {
                        member: self.member,
                    },
                );
            }
            syntax::Expression::ArrayConstruction(expression) => {
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
                if let Some(array) = self.resolve_type_use(
                    &expression.array_type,
                    ResolvedTemplateTypeUseContext::ArrayConstructionTarget {
                        member: self.member,
                    },
                ) {
                    let ResolvedTemplateTypeKind::Array(element) = &array.kind else {
                        return;
                    };
                    match &expression.arguments {
                        syntax::ArrayConstructionArguments::Length { .. } => {
                            self.record_requirement(
                                element,
                                GenericCapability::DefaultConstructible,
                                element.span,
                                GenericRequirementReason::ArrayLengthConstruction {
                                    member: self.member,
                                },
                            );
                        }
                        syntax::ArrayConstructionArguments::Copy { copy_span, .. } => {
                            self.record_requirement(
                                &array,
                                GenericCapability::CopyConstructible,
                                *copy_span,
                                GenericRequirementReason::ExplicitArrayCopy {
                                    member: self.member,
                                },
                            );
                        }
                        syntax::ArrayConstructionArguments::Empty { .. } => {}
                        syntax::ArrayConstructionArguments::Elements(elements) => {
                            for source in &elements.elements {
                                if let Some(copy_term) =
                                    stored_initialization_copy_term(element, source)
                                {
                                    self.record_requirement(
                                        copy_term,
                                        GenericCapability::CopyConstructible,
                                        source.span(),
                                        GenericRequirementReason::StoredInitializationCopy {
                                            member: self.member,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            syntax::Expression::Call(expression) => {
                if let (
                    syntax::Expression::GenericTypeApplication(application),
                    syntax::CallArguments::Copy { copy_span, source },
                ) = (expression.callee.as_ref(), &expression.arguments)
                {
                    self.visit_expression(source);
                    if let Some(target) = self.resolve_named_type_use(
                        &application.target,
                        ResolvedTemplateTypeUseContext::ConstructionTarget {
                            member: self.member,
                        },
                    ) {
                        self.record_requirement(
                            &target,
                            GenericCapability::CopyConstructible,
                            *copy_span,
                            GenericRequirementReason::ExplicitCopyConstruction {
                                member: self.member,
                            },
                        );
                        self.record_operation(
                            ResolvedTemplateDependentSelectionKind::Construction(
                                ResolvedTemplateConstructionMode::Inline,
                            ),
                            target,
                            None,
                            application.span,
                        );
                    }
                    return;
                }
                if let syntax::Expression::Identifier(identifier) = expression.callee.as_ref() {
                    self.resolve_direct_call(identifier);
                } else {
                    self.visit_expression(&expression.callee);
                }
                self.visit_call_arguments(&expression.arguments);
            }
            syntax::Expression::Grouped(expression) => {
                self.visit_expression(&expression.expression)
            }
            syntax::Expression::MemberAccess(expression) => self.visit_member_access(expression),
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

    fn visit_member_access(&mut self, expression: &syntax::MemberAccessExpr) {
        self.visit_expression(&expression.receiver);
        if matches!(
            expression.receiver.as_ref(),
            syntax::Expression::SelfValue(_)
        ) {
            if let Some(member) = self.member_names.get(expression.member.text.as_str()) {
                self.selections
                    .push(ResolvedTemplateSelection::TemplateMember {
                        member: *member,
                        member_name: expression.member.text.to_string(),
                        span: expression.span,
                    });
            } else if !self.has_direct_base {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::UNKNOWN_MEMBER,
                        format!("unknown template member `{}`", expression.member.text),
                    )
                    .with_primary_label(
                        expression.member.span,
                        "no field or method with this name is declared",
                    ),
                );
            }
        }
        let parameter = match expression.operator {
            syntax::MemberAccessOperator::Dot { .. } => {
                self.parameter_of_expression(&expression.receiver)
            }
            syntax::MemberAccessOperator::Arrow { .. } => self
                .type_of_expression(&expression.receiver)
                .and_then(|receiver| match receiver.kind {
                    ResolvedTemplateTypeKind::Shared(target) => target.parameter(),
                    _ => None,
                }),
        };
        if let Some(parameter) = parameter {
            self.report_parameter_member(parameter, &expression.member, expression.span);
        }
    }

    fn resolve_identifier_value(&mut self, identifier: &syntax::IdentifierExpr) {
        if !identifier.name.is_qualified()
            && self.lookup_binding(identifier.name.text.as_str()).is_some()
        {
            return;
        }
        match self.lookup.select(&identifier.name, self.diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Function(function),
                ..
            }) => self.selections.push(ResolvedTemplateSelection::TopLevel {
                declaration: ResolvedTopLevelId::Function(function),
                span: identifier.span,
            }),
            TopLevelLookup::Found(symbol) => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::TOP_LEVEL_USED_AS_VALUE,
                    format!("`{}` is a declaration, not a value", identifier.name.text),
                )
                .with_primary_label(
                    identifier.span,
                    "use the declaration in an appropriate operation",
                )
                .with_secondary_label(symbol.name_span, "declared here"),
            ),
            TopLevelLookup::Missing => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::UNKNOWN_NAME,
                    format!("unknown name `{}`", identifier.name.text),
                )
                .with_primary_label(
                    identifier.span,
                    "no value with this name is visible in the template definition",
                ),
            ),
            TopLevelLookup::Diagnosed => {}
        }
    }

    fn resolve_direct_call(&mut self, identifier: &syntax::IdentifierExpr) {
        if !identifier.name.is_qualified()
            && self.lookup_binding(identifier.name.text.as_str()).is_some()
        {
            return;
        }
        if !identifier.name.is_qualified() {
            if let Some(parameter) = self.parameters.get(identifier.name.text.as_str()) {
                self.report_parameter_construction(parameter.id, identifier.span);
                return;
            }
        }
        match self.lookup.select(&identifier.name, self.diagnostics) {
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Function(function),
                ..
            }) => self.selections.push(ResolvedTemplateSelection::TopLevel {
                declaration: ResolvedTopLevelId::Function(function),
                span: identifier.span,
            }),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(class),
                ..
            }) => self.selections.push(ResolvedTemplateSelection::TopLevel {
                declaration: ResolvedTopLevelId::Class(class),
                span: identifier.span,
            }),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::ClassTemplate(_),
                name_span,
            }) => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::RAW_GENERIC_TYPE,
                    format!(
                        "generic class `{}` requires type arguments",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
            ),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                name_span,
            }) => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::INVALID_CALL_TARGET,
                    format!("interface `{}` is not callable", identifier.name.text),
                )
                .with_primary_label(identifier.span, "interfaces cannot be constructed")
                .with_secondary_label(name_span, "interface declared here"),
            ),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::InterfaceTemplate(_),
                name_span,
            }) => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::RAW_GENERIC_TYPE,
                    format!(
                        "generic interface `{}` requires type arguments",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
            ),
            TopLevelLookup::Missing => self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::UNKNOWN_NAME,
                    format!("unknown function or class `{}`", identifier.name.text),
                )
                .with_primary_label(
                    identifier.span,
                    "lookup uses the template's definition module",
                ),
            ),
            TopLevelLookup::Diagnosed => {}
        }
    }

    fn parameter_of_expression(&self, expression: &syntax::Expression) -> Option<TypeParameterId> {
        self.type_of_expression(expression)
            .as_ref()
            .and_then(ResolvedTemplateType::parameter)
    }

    fn report_parameter_member(
        &mut self,
        parameter: TypeParameterId,
        member: &syntax::Name,
        span: Span,
    ) {
        let mut candidates = Vec::new();
        for (bound_index, bound) in self
            .bounds
            .iter()
            .enumerate()
            .filter(|(_, bound)| bound.parameter == parameter)
        {
            match &bound.interface {
                ResolvedInterfaceType::Ordinary(interface_id) => {
                    let interface = self
                        .interfaces
                        .get(*interface_id)
                        .expect("resolved bounds reference interface declarations");
                    for requirement in &interface.requirements {
                        if requirement.name == member.text.as_str() {
                            candidates.push((
                                bound_index,
                                ResolvedTemplateBoundRequirement::Ordinary(requirement.id),
                                requirement.name_span,
                                None,
                            ));
                        }
                    }
                }
                ResolvedInterfaceType::TemplateApplication { template, .. } => {
                    let semantics = self
                        .interface_semantics
                        .get(*template)
                        .expect("resolved bounds reference interface template semantics");
                    for requirement in &semantics.requirements {
                        if requirement.name == member.text.as_str() {
                            candidates.push((
                                bound_index,
                                ResolvedTemplateBoundRequirement::Generic(requirement.id),
                                requirement.name_span,
                                Some(requirement.return_type.clone()),
                            ));
                        }
                    }
                }
            }
        }

        match candidates.as_slice() {
            [(bound, requirement, _, output)] => {
                self.selections
                    .push(ResolvedTemplateSelection::BoundMember {
                        parameter,
                        bound: *bound,
                        requirement: *requirement,
                        member_name: member.text.to_string(),
                        output: output.clone(),
                        span,
                    });
            }
            [] => {
                let parameter_declaration = self
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == parameter)
                    .expect("dependent receiver parameter belongs to its template");
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::UNCONSTRAINED_TYPE_PARAMETER_MEMBER,
                        format!(
                            "member `{}` is not authorized for type parameter `{}`",
                            member.text, parameter_declaration.name
                        ),
                    )
                    .with_primary_label(
                        member.span,
                        "no declared interface bound provides this member",
                    )
                    .with_secondary_label(
                        parameter_declaration.name_span,
                        "parameter declared here",
                    ),
                );
            }
            candidates => {
                let mut diagnostic = Diagnostic::error(
                    super::super::super::AMBIGUOUS_GENERIC_BOUND_MEMBER,
                    format!(
                        "member `{}` is provided by multiple interface bounds",
                        member.text
                    ),
                )
                .with_primary_label(member.span, "bound member selection is ambiguous");
                for (_, _, requirement_span, _) in candidates {
                    diagnostic = diagnostic.with_secondary_label(
                        *requirement_span,
                        "candidate interface requirement declared here",
                    );
                }
                self.diagnostics.push(diagnostic);
            }
        }
    }

    fn report_parameter_construction(&mut self, parameter: TypeParameterId, span: Span) {
        let declaration = self
            .parameters
            .iter()
            .find(|candidate| candidate.id == parameter)
            .expect("constructed parameter belongs to its template");
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::UNSUPPORTED_PARAMETER_CONSTRUCTION,
                format!(
                    "construction through type parameter `{}` is unsupported",
                    declaration.name
                ),
            )
            .with_primary_label(span, "generic classes do not support constructor bounds")
            .with_secondary_label(declaration.name_span, "parameter declared here"),
        );
    }

    fn record_operation(
        &mut self,
        kind: ResolvedTemplateDependentSelectionKind,
        target: ResolvedTemplateType,
        member_name: Option<String>,
        span: Span,
    ) {
        let selection = if type_depends_on_parameter(&target) {
            ResolvedTemplateSelection::ArgumentDependent {
                kind,
                target,
                member_name,
                span,
            }
        } else {
            ResolvedTemplateSelection::DefinitionSite {
                kind,
                target,
                member_name,
                span,
            }
        };
        self.selections.push(selection);
    }

    fn resolve_type_use(
        &mut self,
        syntax: &syntax::TypeSyntax,
        context: ResolvedTemplateTypeUseContext,
    ) -> Option<ResolvedTemplateType> {
        let resolved = TemplateTypeResolver::new(self.parameters, self.lookup, self.diagnostics)
            .resolve(syntax)?;
        self.type_uses.push(ResolvedTemplateTypeUse {
            context,
            type_term: resolved.clone(),
        });
        infer_type_construction(&resolved, self.requirements);
        Some(resolved)
    }

    fn resolve_named_type_use(
        &mut self,
        syntax: &syntax::NamedTypeSyntax,
        context: ResolvedTemplateTypeUseContext,
    ) -> Option<ResolvedTemplateType> {
        let resolved = TemplateTypeResolver::new(self.parameters, self.lookup, self.diagnostics)
            .resolve_named(syntax)?;
        self.type_uses.push(ResolvedTemplateTypeUse {
            context,
            type_term: resolved.clone(),
        });
        infer_type_construction(&resolved, self.requirements);
        Some(resolved)
    }

    fn lookup_binding(&self, name: &str) -> Option<&TemplateBinding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn mark_binding_parameter_dependent(&mut self, name: &str) {
        if let Some(binding) = self
            .scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
        {
            binding.depends_on_parameter = true;
        }
    }

    fn visit_iteration(&mut self, statement: &syntax::ForInStatement) {
        self.visit_expression(&statement.iterable);
        let annotation = statement.annotation.as_ref().and_then(|annotation| {
            self.resolve_type_use(
                &annotation.type_syntax,
                ResolvedTemplateTypeUseContext::IterationItemAnnotation {
                    member: self.member,
                },
            )
        });
        let Some(parameter) = self
            .type_of_expression(&statement.iterable)
            .as_ref()
            .and_then(ResolvedTemplateType::parameter)
        else {
            // A nondependent iterable is selected after this template closes
            // to an ordinary body. Only parameter-bound selection must be
            // frozen at definition site.
            return;
        };
        let Some(language_item) = self.iterable_language_item else {
            return;
        };
        let mut candidates = self
            .bounds
            .iter()
            .enumerate()
            .filter_map(|(bound, candidate)| {
                if candidate.parameter != parameter {
                    return None;
                }
                let ResolvedInterfaceType::TemplateApplication {
                    template,
                    arguments,
                } = &candidate.interface
                else {
                    return None;
                };
                if *template != language_item.template {
                    return None;
                }
                let [item, state] = arguments.as_slice() else {
                    return None;
                };
                Some((bound, candidate.interface_span, item.clone(), state.clone()))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(bound, _, _, _)| *bound);
        let unfiltered = candidates.clone();
        if let Some(annotation) = &annotation {
            candidates.retain(|(_, _, item, _)| item.semantically_eq(annotation));
        }

        let (bound, _, item, state) = match candidates.as_slice() {
            [(bound, span, item, state)] => (*bound, *span, item.clone(), state.clone()),
            [] if annotation.is_some() && !unfiltered.is_empty() => {
                let mut diagnostic = Diagnostic::error(
                    super::super::super::ITERATION_ITEM_TYPE_MISMATCH,
                    "the iteration item annotation matches no eligible generic bound",
                )
                .with_primary_label(
                    statement
                        .annotation
                        .as_ref()
                        .expect("a structural annotation was resolved")
                        .type_syntax
                        .span,
                    "exact item type required here",
                );
                for (_, span, _, _) in unfiltered {
                    diagnostic = diagnostic
                        .with_secondary_label(span, "candidate bound has a different item type");
                }
                self.diagnostics.push(diagnostic);
                return;
            }
            [] => {
                self.diagnostics.push(
                    Diagnostic::error(
                        super::super::super::MISSING_ITERABLE_APPLICATION,
                        "the generic iterable type has no canonical `Iterable` bound",
                    )
                    .with_primary_label(
                        statement.iterable.span(),
                        "declare an exact `std::iter::Iterable<Item, State>` bound",
                    ),
                );
                return;
            }
            candidates => {
                let mut diagnostic = Diagnostic::error(
                    super::super::super::AMBIGUOUS_ITERABLE_APPLICATION,
                    "multiple canonical `Iterable` bounds remain eligible",
                )
                .with_primary_label(
                    statement
                        .annotation
                        .as_ref()
                        .map_or(statement.iterable.span(), |annotation| {
                            annotation.type_syntax.span
                        }),
                    "generic iteration selection is ambiguous",
                );
                for (_, span, _, _) in candidates {
                    diagnostic =
                        diagnostic.with_secondary_label(*span, "candidate bound declared here");
                }
                self.diagnostics.push(diagnostic);
                return;
            }
        };

        self.selections.push(ResolvedTemplateSelection::Iteration {
            parameter,
            bound,
            item: item.clone(),
            state,
            iter_state: language_item.iter_state_requirement,
            iter_next: language_item.iter_next_requirement,
            span: statement.for_span,
        });
        self.scopes.push(HashMap::new());
        let depends_on_parameter = item.depends_on_parameter()
            || self.expression_depends_on_parameter(&statement.iterable);
        self.declare_binding(
            &statement.binding,
            item,
            depends_on_parameter,
            "iteration binding",
        );
        for statement in &statement.body.statements {
            self.visit_statement(statement);
        }
        self.scopes
            .pop()
            .expect("generic iteration body owns one lexical scope");
    }

    fn declare_binding(
        &mut self,
        name: &syntax::Name,
        ty: ResolvedTemplateType,
        depends_on_parameter: bool,
        binding_kind: &'static str,
    ) -> bool {
        let scope = self
            .scopes
            .last_mut()
            .expect("template body always has a lexical scope");
        if scope.contains_key(name.text.as_str()) {
            self.diagnostics.push(
                Diagnostic::error(
                    super::super::super::DUPLICATE_BINDING,
                    format!("duplicate {binding_kind} `{}`", name.text),
                )
                .with_primary_label(name.span, "redeclared here"),
            );
            return false;
        }
        scope.insert(
            name.text.to_string(),
            TemplateBinding {
                ty,
                depends_on_parameter,
            },
        );
        true
    }

    fn type_of_expression(&self, expression: &syntax::Expression) -> Option<ResolvedTemplateType> {
        match expression {
            syntax::Expression::NumericLiteral(literal) => Some(ResolvedTemplateType {
                kind: match literal.kind {
                    crate::literal::NumericLiteralKind::I64(_) => ResolvedTemplateTypeKind::I64,
                    crate::literal::NumericLiteralKind::U64(_) => ResolvedTemplateTypeKind::U64,
                    crate::literal::NumericLiteralKind::U8(_) => ResolvedTemplateTypeKind::U8,
                    crate::literal::NumericLiteralKind::F64 => ResolvedTemplateTypeKind::F64,
                },
                span: literal.span,
            }),
            syntax::Expression::ByteLiteral(literal) => Some(ResolvedTemplateType {
                kind: ResolvedTemplateTypeKind::U8,
                span: literal.span,
            }),
            syntax::Expression::Boolean(boolean) => Some(ResolvedTemplateType {
                kind: ResolvedTemplateTypeKind::Bool,
                span: boolean.span,
            }),
            syntax::Expression::Identifier(identifier) if !identifier.name.is_qualified() => self
                .lookup_binding(identifier.name.text.as_str())
                .map(|binding| binding.ty.clone()),
            syntax::Expression::Grouped(grouped) => self.type_of_expression(&grouped.expression),
            syntax::Expression::MemberAccess(access)
                if matches!(access.receiver.as_ref(), syntax::Expression::SelfValue(_)) =>
            {
                self.fields.get(access.member.text.as_str()).cloned()
            }
            syntax::Expression::BracketProjection(projection) => {
                let receiver = self.type_of_expression(&projection.receiver)?;
                let ResolvedTemplateTypeKind::Array(element) = receiver.kind else {
                    return None;
                };
                Some(*element)
            }
            syntax::Expression::Unwrap(unwrap) => {
                let source = self.type_of_expression(&unwrap.source)?;
                let ResolvedTemplateTypeKind::Optional(payload) = source.kind else {
                    return None;
                };
                Some(*payload)
            }
            syntax::Expression::Call(call) => match call.callee.as_ref() {
                syntax::Expression::MemberAccess(access)
                    if matches!(access.receiver.as_ref(), syntax::Expression::SelfValue(_)) =>
                {
                    self.member_results
                        .get(access.member.text.as_str())
                        .cloned()
                }
                syntax::Expression::MemberAccess(access) => {
                    self.selections
                        .iter()
                        .rev()
                        .find_map(|selection| match selection {
                            ResolvedTemplateSelection::BoundMember { span, output, .. }
                                if *span == access.span =>
                            {
                                output.clone()
                            }
                            _ => None,
                        })
                }
                _ => None,
            },
            syntax::Expression::Unary(unary) => self.operator_output(unary.span),
            syntax::Expression::Binary(binary) => self.operator_output(binary.span),
            syntax::Expression::Logical(_)
            | syntax::Expression::TypeTest(_)
            | syntax::Expression::PresenceTest(_) => Some(ResolvedTemplateType {
                kind: ResolvedTemplateTypeKind::Bool,
                span: expression.span(),
            }),
            syntax::Expression::PrimitiveCast(cast) => Some(ResolvedTemplateType {
                kind: match cast.target {
                    syntax::PrimitiveType::I64 => ResolvedTemplateTypeKind::I64,
                    syntax::PrimitiveType::U64 => ResolvedTemplateTypeKind::U64,
                    syntax::PrimitiveType::U8 => ResolvedTemplateTypeKind::U8,
                    syntax::PrimitiveType::F64 => ResolvedTemplateTypeKind::F64,
                    syntax::PrimitiveType::Bool => ResolvedTemplateTypeKind::Bool,
                },
                span: cast.span,
            }),
            _ => None,
        }
    }

    fn field_assignment_type(
        &self,
        place: &syntax::MemberAccessExpr,
    ) -> Option<ResolvedTemplateType> {
        matches!(place.receiver.as_ref(), syntax::Expression::SelfValue(_))
            .then(|| self.fields.get(place.member.text.as_str()).cloned())
            .flatten()
    }

    fn member_assigns_fields(&self) -> bool {
        self.field_writes_assign
    }

    fn record_requirement(
        &mut self,
        term: &ResolvedTemplateType,
        capability: GenericCapability,
        origin: Span,
        reason: GenericRequirementReason,
    ) {
        push(self.requirements, term, capability, origin, reason);
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

fn type_depends_on_parameter(term: &ResolvedTemplateType) -> bool {
    term.depends_on_parameter()
}
