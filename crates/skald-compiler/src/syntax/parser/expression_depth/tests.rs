use crate::{
    syntax::{Expression, Statement, TopLevelDeclaration},
    test_support::parse_source,
};

use super::{measure, Depths};

fn parse_expression(text: &str) -> Expression {
    let (_, output) = parse_source(format!("fn probe() -> unit {{ return {text}; }}"));
    assert!(
        output.diagnostics.is_empty(),
        "failed to parse {text:?}: {:?}",
        output.diagnostics
    );

    let TopLevelDeclaration::Function(function) = &output.ast.declarations[0] else {
        panic!("expected function declaration");
    };
    let Statement::Return(statement) = &function.body.statements[0] else {
        panic!("expected return statement");
    };
    statement.value.clone().expect("expected return expression")
}

fn assert_depths(cases: &[(&str, usize, usize)]) {
    for &(text, expression, logical) in cases {
        assert_eq!(
            measure(&parse_expression(text)),
            Depths {
                expression,
                logical,
            },
            "unexpected depths for {text:?}"
        );
    }
}

#[test]
fn measures_leaf_unary_binary_logical_and_postfix_shapes() {
    assert_depths(&[
        ("none", 1, 0),
        ("value", 1, 0),
        ("some(value)", 2, 0),
        ("-value", 2, 0),
        ("left + right", 2, 0),
        ("left + middle + right", 3, 0),
        ("left && right", 2, 1),
        ("left && middle || right", 3, 2),
        ("value is Item", 2, 0),
        ("maybe is some", 2, 0),
        ("maybe!", 2, 0),
        ("(i64) value", 2, 0),
        ("(Item) value", 2, 0),
        ("(value)", 2, 0),
        ("value.member", 2, 0),
        ("values[start:end]", 2, 0),
        ("value.member(1)[0]", 4, 0),
    ]);
}

#[test]
fn measures_call_allocation_optional_and_array_shapes() {
    assert_depths(&[
        ("invoke(first, second)", 2, 0),
        ("Item(copy source.member)", 3, 0),
        ("Generic<Item>()", 2, 0),
        ("Generic<Item>.make()", 2, 0),
        ("new Item(first, second)", 2, 0),
        ("new Item(copy source.member)", 3, 0),
        ("new i64?()", 1, 0),
        ("new i64?(some(value))", 3, 0),
        ("i64[]()", 1, 0),
        ("i64[](length)", 2, 0),
        ("i64[](copy values)", 2, 0),
        ("i64[](length; index => index + 1)", 3, 0),
        ("i64[]{first, second + third}", 3, 0),
    ]);
}
