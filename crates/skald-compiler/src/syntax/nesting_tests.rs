use super::*;
use crate::test_support::parse_source;

fn parse_text(text: String) -> ParseOutput {
    parse_source(text).1
}

fn grouped_expression(groups: usize) -> String {
    format!("{}1{}", "(".repeat(groups), ")".repeat(groups))
}

fn nested_blocks(blocks: usize) -> String {
    format!("{}return 0;{}", "{".repeat(blocks), "}".repeat(blocks))
}

fn source_with_return(expression: &str) -> String {
    format!("fn main() -> i64 {{ return {expression}; }}")
}

fn assert_single_nesting_error(output: &ParseOutput) {
    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, EXCESSIVE_NESTING);
    assert_eq!(
        diagnostic.message,
        format!("syntax nesting exceeds the implementation limit of {MAX_SYNTAX_NESTING}")
    );
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(
        diagnostic.notes,
        ["split deeply nested expressions or blocks into smaller statements"]
    );
}

#[test]
fn expressions_immediately_below_and_at_the_nesting_limit_parse() {
    // The function body itself consumes one nesting level.
    for groups in [MAX_SYNTAX_NESTING - 2, MAX_SYNTAX_NESTING - 1] {
        let output = parse_text(source_with_return(&grouped_expression(groups)));
        assert!(output.diagnostics.is_empty(), "failed with {groups} groups");
        assert_eq!(output.ast.declarations.len(), 1);
    }
}

#[test]
fn expression_above_the_limit_is_omitted_and_recovers_at_a_later_declaration() {
    let source = format!(
        "{} fn recovered() -> i64 {{ return 0; }}",
        source_with_return(&grouped_expression(MAX_SYNTAX_NESTING))
    );
    let output = parse_text(source);

    assert_single_nesting_error(&output);
    assert_eq!(output.ast.declarations.len(), 1);
    let TopLevelDeclaration::Function(function) = &output.ast.declarations[0] else {
        panic!("expected recovered function");
    };
    assert_eq!(function.name.text, "recovered");
}

#[test]
fn blocks_immediately_below_and_at_the_nesting_limit_parse() {
    for nested in [MAX_SYNTAX_NESTING - 2, MAX_SYNTAX_NESTING - 1] {
        let source = format!("fn main() -> i64 {{ {} }}", nested_blocks(nested));
        let output = parse_text(source);
        assert!(
            output.diagnostics.is_empty(),
            "failed with {nested} nested blocks"
        );
        assert_eq!(output.ast.declarations.len(), 1);
    }
}

#[test]
fn block_above_the_limit_is_omitted_and_reports_without_cascades() {
    let source = format!(
        "fn too_deep() -> i64 {{ {} }} fn recovered() -> i64 {{ return 0; }}",
        nested_blocks(MAX_SYNTAX_NESTING)
    );
    let output = parse_text(source);

    assert_single_nesting_error(&output);
    assert_eq!(output.ast.declarations.len(), 1);
    let TopLevelDeclaration::Function(function) = &output.ast.declarations[0] else {
        panic!("expected recovered function");
    };
    assert_eq!(function.name.text, "recovered");
}

#[test]
fn unary_and_postfix_recursion_use_the_same_nesting_budget() {
    let unary = format!("{}1", "-".repeat(MAX_SYNTAX_NESTING));
    assert_single_nesting_error(&parse_text(source_with_return(&unary)));

    let calls = format!(
        "{}1{}",
        "callee(".repeat(MAX_SYNTAX_NESTING),
        ")".repeat(MAX_SYNTAX_NESTING)
    );
    assert_single_nesting_error(&parse_text(source_with_return(&calls)));
}
