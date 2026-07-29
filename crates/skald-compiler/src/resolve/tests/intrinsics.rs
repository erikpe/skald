use super::*;
use crate::{
    intrinsic::Intrinsic,
    mir::{lower_hir, verify_mir, MirFunctionLinkage},
    test_support::{load_module_sources, CANONICAL_ERROR_SOURCE, CANONICAL_STR_SOURCE},
    typeck::type_check,
};

fn direct_call(statement: &ResolvedStatement) -> &ResolvedDirectCallExpr {
    let ResolvedStatement::Expression(statement) = statement else {
        panic!("expected a call statement");
    };
    let ResolvedExpression::DirectCall(call) = &statement.expression else {
        panic!("expected a direct call");
    };
    call
}

#[test]
fn all_supported_spellings_resolve_to_one_panic_intrinsic_identity() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import std::error;\n",
                    "import std::error as errors;\n",
                    "import std::str;\n",
                    "from std::error import panic, panic as fail;\n",
                    "fn qualified(message: std::str::Str) -> unit { std::error::panic(message); }\n",
                    "fn module_alias(message: std::str::Str) -> unit { errors::panic(message); }\n",
                    "fn selective(message: std::str::Str) -> unit { panic(message); }\n",
                    "fn selective_alias(message: std::str::Str) -> unit { fail(message); }\n",
                    "fn main() -> i64 { return 0; }\n",
                ),
            ),
            (
                "std/error.ska",
                concat!(
                    "import std::str;\n",
                    "public intrinsic fn panic(message: std::str::Str) -> unit;\n",
                    "public fn direct(message: std::str::Str) -> unit { panic(message); }\n",
                ),
            ),
            ("std/str.ska", CANONICAL_STR_SOURCE),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "canonical intrinsic must resolve: {:?}",
        output.diagnostics
    );
    let panic = output
        .program
        .declarations
        .iter()
        .find(|declaration| {
            matches!(
                declaration.linkage,
                ResolvedFunctionLinkage::Intrinsic {
                    intrinsic: Intrinsic::Panic
                }
            )
        })
        .expect("canonical panic declaration");
    let targets = output
        .program
        .definitions
        .iter()
        .filter_map(|definition| definition.body.statements.first())
        .filter_map(|statement| match statement {
            ResolvedStatement::Expression(_) => Some(direct_call(statement).function),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets, vec![panic.id; 5]);
    assert!(output.program.definitions.get(panic.id).is_none());
    assert!(output.program.external_links.is_empty());
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("intrinsic Panic"));
}

#[test]
fn unused_canonical_intrinsic_remains_bodyless_through_verified_mir() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import std::error;\nfn main() -> i64 { return 0; }\n",
            ),
            ("std/error.ska", CANONICAL_ERROR_SOURCE),
            ("std/str.ska", CANONICAL_STR_SOURCE),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());

    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "unused intrinsic must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.expect("unused intrinsic permits complete HIR");
    let hir_dump = crate::hir::dump_hir(&hir);
    assert_eq!(hir_dump, crate::hir::dump_hir(&hir));
    assert!(hir_dump.contains("intrinsic Panic"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let intrinsic = mir
        .declarations
        .iter()
        .find(|declaration| {
            matches!(
                declaration.linkage,
                MirFunctionLinkage::Intrinsic {
                    intrinsic: Intrinsic::Panic
                }
            )
        })
        .expect("intrinsic declaration reaches metadata");
    assert!(mir.definitions.get(intrinsic.id).is_none());
    assert!(mir.external_links.is_empty());
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    assert!(mir_dump.contains("intrinsic Panic"));
}

#[test]
fn panic_calls_lower_as_terminating_hir_and_mir_statements() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import std::error;\n",
                    "import std::str;\n",
                    "fn later() -> unit {}\n",
                    "fn stop(message: std::str::Str) -> unit {\n",
                    "  std::error::panic(message);\n",
                    "  later();\n",
                    "}\n",
                    "fn main() -> i64 { return 0; }\n",
                ),
            ),
            ("std/error.ska", CANONICAL_ERROR_SOURCE),
            ("std/str.ska", CANONICAL_STR_SOURCE),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("panic statement must produce HIR");
    let stop = hir
        .definitions
        .iter()
        .find(|definition| hir.declarations.get(definition.function).unwrap().name == "stop")
        .unwrap();
    assert!(!stop.body.effects.can_fall_through());
    assert!(stop.body.effects.can_diverge());
    assert!(matches!(
        stop.body.statements[0],
        crate::hir::HirStatement::Panic(_)
    ));
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("Panic"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("panic "));
    assert!(!mir_dump.contains("call direct"));
}

#[test]
fn rejects_noncanonical_and_malformed_panic_intrinsics_during_resolution() {
    let noncanonical = resolve_text(concat!(
        "public intrinsic fn panic(message: i64) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(noncanonical
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INTRINSIC_DECLARATION));

    for declaration in [
        "public fn other() -> unit {}",
        "intrinsic fn panic(message: std::str::Str) -> unit;",
        "public intrinsic fn other(message: std::str::Str) -> unit;",
        "public intrinsic fn panic() -> unit;",
        "public intrinsic fn panic(ref message: std::str::Str) -> unit;",
        "public intrinsic fn panic(text: std::str::Str) -> unit;",
        "public intrinsic fn panic(message: i64) -> unit;",
        "public intrinsic fn panic(message: std::str::Str) -> i64;",
        "public fn panic(message: std::str::Str) -> unit {}",
        "public extern fn panic(message: i64) -> unit;",
    ] {
        let error_module = format!("import std::str;\n{declaration}\n");
        let (_workspace, graph) = load_module_sources(
            "app",
            &[
                (
                    "app.ska",
                    "import std::error;\nfn main() -> i64 { return 0; }\n",
                ),
                ("std/error.ska", &error_module),
                ("std/str.ska", CANONICAL_STR_SOURCE),
            ],
        );
        let output = resolve_module_graph(&graph);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_INTRINSIC_DECLARATION),
            "expected intrinsic diagnostic for {declaration:?}: {:?}",
            output.diagnostics
        );
    }
}
