//! Source-shape tests for the contract in `docs/language/GRAMMAR.md`.

use super::*;
use crate::{
    lexer::lex, literal::NumericLiteralKind, source::SourceDatabase, syntax::dump_ast,
    test_support::parse_source,
};

fn parse_text(text: &str) -> (SourceDatabase, ParseOutput) {
    parse_source(text)
}

fn function(ast: &CompilationUnit, index: usize) -> &FunctionDecl {
    let TopLevelDeclaration::Function(function) = &ast.declarations[index] else {
        panic!("expected a local function definition");
    };
    function
}

fn return_value(function: &FunctionDecl) -> &Expression {
    let Statement::Return(statement) = function.body.statements.last().unwrap() else {
        panic!("expected final return statement");
    };
    statement.value.as_ref().expect("expected a return value")
}

mod alias_parameters;
mod arrays;
mod comparisons;
mod conditionals;
mod declarations;
mod dumps;
mod expressions;
mod integer_casts;
mod interfaces;
mod intrinsics;
mod modules;
mod objects;
mod optional_values;
mod recovery;
mod shared_ownership;
mod static_methods;
mod type_operations;
