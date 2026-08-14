//! Source-shape tests for the contract in `docs/language/GRAMMAR.md`.

use super::*;
use crate::{
    lexer::lex,
    literal::{IntegerRadix, NumericLiteralKind},
    source::SourceDatabase,
    syntax::dump_ast,
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
mod bitwise_operators;
mod bracket_projections;
mod comparisons;
mod conditionals;
mod declarations;
mod dumps;
mod eager_boolean_operators;
mod expressions;
mod generic_classes;
mod integer_division;
mod interfaces;
mod intrinsics;
mod modules;
mod objects;
mod optional_values;
mod primitive_casts;
mod recovery;
mod shared_ownership;
mod shifts;
mod short_circuit_boolean;
mod static_fields;
mod static_methods;
mod type_operations;
mod while_loops;
