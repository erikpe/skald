//! Two-pass declaration collection and lexical name resolution.

use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{BindingId, FunctionId, LocalId, ParameterId},
    source::Span,
    syntax,
};

use super::ir::*;

pub const DUPLICATE_FUNCTION: &str = "RES001";
pub const DUPLICATE_BINDING: &str = "RES002";
pub const UNKNOWN_NAME: &str = "RES003";
pub const INVALID_CALL_TARGET: &str = "RES004";
pub const FUNCTION_USED_AS_VALUE: &str = "RES005";

#[derive(Debug)]
pub struct ResolveOutput {
    pub program: ResolvedProgram,
    pub diagnostics: Diagnostics,
}

impl ResolveOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

/// Resolves a parsed single-file compilation unit.
///
/// This phase should only feed later phases when it returns no errors. It still
/// returns a partial resolved program on failure so diagnostics and tests can
/// inspect successful declarations without re-running resolution.
pub fn resolve(ast: &syntax::CompilationUnit) -> ResolveOutput {
    Resolver::new(ast).resolve()
}

#[derive(Clone, Copy)]
struct FunctionSymbol {
    id: FunctionId,
    name_span: Span,
}

struct Resolver<'ast> {
    ast: &'ast syntax::CompilationUnit,
    functions_by_name: HashMap<String, FunctionSymbol>,
    declarations: Vec<(FunctionId, usize)>,
    diagnostics: Diagnostics,
}

impl<'ast> Resolver<'ast> {
    fn new(ast: &'ast syntax::CompilationUnit) -> Self {
        Self {
            ast,
            functions_by_name: HashMap::new(),
            declarations: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    fn resolve(mut self) -> ResolveOutput {
        self.collect_functions();

        let entry_function = self.functions_by_name.get("main").map(|symbol| symbol.id);
        let declarations = self.declarations.clone();
        let mut function_declarations = Vec::with_capacity(declarations.len());
        let mut function_definitions = Vec::with_capacity(declarations.len());
        for (id, ast_index) in declarations {
            let resolver =
                FunctionResolver::new(id, &self.functions_by_name, &mut self.diagnostics);
            match &self.ast.declarations[ast_index] {
                syntax::TopLevelDeclaration::Function(function) => {
                    let (declaration, definition) = resolver.resolve_definition(function);
                    function_declarations.push(declaration);
                    function_definitions.push(Some(definition));
                }
                syntax::TopLevelDeclaration::ExternalFunction(function) => {
                    function_declarations.push(resolver.resolve_external(function));
                    function_definitions.push(None);
                }
            }
        }

        ResolveOutput {
            program: ResolvedProgram {
                declarations: ResolvedFunctionDeclarationTable::new(function_declarations),
                definitions: ResolvedFunctionDefinitionTable::new(function_definitions),
                entry_function,
                span: self.ast.span,
            },
            diagnostics: self.diagnostics,
        }
    }

    fn collect_functions(&mut self) {
        for (ast_index, declaration) in self.ast.declarations.iter().enumerate() {
            let name = declaration.name();
            if let Some(previous) = self.functions_by_name.get(&name.text) {
                self.diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_FUNCTION,
                        format!("duplicate function `{}`", name.text),
                    )
                    .with_primary_label(name.span, "redeclared here")
                    .with_secondary_label(previous.name_span, "first declared here"),
                );
                continue;
            }

            let id = FunctionId::new(self.declarations.len());
            self.functions_by_name.insert(
                name.text.clone(),
                FunctionSymbol {
                    id,
                    name_span: name.span,
                },
            );
            self.declarations.push((id, ast_index));
        }
    }
}

#[derive(Clone, Copy)]
struct BindingSymbol {
    id: BindingId,
    name_span: Span,
}

struct FunctionResolver<'program> {
    function_id: FunctionId,
    functions_by_name: &'program HashMap<String, FunctionSymbol>,
    diagnostics: &'program mut Diagnostics,
    scopes: Vec<HashMap<String, BindingSymbol>>,
    parameters: Vec<ResolvedParameter>,
    locals: Vec<ResolvedLocal>,
}

