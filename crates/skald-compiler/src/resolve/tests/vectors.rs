use super::*;
use crate::{
    hir::dump_hir, test_support::load_module_sources_with_standard_library, typeck::type_check,
};

#[test]
fn canonical_vector_brackets_select_specialized_ordinary_methods() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::vec import Vec;\n",
                "fn main() -> i64 {\n",
                "  var values: Vec<i64> = Vec<i64>();\n",
                "  values.push(1);\n",
                "  var read: i64 = values[0];\n",
                "  values[0] = 2;\n",
                "  var slice: Vec<i64> = values[:];\n",
                "  values[:] = slice;\n",
                "  return read;\n",
                "}\n",
            ),
        )],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "canonical vector source must resolve: {:?}",
        resolved.diagnostics
    );

    let main = resolved
        .program
        .entry_function
        .expect("application must declare main");
    let definition = resolved.program.definitions.get(main).unwrap();
    let selected_name = |expression: &ResolvedExpression| {
        let ResolvedExpression::MethodCall(call) = expression else {
            panic!("structural vector use must normalize to an ordinary method call");
        };
        resolved
            .program
            .classes
            .get(call.method.class())
            .and_then(|class| class.method(call.method))
            .map(|method| method.name.as_str())
            .expect("selected vector method must retain declaration metadata")
    };

    let ResolvedStatement::Local(read) = &definition.body.statements[2] else {
        panic!("expected indexed read local");
    };
    assert_eq!(selected_name(&read.initializer), "index_get");
    let ResolvedStatement::Expression(index_write) = &definition.body.statements[3] else {
        panic!("expected indexed write call statement");
    };
    assert_eq!(selected_name(&index_write.expression), "index_set");
    let ResolvedStatement::Local(slice) = &definition.body.statements[4] else {
        panic!("expected slice read local");
    };
    assert_eq!(selected_name(&slice.initializer), "slice_get");
    let ResolvedStatement::Expression(slice_write) = &definition.body.statements[5] else {
        panic!("expected slice write call statement");
    };
    assert_eq!(selected_name(&slice_write.expression), "slice_set");

    let resolved_dump = dump_resolved(&resolved.program);
    assert_eq!(resolved_dump, dump_resolved(&resolved.program));
    assert!(resolved_dump.contains("Vec<i64>"), "{resolved_dump}");
    assert!(!resolved_dump.contains("Structural"), "{resolved_dump}");

    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "canonical vector source must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.expect("valid vector source must produce HIR");
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("Vec<i64>"), "{hir_dump}");
    assert!(!hir_dump.contains("Structural"), "{hir_dump}");
}
