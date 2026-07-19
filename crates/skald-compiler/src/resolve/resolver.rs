//! Two-pass declaration collection and lexical name resolution.

use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
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
        let mut functions = Vec::with_capacity(declarations.len());
        for (id, ast_index) in declarations {
            let declaration = &self.ast.functions[ast_index];
            functions.push(
                FunctionResolver::new(id, &self.functions_by_name, &mut self.diagnostics)
                    .resolve(declaration),
            );
        }

        ResolveOutput {
            program: ResolvedProgram {
                functions: FunctionTable::new(functions),
                entry_function,
                span: self.ast.span,
            },
            diagnostics: self.diagnostics,
        }
    }

    fn collect_functions(&mut self) {
        for (ast_index, function) in self.ast.functions.iter().enumerate() {
            if let Some(previous) = self.functions_by_name.get(&function.name.text) {
                self.diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_FUNCTION,
                        format!("duplicate function `{}`", function.name.text),
                    )
                    .with_primary_label(function.name.span, "redeclared here")
                    .with_secondary_label(previous.name_span, "first declared here"),
                );
                continue;
            }

            let id = FunctionId::new(self.declarations.len());
            self.functions_by_name.insert(
                function.name.text.clone(),
                FunctionSymbol {
                    id,
                    name_span: function.name.span,
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

    fn resolve(mut self, function: &syntax::FunctionDecl) -> ResolvedFunction {
        for parameter in &function.parameters {
            self.declare_parameter(parameter);
        }
        let body = self.resolve_block(&function.body, false);

        ResolvedFunction {
            id: self.function_id,
            name: function.name.text.clone(),
            name_span: function.name.span,
            parameters: self.parameters,
            return_type: resolve_type(&function.return_type),
            locals: self.locals,
            body,
            span: function.span,
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
                let value = self.resolve_expression(&statement.value)?;
                Some(ResolvedStatement::Return(ResolvedReturn {
                    value,
                    span: statement.span,
                }))
            }
            syntax::Statement::Block(block) => {
                Some(ResolvedStatement::Block(self.resolve_block(block, true)))
            }
        }
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
            syntax::Expression::Integer(integer) => {
                Some(ResolvedExpression::Integer(ResolvedIntegerExpr {
                    spelling: integer.spelling.clone(),
                    span: integer.span,
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
                        "function `{}` cannot be used as a value in the first vertical slice",
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
                Diagnostic::error(
                    INVALID_CALL_TARGET,
                    "invalid direct-call target in the first vertical slice",
                )
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
        },
        span: type_syntax.span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lexer::lex,
        source::SourceDatabase,
        syntax::{parse, Statement},
    };

    use crate::resolve::dump_resolved;

    fn resolve_text(text: &str) -> ResolveOutput {
        let mut sources = SourceDatabase::new();
        let source_id = sources.add("test.ska", text);
        let source = sources.get(source_id).unwrap();
        let lexed = lex(source);
        assert!(lexed.diagnostics.is_empty(), "test source must lex cleanly");
        let parsed = parse(source, &lexed.tokens);
        assert!(
            parsed.diagnostics.is_empty(),
            "test source must parse cleanly"
        );
        resolve(&parsed.ast)
    }

    fn local_initializer(statement: &ResolvedStatement) -> &ResolvedExpression {
        let ResolvedStatement::Local(local) = statement else {
            panic!("expected local declaration");
        };
        &local.initializer
    }

    fn return_value(statement: &ResolvedStatement) -> &ResolvedExpression {
        let ResolvedStatement::Return(statement) = statement else {
            panic!("expected return statement");
        };
        &statement.value
    }

    #[test]
    fn collects_functions_before_resolving_forward_calls() {
        let output = resolve_text(concat!(
            "fn main() -> i64 { return twice(21); }\n",
            "fn twice(value: i64) -> i64 { return value * 2; }\n",
        ));

        assert!(!output.has_errors());
        assert_eq!(output.program.functions.len(), 2);
        assert_eq!(output.program.entry_function.unwrap().index(), 0);

        let main = output.program.functions.iter().next().unwrap();
        let ResolvedExpression::DirectCall(call) = return_value(&main.body.statements[0]) else {
            panic!("expected a resolved direct call");
        };
        assert_eq!(call.function.index(), 1);
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn assigns_dense_owner_qualified_ids_in_source_order() {
        let output = resolve_text(concat!(
            "fn add(left: i64, right: i64) -> i64 {\n",
            "  var first: i64 = left;\n",
            "  { var second: i64 = right; return second; }\n",
            "  return first;\n",
            "}\n",
        ));
        let function = output.program.functions.iter().next().unwrap();

        assert_eq!(function.id.index(), 0);
        assert_eq!(function.parameters[0].id.index(), 0);
        assert_eq!(function.parameters[1].id.index(), 1);
        assert_eq!(function.locals[0].id.index(), 0);
        assert_eq!(function.locals[1].id.index(), 1);
        assert_eq!(function.locals[1].id.function(), function.id);
        assert_eq!(
            function.parameter(function.parameters[1].id).unwrap().name,
            "right"
        );
        assert_eq!(function.local(function.locals[0].id).unwrap().name, "first");
    }

    #[test]
    fn nested_blocks_shadow_and_then_restore_outer_bindings() {
        let output = resolve_text(concat!(
            "fn main(value: i64) -> i64 {\n",
            "  var result: i64 = value;\n",
            "  { var result: i64 = 2; return result; }\n",
            "  return result;\n",
            "}\n",
        ));
        assert!(!output.has_errors());
        let function = output.program.functions.iter().next().unwrap();
        assert_eq!(function.locals.len(), 2);

        let ResolvedExpression::Binding(initial_value) =
            local_initializer(&function.body.statements[0])
        else {
            panic!("outer initializer must resolve to the parameter");
        };
        assert_eq!(
            initial_value.binding,
            BindingId::Parameter(function.parameters[0].id)
        );

        let ResolvedStatement::Block(nested) = &function.body.statements[1] else {
            panic!("expected nested block");
        };
        let ResolvedExpression::Binding(nested_value) = return_value(&nested.statements[1]) else {
            panic!("nested return must resolve to a local");
        };
        assert_eq!(
            nested_value.binding,
            BindingId::Local(function.locals[1].id)
        );

        let ResolvedExpression::Binding(outer_value) = return_value(&function.body.statements[2])
        else {
            panic!("outer return must resolve to a local");
        };
        assert_eq!(outer_value.binding, BindingId::Local(function.locals[0].id));
    }

    #[test]
    fn diagnoses_duplicate_functions_and_keeps_the_first() {
        let output = resolve_text(concat!(
            "fn same() -> i64 { return 1; }\n",
            "fn same() -> i64 { return 2; }\n",
            "fn other() -> i64 { return same(); }\n",
        ));

        assert!(output.has_errors());
        assert_eq!(output.program.functions.len(), 2);
        assert_eq!(
            output.program.functions.iter().nth(1).unwrap().id.index(),
            1
        );
        let diagnostic = output.diagnostics.iter().next().unwrap();
        assert_eq!(diagnostic.code, DUPLICATE_FUNCTION);
        assert_eq!(diagnostic.labels.len(), 2);
    }

    #[test]
    fn diagnoses_duplicate_parameters_and_outer_block_locals() {
        let output = resolve_text(concat!(
            "fn main(value: i64, value: i64) -> i64 {\n",
            "  var value: i64 = 1;\n",
            "  return value;\n",
            "}\n",
        ));
        let function = output.program.functions.iter().next().unwrap();

        assert_eq!(output.diagnostics.len(), 2);
        assert!(output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DUPLICATE_BINDING));
        assert_eq!(function.parameters.len(), 1);
        assert!(function.locals.is_empty());
        let ResolvedExpression::Binding(binding) = return_value(&function.body.statements[0])
        else {
            panic!("return must resolve to the first parameter");
        };
        assert_eq!(
            binding.binding,
            BindingId::Parameter(function.parameters[0].id)
        );
    }

    #[test]
    fn local_is_not_visible_in_its_own_initializer_but_is_visible_afterward() {
        let output = resolve_text(concat!(
            "fn main() -> i64 {\n",
            "  var value: i64 = value;\n",
            "  return value;\n",
            "}\n",
        ));
        let function = output.program.functions.iter().next().unwrap();

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics.iter().next().unwrap().code, UNKNOWN_NAME);
        assert_eq!(function.locals.len(), 1);
        assert_eq!(function.body.statements.len(), 1);
        let ResolvedExpression::Binding(binding) = return_value(&function.body.statements[0])
        else {
            panic!("later use must resolve to the local");
        };
        assert_eq!(binding.binding, BindingId::Local(function.locals[0].id));
    }

    #[test]
    fn reports_multiple_unknown_names_without_stopping() {
        let output = resolve_text("fn main() -> i64 { var value: i64 = first; return second; }");

        assert_eq!(output.diagnostics.len(), 2);
        assert!(output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == UNKNOWN_NAME));
    }

    #[test]
    fn local_binding_shadows_a_function_as_a_call_target() {
        let output = resolve_text(concat!(
            "fn target() -> i64 { return 1; }\n",
            "fn main() -> i64 {\n",
            "  var target: i64 = 2;\n",
            "  return target();\n",
            "}\n",
        ));

        assert_eq!(output.diagnostics.len(), 1);
        let diagnostic = output.diagnostics.iter().next().unwrap();
        assert_eq!(diagnostic.code, INVALID_CALL_TARGET);
        assert_eq!(diagnostic.labels.len(), 2);
    }

    #[test]
    fn rejects_non_identifier_and_unknown_call_targets() {
        let output = resolve_text(concat!(
            "fn target() -> i64 { return 1; }\n",
            "fn main() -> i64 {\n",
            "  var one: i64 = (target)();\n",
            "  return missing();\n",
            "}\n",
        ));

        let codes: Vec<_> = output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(codes, vec![INVALID_CALL_TARGET, UNKNOWN_NAME]);
    }

    #[test]
    fn function_name_without_a_call_is_not_a_value() {
        let output = resolve_text(concat!(
            "fn target() -> i64 { return 1; }\n",
            "fn main() -> i64 { return target; }\n",
        ));

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            FUNCTION_USED_AS_VALUE
        );
    }

    #[test]
    fn resolved_dump_is_deterministic_and_exposes_only_ids_at_uses() {
        let output = resolve_text("fn main(value: i64) -> i64 { return value; }");

        assert_eq!(
            dump_resolved(&output.program),
            concat!(
                "ResolvedProgram @0..44\n",
                "  Entry f0\n",
                "  Functions\n",
                "    Function f0 \"main\" @0..44\n",
                "      Parameters\n",
                "        Parameter f0:p0 \"value\" @8..18\n",
                "          Type I64 @15..18\n",
                "      ReturnType\n",
                "        Type I64 @23..26\n",
                "      Locals\n",
                "      Block @27..44\n",
                "        Return @29..42\n",
                "          Binding f0:p0 @36..41\n",
            )
        );
    }

    #[test]
    fn parsed_source_ast_still_contains_names_before_resolution() {
        // This compile-time shape check documents the phase boundary: M3 reads
        // source names, while resolved uses are represented only by BindingId
        // or FunctionId.
        let mut sources = SourceDatabase::new();
        let source_id = sources.add("test.ska", "fn main() -> i64 { return name; }");
        let source = sources.get(source_id).unwrap();
        let tokens = lex(source).tokens;
        let ast = parse(source, &tokens).ast;
        let Statement::Return(statement) = &ast.functions[0].body.statements[0] else {
            panic!("expected return");
        };
        assert!(matches!(statement.value, syntax::Expression::Identifier(_)));
    }
}