impl<'program> FunctionResolver<'program> {
    fn new(
        function_id: FunctionId,
        functions_by_name: &'program HashMap<String, FunctionSymbol>,
        diagnostics: &'program mut Diagnostics,
    ) -> Self {
        Self {
            function_id,
            functions_by_name,
            diagnostics,
            scopes: vec![HashMap::new()],
            parameters: Vec::new(),
            locals: Vec::new(),
        }
    }

    fn resolve_definition(
        mut self,
        function: &syntax::FunctionDecl,
    ) -> (ResolvedFunctionDeclaration, ResolvedFunctionDefinition) {
        self.resolve_parameters(&function.parameters);
        let body = self.resolve_block(&function.body, false);

        (
            ResolvedFunctionDeclaration {
                id: self.function_id,
                name: function.name.text.clone(),
                name_span: function.name.span,
                parameters: self.parameters,
                return_type: resolve_type(&function.return_type),
                linkage: ResolvedFunctionLinkage::Internal,
                span: function.span,
            },
            ResolvedFunctionDefinition {
                function: self.function_id,
                locals: self.locals,
                body,
                span: function.span,
            },
        )
    }

    fn resolve_external(
        mut self,
        function: &syntax::ExternalFunctionDecl,
    ) -> ResolvedFunctionDeclaration {
        self.resolve_parameters(&function.parameters);
        ResolvedFunctionDeclaration {
            id: self.function_id,
            name: function.name.text.clone(),
            name_span: function.name.span,
            parameters: self.parameters,
            return_type: resolve_type(&function.return_type),
            linkage: ResolvedFunctionLinkage::External {
                symbol: function.name.text.clone(),
            },
            span: function.span,
        }
    }

    fn resolve_parameters(&mut self, parameters: &[syntax::Parameter]) {
        for parameter in parameters {
            self.declare_parameter(parameter);
        }
    }

    fn declare_parameter(&mut self, parameter: &syntax::Parameter) {
        let id = ParameterId::new(self.function_id, self.parameters.len());
        let symbol = BindingSymbol {
            id: BindingId::Parameter(id),
            name_span: parameter.name.span,
        };
        if !self.declare_binding(&parameter.name.text, symbol, "parameter") {
            return;
        }

        self.parameters.push(ResolvedParameter {
            id,
            name: parameter.name.text.clone(),
            name_span: parameter.name.span,
            type_syntax: resolve_type(&parameter.type_syntax),
            span: parameter.span,
        });
    }

    fn resolve_block(&mut self, block: &syntax::Block, nested: bool) -> ResolvedBlock {
        if nested {
            self.scopes.push(HashMap::new());
        }

        let statements = block
            .statements
            .iter()
            .filter_map(|statement| self.resolve_statement(statement))
            .collect();

        if nested {
            self.scopes
                .pop()
                .expect("nested block must have a lexical scope");
        }

        ResolvedBlock {
            statements,
            span: block.span,
        }
    }

    fn resolve_statement(&mut self, statement: &syntax::Statement) -> Option<ResolvedStatement> {
        match statement {
            syntax::Statement::Local(local) => {
                self.resolve_local(local).map(ResolvedStatement::Local)
            }
            syntax::Statement::Return(statement) => {
                let value = match &statement.value {
                    Some(value) => Some(self.resolve_expression(value)?),
                    None => None,
                };
                Some(ResolvedStatement::Return(ResolvedReturn {
                    value,
                    span: statement.span,
                }))
            }
            syntax::Statement::Expression(statement) => {
                let expression = self.resolve_expression(&statement.expression)?;
                Some(ResolvedStatement::Expression(ResolvedExpressionStatement {
                    expression,
                    span: statement.span,
                }))
            }
            syntax::Statement::Conditional(statement) => self
                .resolve_conditional(statement)
                .map(ResolvedStatement::Conditional),
            syntax::Statement::Block(block) => {
                Some(ResolvedStatement::Block(self.resolve_block(block, true)))
            }
        }
    }

