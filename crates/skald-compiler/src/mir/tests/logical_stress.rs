//! Bounded-growth and determinism coverage for large logical-expression CFGs.

use super::logical_fixtures::function_id_from_mir;
use super::*;
use crate::{
    backend::Target, syntax::MAX_LOGICAL_EXPRESSION_DEPTH,
    test_support::emit_assembly_without_runtime_trace as emit_assembly,
};

fn mixed_chain(terms: usize, operand: impl Fn(usize) -> String) -> String {
    let mut expression = operand(0);
    for index in 1..terms {
        let operator = if index % 2 == 0 { "&&" } else { "||" };
        expression.push_str(&format!(" {operator} {}", operand(index)));
    }
    expression
}

fn nested_mixed_expression(depth: usize) -> String {
    let mut expression = "true".to_owned();
    for index in 0..depth {
        let operator = if index % 2 == 0 { "&&" } else { "||" };
        let left = if index % 3 == 0 { "false" } else { "true" };
        expression = format!("{left} {operator} ({expression})");
    }
    expression
}

fn logical_definition(program: &MirProgram) -> &MirFunctionDefinition {
    program
        .definitions
        .get(FunctionId::new(0))
        .expect("stress fixture must define `evaluate` first")
}

fn assert_linear_chain_shape(program: &MirProgram, expression_count: usize) {
    let definition = logical_definition(program);
    assert_eq!(definition.body.logical_expressions.len(), expression_count);
    assert_eq!(definition.body.path_conditions.len(), expression_count);
    assert!(
        definition.body.blocks.len() <= expression_count * 12 + 8,
        "{} logical expressions produced {} blocks",
        expression_count,
        definition.body.blocks.len()
    );
    assert!(
        definition.storage.len() <= expression_count * 3 + 4,
        "{} logical expressions produced {} storage entries",
        expression_count,
        definition.storage.len()
    );
}

#[test]
fn long_and_deep_mixed_source_have_bounded_deterministic_graph_growth() {
    let long_terms = MAX_LOGICAL_EXPRESSION_DEPTH + 1;
    let long_expression = mixed_chain(long_terms, |index| {
        if index % 3 == 0 {
            "false".to_owned()
        } else {
            "true".to_owned()
        }
    });
    let nested_depth = MAX_LOGICAL_EXPRESSION_DEPTH / 2 + 1;
    let nested_expression = nested_mixed_expression(nested_depth);
    let source = format!(
        "fn evaluate() -> bool {{ return {long_expression}; }}\n\
         fn nested() -> bool {{ return {nested_expression}; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    );

    let first = lower_text(&source);
    let second = lower_text(&source);
    verify_mir(&first).unwrap();
    verify_mir(&second).unwrap();
    assert_linear_chain_shape(&first, long_terms - 1);
    let nested = first
        .definitions
        .get(FunctionId::new(1))
        .expect("stress fixture must define `nested` second");
    assert_eq!(nested.body.logical_expressions.len(), nested_depth);
    // Each operation contributes one six-block logical diamond. Right nesting
    // also makes the result and activation lifetimes conditional on every
    // parent, producing two triangular sets of three-block local decisions.
    let nested_block_bound = 1 + 6 * nested_depth + 3 * nested_depth * (nested_depth - 1);
    assert_eq!(nested.body.blocks.len(), nested_block_bound);

    assert_eq!(dump_mir(&first), dump_mir(&second));
    assert_eq!(
        emit_assembly(Target::X86_64SysV, &first).unwrap(),
        emit_assembly(Target::X86_64SysV, &second).unwrap()
    );
}

#[test]
fn effectful_mixed_chain_keeps_conditional_cleanup_graph_growth_linear() {
    let terms = MAX_LOGICAL_EXPRESSION_DEPTH / 2 + 1;
    let expression = mixed_chain(terms, |_| "make(true)->read()".to_owned());
    let source = format!(
        "class Flag {{\n\
           truth: bool;\n\
           init(truth: bool) {{ self.truth = truth; }}\n\
           fn read() -> bool {{ return self.truth; }}\n\
           destroy {{}}\n\
         }}\n\
         fn make(truth: bool) -> shared Flag {{ return new Flag(truth); }}\n\
         fn evaluate() -> bool {{ return {expression}; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    );

    let first = lower_text(&source);
    let second = lower_text(&source);
    verify_mir(&first).unwrap();
    verify_mir(&second).unwrap();
    let evaluate = first
        .definitions
        .get(function_id_from_mir(&first, "evaluate"))
        .expect("stress fixture must define `evaluate`");
    assert_eq!(evaluate.body.logical_expressions.len(), terms - 1);
    assert_eq!(evaluate.body.path_conditions.len(), terms - 1);
    assert!(
        evaluate.body.blocks.len() <= (terms - 1) * 16 + 16,
        "{} effectful terms produced {} blocks",
        terms,
        evaluate.body.blocks.len()
    );
    assert_eq!(dump_mir(&first), dump_mir(&second));
}
