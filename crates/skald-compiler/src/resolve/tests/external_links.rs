use super::*;
use crate::{
    backend::Target,
    mir::{dump_mir, lower_hir, verify_mir},
    test_support::{
        emit_assembly_without_runtime_trace as emit_assembly, load_module_sources,
        run_native_assembly,
    },
    typeck::type_check,
};

#[test]
fn compatible_cross_module_declarations_share_one_external_link() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import a as first;\n",
                    "import b as second;\n",
                    "import c;\n",
                    "fn main() -> i64 {\n",
                    "  return first::foreign(20) + second::foreign(20);\n",
                    "}\n",
                ),
            ),
            ("a.ska", "public extern fn foreign(left: i64) -> i64;\n"),
            ("b.ska", "public extern fn foreign(right: i64) -> i64;\n"),
            ("c.ska", "extern fn foreign(value: i64) -> i64;\n"),
        ],
    );

    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "compatible declarations must resolve: {:?}",
        resolved.diagnostics
    );
    let link = resolved.program.external_links.iter().next().unwrap();
    assert_eq!(link.id.index(), 0);
    assert_eq!(link.symbol, "foreign");
    assert_eq!(
        link.declarations
            .iter()
            .map(|function| function.index())
            .collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert!(link.declarations.iter().all(|function| {
        matches!(
            resolved.program.declarations.get(*function).unwrap().linkage,
            ResolvedFunctionLinkage::External { link: target } if target == link.id
        )
    }));

    let resolved_dump = dump_resolved(&resolved.program);
    assert!(resolved_dump.contains("Link ext0 \"foreign\" declarations f0 f2 f3"));
    assert!(resolved_dump.contains("Declaration f3 module m3 \"foreign\" external ext0"));

    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "compatible declarations must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.unwrap();
    assert_eq!(hir.external_links, resolved.program.external_links);
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("Link ext0 \"foreign\" declarations f0 f2 f3"));
    let mir = lower_hir(&hir);
    assert_eq!(mir.external_links, hir.external_links);
    verify_mir(&mir).unwrap();
    assert!(dump_mir(&mir).contains("Link ext0 \"foreign\" declarations f0 f2 f3"));
    let mut assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert_eq!(assembly.matches("call foreign").count(), 2);
    assembly.push_str(concat!(
        "\n.text\n",
        ".globl foreign\n",
        ".type foreign, @function\n",
        "foreign:\n",
        "    lea rax, [rdi + 1]\n",
        "    ret\n",
        ".size foreign, .-foreign\n",
    ));
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn external_link_ids_are_allocated_by_symbol_not_module_or_discovery_order() {
    let sources = [
        (
            "app.ska",
            "import z;\nimport a;\nfn main() -> i64 { return z::alpha() + a::zeta(); }\n",
        ),
        ("z.ska", "public extern fn alpha() -> i64;\n"),
        ("a.ska", "public extern fn zeta() -> i64;\n"),
    ];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources(
        "app",
        &[
            sources[2],
            (
                "app.ska",
                "import a;\nimport z;\nfn main() -> i64 { return z::alpha() + a::zeta(); }\n",
            ),
            sources[1],
        ],
    );

    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);
    assert!(first.diagnostics.is_empty());
    assert!(second.diagnostics.is_empty());
    let observe = |program: &ResolvedProgram| {
        program
            .external_links
            .iter()
            .map(|link| {
                (
                    link.id.index(),
                    link.symbol.clone(),
                    link.declarations
                        .iter()
                        .map(|function| function.index())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        observe(&first.program),
        [
            (0, "alpha".to_owned(), vec![2]),
            (1, "zeta".to_owned(), vec![0]),
        ]
    );
    assert_eq!(observe(&first.program), observe(&second.program));

    let declaration_order_a = resolve_text(concat!(
        "extern fn zeta() -> i64;\n",
        "extern fn alpha() -> i64;\n",
        "fn main() -> i64 { return alpha() + zeta(); }\n",
    ));
    let declaration_order_b = resolve_text(concat!(
        "extern fn alpha() -> i64;\n",
        "extern fn zeta() -> i64;\n",
        "fn main() -> i64 { return alpha() + zeta(); }\n",
    ));
    assert!(declaration_order_a.diagnostics.is_empty());
    assert!(declaration_order_b.diagnostics.is_empty());
    let links = |program: &ResolvedProgram| {
        program
            .external_links
            .iter()
            .map(|link| (link.id.index(), link.symbol.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        links(&declaration_order_a.program),
        links(&declaration_order_b.program)
    );
    let assembly = |program: &ResolvedProgram| {
        let checked = type_check(program);
        assert!(checked.diagnostics.is_empty());
        let mir = lower_hir(&checked.hir.unwrap());
        verify_mir(&mir).unwrap();
        emit_assembly(Target::X86_64SysV, &mir).unwrap()
    };
    assert_eq!(
        assembly(&declaration_order_a.program),
        assembly(&declaration_order_b.program)
    );
}

#[test]
fn incompatible_external_abi_declarations_receive_one_complete_diagnostic() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import a;\nimport b;\nimport c;\nfn main() -> i64 { return 0; }\n",
            ),
            ("a.ska", "extern fn foreign(value: i64) -> i64;\n"),
            ("b.ska", "extern fn foreign(value: u64) -> i64;\n"),
            (
                "c.ska",
                "extern fn foreign(left: i64, right: i64) -> bool;\n",
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INCOMPATIBLE_EXTERNAL_ABI)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics[0];
    assert!(diagnostic.message.contains("external symbol `foreign`"));
    assert_eq!(diagnostic.labels.len(), 3);
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("fn(i64) -> i64")));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("fn(u64) -> i64")));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("fn(i64, i64) -> bool")));
    assert_eq!(
        output
            .program
            .external_links
            .iter()
            .next()
            .unwrap()
            .declarations
            .len(),
        3
    );
}

#[test]
fn internal_definitions_never_participate_in_external_coalescing() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import external;\nimport internal;\nfn main() -> i64 { return internal::same(); }\n",
            ),
            ("external.ska", "extern fn same() -> i64;\n"),
            (
                "internal.ska",
                "public fn same() -> i64 { return 7; }\n",
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty());
    let link = output.program.external_links.iter().next().unwrap();
    assert_eq!(link.symbol, "same");
    assert_eq!(link.declarations.len(), 1);
    let internal = output
        .program
        .declarations
        .iter()
        .find(|declaration| {
            declaration.name == "same"
                && matches!(declaration.linkage, ResolvedFunctionLinkage::Internal)
        })
        .unwrap();
    assert!(!link.declarations.contains(&internal.id));
}