    fn resolve_conditional(
        &mut self,
        conditional: &syntax::ConditionalStatement,
    ) -> Option<ResolvedConditional> {
        let source_arms = std::iter::once(&conditional.if_arm).chain(&conditional.elif_arms);
        let mut arms = Vec::with_capacity(1 + conditional.elif_arms.len());
        let mut valid = true;
        for arm in source_arms {
            // Conditions deliberately resolve before entering their arm's
            // child scope. An earlier arm can never leak bindings into a later
            // condition or body.
            let condition = self.resolve_expression(&arm.condition);
            let body = self.resolve_block(&arm.body, true);
            match condition {
                Some(condition) => arms.push(ResolvedConditionalArm {
                    condition,
                    body,
                    span: arm.span,
                }),
                None => valid = false,
            }
        }
        let else_block = conditional
            .else_block
            .as_ref()
            .map(|block| self.resolve_block(block, true));

        valid.then_some(ResolvedConditional {
            arms,
            else_block,
            span: conditional.span,
        })
    }

    fn resolve_local(&mut self, local: &syntax::LocalDecl) -> Option<ResolvedLocalDecl> {
        // The initializer is resolved before introducing the binding, matching
        // source-order visibility and preventing self-reference.
        let initializer = self.resolve_expression(&local.initializer);
        let id = LocalId::new(self.function_id, self.locals.len());
        let symbol = BindingSymbol {
            id: BindingId::Local(id),
            name_span: local.name.span,
        };
        let declared = self.declare_binding(&local.name.text, symbol, "local binding");

        if declared {
            self.locals.push(ResolvedLocal {
                id,
                name: local.name.text.clone(),
                name_span: local.name.span,
                type_syntax: resolve_type(&local.type_syntax),
                span: local.span,
            });
        }

        match (declared, initializer) {
            (true, Some(initializer)) => Some(ResolvedLocalDecl {
                local: id,
                initializer,
                span: local.span,
            }),
            _ => None,
        }
    }

