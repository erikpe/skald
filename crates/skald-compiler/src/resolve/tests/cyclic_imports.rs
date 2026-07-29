use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::dump_hir,
    mir::{dump_mir, lower_hir, verify_mir},
    test_support::load_module_sources,
    typeck::{type_check, INVALID_INTERFACE_CONFORMANCE, RECURSIVE_INLINE_CONTAINMENT},
};

const APP: &str = include_str!("../../../../../tests/golden/run/modules_cycle/modules/app.ska");
const FIRST: &str = include_str!("../../../../../tests/golden/run/modules_cycle/modules/first.ska");
const SECOND: &str =
    include_str!("../../../../../tests/golden/run/modules_cycle/modules/second.ska");

fn semantic_cycle_sources() -> [(&'static str, &'static str); 3] {
    [
        ("app.ska", APP),
        ("first.ska", FIRST),
        ("second.ska", SECOND),
    ]
}

fn normalize_selected_entry(text: &str) -> String {
    text.lines()
        .map(|line| {
            let indentation = &line[..line.len() - line.trim_start().len()];
            let trimmed = line.trim_start();
            if trimmed.starts_with("SelectedModule ") {
                format!("{indentation}SelectedModule <selected>")
            } else if trimmed.starts_with("Entry ") {
                format!("{indentation}Entry <entry>")
            } else if trimmed.starts_with("call .Lska.fn.") && trimmed.contains(".main.f") {
                format!("{indentation}call <entry>")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn semantic_cycle_lowers_to_verified_native_assembly() {
    let (_workspace, graph) = load_module_sources("app", &semantic_cycle_sources());
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "semantic cycle must resolve: {:?}",
        resolved.diagnostics
    );

    let resolved_dump = dump_resolved(&resolved.program);
    assert!(resolved_dump.matches("\"recurse\"").count() >= 2);
    assert!(resolved_dump.contains("DirectCall"));
    assert!(resolved_dump.contains("Implements"));

    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "semantic cycle must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("\"make_first\""));
    assert!(hir_dump.contains("\"make_second\""));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = dump_mir(&mir);
    assert!(mir_dump.contains("\"recurse\""));

    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(assembly.contains(".Lska.fn.first.recurse"));
    assert!(assembly.contains(".Lska.fn.second.recurse"));
    assert!(assembly.contains("call .Lska.fn.first.recurse"));
    assert!(assembly.contains("call .Lska.fn.second.recurse"));
}

#[test]
fn non_module_semantic_cycles_keep_their_existing_owners() {
    let cases = [
        (
            "inheritance",
            [
                (
                    "first.ska",
                    "import second;\npublic class First extends second::Second { init() { super(); } }\nfn main() -> i64 { return 0; }\n",
                ),
                (
                    "second.ska",
                    "import first;\npublic class Second extends first::First { init() { super(); } }\n",
                ),
            ],
            INHERITANCE_CYCLE,
        ),
        (
            "external ABI",
            [
                (
                    "first.ska",
                    "import second;\npublic extern fn foreign(value: i64) -> i64;\nfn main() -> i64 { return 0; }\n",
                ),
                (
                    "second.ska",
                    "import first;\npublic extern fn foreign(value: u64) -> i64;\n",
                ),
            ],
            INCOMPATIBLE_EXTERNAL_ABI,
        ),
    ];

    for (label, sources, expected_code) in cases {
        let (_workspace, graph) = load_module_sources("first", &sources);
        let resolved = resolve_module_graph(&graph);
        assert!(
            resolved
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "{label} must retain diagnostic {expected_code}: {:?}",
            resolved.diagnostics
        );
    }

    let (_workspace, graph) = load_module_sources(
        "first",
        &[
            (
                "first.ska",
                "import second;\npublic interface View { fn value() -> i64; }\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "second.ska",
                "import first;\npublic class Broken implements first::View { init() {} }\n",
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_INTERFACE_CONFORMANCE
            && diagnostic
                .message
                .contains("does not implement requirement `View.value`")
    }));

    let (_workspace, graph) = load_module_sources(
        "first",
        &[
            (
                "first.ska",
                "import second;\npublic class First { second: second::Second; init() {} }\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "second.ska",
                "import first;\npublic class Second { first: first::First; init() {} }\n",
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == RECURSIVE_INLINE_CONTAINMENT
            && diagnostic.message.contains("First.second")
            && diagnostic.message.contains("Second.first")
    }));
}

#[test]
fn selecting_either_cycle_member_preserves_all_non_entry_products() {
    let sources = [
        (
            "a.ska",
            "import b;\npublic fn value() -> i64 { return 42; }\nfn main() -> i64 { return b::value(); }\n",
        ),
        (
            "b.ska",
            "import a;\npublic fn value() -> i64 { return 42; }\nfn main() -> i64 { return a::value(); }\n",
        ),
    ];
    let (_a_workspace, a_graph) = load_module_sources("a", &sources);
    let (_b_workspace, b_graph) = load_module_sources("b", &sources);
    let a_resolved = resolve_module_graph(&a_graph);
    let b_resolved = resolve_module_graph(&b_graph);
    assert!(a_resolved.diagnostics.is_empty());
    assert!(b_resolved.diagnostics.is_empty());

    let identities = |program: &ResolvedProgram| {
        program
            .declarations
            .iter()
            .map(|declaration| (declaration.id, declaration.module, declaration.name.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        identities(&a_resolved.program),
        identities(&b_resolved.program)
    );
    assert_ne!(
        a_resolved.program.entry_function,
        b_resolved.program.entry_function
    );
    assert_eq!(
        normalize_selected_entry(&dump_resolved(&a_resolved.program)),
        normalize_selected_entry(&dump_resolved(&b_resolved.program))
    );

    let a_checked = type_check(&a_resolved.program);
    let b_checked = type_check(&b_resolved.program);
    assert!(a_checked.diagnostics.is_empty());
    assert!(b_checked.diagnostics.is_empty());
    let a_hir = a_checked.hir.unwrap();
    let b_hir = b_checked.hir.unwrap();
    assert_eq!(
        normalize_selected_entry(&dump_hir(&a_hir)),
        normalize_selected_entry(&dump_hir(&b_hir))
    );

    let a_mir = lower_hir(&a_hir);
    let b_mir = lower_hir(&b_hir);
    verify_mir(&a_mir).unwrap();
    verify_mir(&b_mir).unwrap();
    assert_eq!(
        normalize_selected_entry(&dump_mir(&a_mir)),
        normalize_selected_entry(&dump_mir(&b_mir))
    );
    assert_eq!(
        normalize_selected_entry(&emit_assembly(Target::X86_64SysV, &a_mir).unwrap()),
        normalize_selected_entry(&emit_assembly(Target::X86_64SysV, &b_mir).unwrap())
    );
}