    fn resolve_expression(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedExpression> {
        match expression {
            syntax::Expression::Identifier(identifier) => self.resolve_identifier(identifier),
            syntax::Expression::NumericLiteral(literal) => Some(
                ResolvedExpression::NumericLiteral(ResolvedNumericLiteralExpr {
                    kind: literal.kind,
                    spelling: literal.spelling.clone(),
                    span: literal.span,
                }),
            ),
            syntax::Expression::Boolean(boolean) => {
                Some(ResolvedExpression::Boolean(ResolvedBooleanExpr {
                    value: boolean.value,
                    span: boolean.span,
                }))
            }
            syntax::Expression::Unary(unary) => {
                let operand = self.resolve_expression(&unary.operand)?;
                Some(ResolvedExpression::Unary(ResolvedUnaryExpr {
                    operator: match unary.operator {
                        syntax::UnaryOperator::Negate => ResolvedUnaryOperator::Negate,
                    },
                    operator_span: unary.operator_span,
                    operand: Box::new(operand),
                    span: unary.span,
                }))
            }
            syntax::Expression::Binary(binary) => {
                let left = self.resolve_expression(&binary.left);
                let right = self.resolve_expression(&binary.right);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        Some(ResolvedExpression::Binary(ResolvedBinaryExpr {
                            left: Box::new(left),
                            operator: match binary.operator {
                                syntax::BinaryOperator::Add => ResolvedBinaryOperator::Add,
                                syntax::BinaryOperator::Subtract => {
                                    ResolvedBinaryOperator::Subtract
                                }
                                syntax::BinaryOperator::Multiply => {
                                    ResolvedBinaryOperator::Multiply
                                }
                            },
                            operator_span: binary.operator_span,
                            right: Box::new(right),
                            span: binary.span,
                        }))
                    }
                    _ => None,
                }
            }
            syntax::Expression::Call(call) => self.resolve_call(call),
            syntax::Expression::Grouped(grouped) => {
                let expression = self.resolve_expression(&grouped.expression)?;
                Some(ResolvedExpression::Grouped(ResolvedGroupedExpr {
                    expression: Box::new(expression),
                    span: grouped.span,
                }))
            }
        }
    }

    fn resolve_identifier(
        &mut self,
        identifier: &syntax::IdentifierExpr,
    ) -> Option<ResolvedExpression> {
        if let Some(symbol) = self.lookup_binding(&identifier.name.text) {
            return Some(ResolvedExpression::Binding(ResolvedBindingExpr {
                binding: symbol.id,
                span: identifier.span,
            }));
        }

        if self.functions_by_name.contains_key(&identifier.name.text) {
            self.diagnostics.push(
                Diagnostic::error(
                    FUNCTION_USED_AS_VALUE,
                    format!(
                        "function `{}` cannot be used as a value",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "call the function with `(...)`"),
            );
        } else {
            self.report_unknown(&identifier.name.text, identifier.span, "unknown name");
        }
        None
    }

    fn resolve_call(&mut self, call: &syntax::CallExpr) -> Option<ResolvedExpression> {
        let target = self.resolve_call_target(&call.callee);
        let mut arguments = Vec::with_capacity(call.arguments.len());
        let mut arguments_valid = true;
        for argument in &call.arguments {
            match self.resolve_expression(argument) {
                Some(argument) => arguments.push(argument),
                None => arguments_valid = false,
            }
        }

        match (target, arguments_valid) {
            (Some(function), true) => {
                Some(ResolvedExpression::DirectCall(ResolvedDirectCallExpr {
                    function,
                    callee_span: call.callee.span(),
                    arguments,
                    span: call.span,
                }))
            }
            _ => None,
        }
    }

    fn resolve_call_target(&mut self, callee: &syntax::Expression) -> Option<FunctionId> {
        let syntax::Expression::Identifier(identifier) = callee else {
            self.diagnostics.push(
                Diagnostic::error(INVALID_CALL_TARGET, "call target must be a function name")
                    .with_primary_label(callee.span(), "expected a function name here"),
            );
            return None;
        };

        if let Some(binding) = self.lookup_binding(&identifier.name.text) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CALL_TARGET,
                    format!("binding `{}` is not callable", identifier.name.text),
                )
                .with_primary_label(identifier.span, "called here")
                .with_secondary_label(binding.name_span, "binding declared here"),
            );
            return None;
        }

        if let Some(function) = self.functions_by_name.get(&identifier.name.text) {
            return Some(function.id);
        }

        self.report_unknown(&identifier.name.text, identifier.span, "unknown function");
        None
    }

    fn declare_binding(
        &mut self,
        name: &str,
        symbol: BindingSymbol,
        binding_kind: &'static str,
    ) -> bool {
        let scope = self
            .scopes
            .last_mut()
            .expect("function resolver must always have an active scope");
        if let Some(previous) = scope.get(name) {
            self.diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_BINDING,
                    format!("duplicate {binding_kind} `{name}`"),
                )
                .with_primary_label(symbol.name_span, "redeclared here")
                .with_secondary_label(previous.name_span, "first declared here"),
            );
            return false;
        }

        scope.insert(name.to_owned(), symbol);
        true
    }

    fn lookup_binding(&self, name: &str) -> Option<BindingSymbol> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn report_unknown(&mut self, name: &str, span: Span, kind: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(UNKNOWN_NAME, format!("{kind} `{name}`"))
                .with_primary_label(span, "not declared in this scope"),
        );
    }
}

fn resolve_type(type_syntax: &syntax::TypeSyntax) -> ResolvedType {
    ResolvedType {
        kind: match type_syntax.kind {
            syntax::TypeKind::I64 => ResolvedTypeKind::I64,
            syntax::TypeKind::U64 => ResolvedTypeKind::U64,
            syntax::TypeKind::U8 => ResolvedTypeKind::U8,
            syntax::TypeKind::F64 => ResolvedTypeKind::F64,
            syntax::TypeKind::Bool => ResolvedTypeKind::Bool,
            syntax::TypeKind::Unit => ResolvedTypeKind::Unit,
        },
        span: type_syntax.span,
    }
}
