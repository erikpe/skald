use super::*;
use crate::{
    backend::Target,
    hir::{HirExpressionKind, HirIoOperation, HirReturnValue, HirStatement},
    intrinsic::Intrinsic,
    mir::{verify_mir, MirFunctionLinkage},
    test_support::{
        emit_assembly_without_runtime_trace as emit_assembly,
        load_module_sources_with_standard_library,
        load_module_sources_with_standard_library_overrides, lower_hir_to_final_mir,
        CANONICAL_F64_SOURCE, CANONICAL_IO_SOURCE,
    },
    typeck::{
        type_check, INSUFFICIENT_ALIAS_ACCESS, INVALID_ALIAS_ARGUMENT, INVALID_CALL_STATEMENT,
    },
};

#[test]
fn canonical_f64_bit_intrinsics_lower_to_verified_bit_reinterpretation() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "import std::f64;\n",
                "fn main() -> i64 {\n",
                "  return (i64) std::f64::to_bits(std::f64::from_bits(0u));\n",
                "}\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );

    let identities = resolved
        .program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration.linkage {
            ResolvedFunctionLinkage::Intrinsic { intrinsic } => {
                Some((declaration.name.as_str(), intrinsic, declaration.visibility))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            (
                "_to_bits",
                Intrinsic::F64ToBits,
                ResolvedVisibility::Private
            ),
            (
                "_from_bits",
                Intrinsic::F64FromBits,
                ResolvedVisibility::Private,
            ),
        ]
    );
    assert!(resolved.program.external_links.is_empty());

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("valid f64 bit intrinsics produce HIR");
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("PrimitiveCast bit_reinterpretation f64.u64 : u64"));
    assert!(hir_dump.contains("PrimitiveCast bit_reinterpretation u64.f64 : f64"));

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("cast.f64.u64 bit_reinterpretation"));
    assert!(mir_dump.contains("cast.u64.f64 bit_reinterpretation"));

    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(assembly.contains("movq rax, xmm14"));
    assert!(assembly.contains("movq xmm14, rax"));
    assert!(!assembly.contains("ska_rt_f64"));
}

#[test]
fn rejects_malformed_f64_bit_intrinsic_declarations() {
    for replacement in [
        CANONICAL_F64_SOURCE.replace("intrinsic fn _to_bits", "public intrinsic fn _to_bits"),
        CANONICAL_F64_SOURCE.replace("value: f64", "number: f64"),
        CANONICAL_F64_SOURCE.replace("value: f64", "value: u64"),
        CANONICAL_F64_SOURCE.replace("_to_bits(value: f64)", "_to_bits()"),
        CANONICAL_F64_SOURCE.replace("_to_bits(value: f64) -> u64", "_to_bits(value: f64) -> f64"),
        CANONICAL_F64_SOURCE.replace(
            "intrinsic fn _to_bits(value: f64) -> u64;",
            "fn _to_bits(value: f64) -> u64 { return 0u; }",
        ),
        CANONICAL_F64_SOURCE.replace("_from_bits", "_unknown_from_bits"),
    ] {
        let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
            "app",
            &[(
                "app.ska",
                "import std::f64;\nfn main() -> i64 { return 0; }\n",
            )],
            &[("std/f64.ska", &replacement)],
        );
        let output = resolve_module_graph(&graph);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_INTRINSIC_DECLARATION),
            "expected intrinsic diagnostic for replacement:\n{replacement}\n{:?}",
            output.diagnostics
        );
    }
}

fn io_module_with_bodies(bodies: &str) -> String {
    format!("{CANONICAL_IO_SOURCE}\n{bodies}")
}

fn direct_call(statement: &ResolvedStatement) -> &ResolvedDirectCallExpr {
    let ResolvedStatement::Expression(statement) = statement else {
        panic!("expected a call statement");
    };
    let ResolvedExpression::DirectCall(call) = &statement.expression else {
        panic!("expected a direct call");
    };
    call
}

fn is_io_intrinsic(intrinsic: Intrinsic) -> bool {
    matches!(
        intrinsic,
        Intrinsic::IoStandardHandle
            | Intrinsic::IoOpen
            | Intrinsic::IoRead
            | Intrinsic::IoWrite
            | Intrinsic::IoClose
    )
}

#[test]
fn canonical_io_intrinsics_have_exact_stable_identities_and_no_definitions() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            "import std::io;\nfn main() -> i64 { return 0; }\n",
        )],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let identities = output
        .program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration.linkage {
            ResolvedFunctionLinkage::Intrinsic { intrinsic } if is_io_intrinsic(intrinsic) => {
                Some((declaration.name.as_str(), intrinsic))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            ("_io_standard_handle", Intrinsic::IoStandardHandle),
            ("_io_open", Intrinsic::IoOpen),
            ("_io_read", Intrinsic::IoRead),
            ("_io_write", Intrinsic::IoWrite),
            ("_io_close", Intrinsic::IoClose),
        ]
    );
    for declaration in output.program.declarations.iter().filter(|declaration| {
        matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::Intrinsic { intrinsic } if is_io_intrinsic(intrinsic)
        )
    }) {
        assert_eq!(declaration.visibility, ResolvedVisibility::Private);
        assert!(output.program.definitions.get(declaration.id).is_none());
    }
    assert!(output.program.external_links.is_empty());

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    for identity in [
        "intrinsic IoStandardHandle",
        "intrinsic IoOpen",
        "intrinsic IoRead",
        "intrinsic IoWrite",
        "intrinsic IoClose",
    ] {
        assert!(dump.contains(identity), "missing {identity:?} in:\n{dump}");
    }
}

#[test]
fn io_intrinsic_calls_type_to_dedicated_target_independent_hir() {
    let io = io_module_with_bodies(concat!(
        "public fn standard(stream: u8) -> i64 { return _io_standard_handle(stream); }\n",
        "public fn open(ref path: u8[], mode: u8) -> i64 { return _io_open(path, mode); }\n",
        "public fn read(handle: i64, mut ref destination: u8[], offset: u64) -> i64 {\n",
        "  return _io_read(handle, destination, offset);\n",
        "}\n",
        "public fn write(handle: i64, ref source: u8[], offset: u64) -> i64 {\n",
        "  return _io_write(handle, source, offset);\n",
        "}\n",
        "public fn close(handle: i64) -> i64 { return _io_close(handle); }\n",
    ));
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[(
            "app.ska",
            "import std::io;\nfn main() -> i64 { return 0; }\n",
        )],
        &[("std/io.ska", &io)],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("valid I/O calls produce HIR");

    let mut operations = Vec::new();
    for definition in hir.definitions.iter() {
        let Some(HirStatement::Return(statement)) = definition.body.statements.last() else {
            continue;
        };
        let Some(HirReturnValue::Scalar(expression)) = statement.value.as_ref() else {
            continue;
        };
        if let HirExpressionKind::Io(operation) = &expression.kind {
            operations.push(match operation.as_ref() {
                HirIoOperation::StandardHandle { .. } => Intrinsic::IoStandardHandle,
                HirIoOperation::Open { .. } => Intrinsic::IoOpen,
                HirIoOperation::Read { .. } => Intrinsic::IoRead,
                HirIoOperation::Write { .. } => Intrinsic::IoWrite,
                HirIoOperation::Close { .. } => Intrinsic::IoClose,
            });
            assert_eq!(expression.ty, crate::hir::Type::I64);
        }
    }
    assert_eq!(
        operations,
        vec![
            Intrinsic::IoStandardHandle,
            Intrinsic::IoOpen,
            Intrinsic::IoRead,
            Intrinsic::IoWrite,
            Intrinsic::IoClose,
        ]
    );

    let dump = crate::hir::dump_hir(&hir);
    assert_eq!(dump, crate::hir::dump_hir(&hir));
    for operation in [
        "Io StandardHandle : i64",
        "Io Open : i64",
        "Io Read : i64",
        "Io Write : i64",
        "Io Close : i64",
    ] {
        assert!(
            dump.contains(operation),
            "missing {operation:?} in:\n{dump}"
        );
    }
    assert!(dump.contains("ArrayAliasArgument : array"));
    assert!(dump.contains("access=ReadOnly"));
    assert!(dump.contains("access=Mutable"));
    assert!(!dump.contains("ska_rt_io_"));
}

#[test]
fn io_intrinsics_reuse_array_alias_eligibility_and_expression_consumer_rules() {
    for body in [
        "public fn bad() -> i64 { return _io_open(u8[](1u), 0u8); }",
        concat!(
            "public fn bad(ref bytes: u8[]) -> i64 {\n",
            "  return _io_write(1, bytes[:], 0u);\n",
            "}"
        ),
        concat!(
            "public fn bad(ref bytes: u8[]) -> i64 {\n",
            "  return _io_read(0, bytes, 0u);\n",
            "}"
        ),
    ] {
        let io = io_module_with_bodies(body);
        let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
            "app",
            &[(
                "app.ska",
                "import std::io;\nfn main() -> i64 { return 0; }\n",
            )],
            &[("std/io.ska", &io)],
        );
        let resolved = resolve_module_graph(&graph);
        assert!(
            resolved.diagnostics.is_empty(),
            "{:?}",
            resolved.diagnostics
        );
        let checked = type_check(&resolved.program);
        assert!(
            checked.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == INVALID_ALIAS_ARGUMENT
                    || diagnostic.code == INSUFFICIENT_ALIAS_ACCESS
            }),
            "expected alias diagnostic for {body:?}: {:?}",
            checked.diagnostics
        );
        assert!(checked.hir.is_none());
    }

    let io =
        io_module_with_bodies("public fn bad(handle: i64) -> unit { _io_close(handle); return; }");
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[(
            "app.ska",
            "import std::io;\nfn main() -> i64 { return 0; }\n",
        )],
        &[("std/io.ska", &io)],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_CALL_STATEMENT));
}

#[test]
fn rejects_private_io_imports_and_manufactured_intrinsics() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            "from std::io import _io_open;\nfn main() -> i64 { return 0; }\n",
        )],
    );
    let output = resolve_module_graph(&graph);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_DECLARATION));

    let manufactured = resolve_text(concat!(
        "intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(manufactured
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INTRINSIC_DECLARATION));
    assert!(dump_resolved(&manufactured.program).contains("intrinsic Unrecognized"));
}

#[test]
fn rejects_malformed_replacement_io_intrinsic_declarations() {
    for replacement in [
        CANONICAL_IO_SOURCE.replace("intrinsic fn _io_close", "public intrinsic fn _io_close"),
        CANONICAL_IO_SOURCE.replace("handle: i64", "descriptor: i64"),
        CANONICAL_IO_SOURCE.replace("_io_close(handle: i64)", "_io_close()"),
        CANONICAL_IO_SOURCE.replace("ref path: u8[]", "mut ref path: u8[]"),
        CANONICAL_IO_SOURCE.replace("destination: u8[]", "destination: i64[]"),
        CANONICAL_IO_SOURCE.replace("mut ref destination", "ref destination"),
        CANONICAL_IO_SOURCE.replace("offset: u64", "offset: i64"),
        CANONICAL_IO_SOURCE.replace("-> i64;", "-> u64;"),
        CANONICAL_IO_SOURCE.replace(
            "intrinsic fn _io_close(handle: i64) -> i64;",
            "fn _io_close(handle: i64) -> i64 { return 0; }",
        ),
        CANONICAL_IO_SOURCE.replace(
            "intrinsic fn _io_close(handle: i64) -> i64;",
            "extern fn _io_close(handle: i64) -> i64;",
        ),
        CANONICAL_IO_SOURCE.replace("_io_standard_handle", "_io_unknown"),
    ] {
        let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
            "app",
            &[(
                "app.ska",
                "import std::io;\nfn main() -> i64 { return 0; }\n",
            )],
            &[("std/io.ska", &replacement)],
        );
        let output = resolve_module_graph(&graph);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_INTRINSIC_DECLARATION),
            "expected intrinsic diagnostic for replacement:\n{replacement}\n{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn all_supported_spellings_resolve_to_one_panic_intrinsic_identity() {
    let error = concat!(
        "import std::str;\n",
        "public intrinsic fn panic(message: std::str::Str) -> unit;\n",
        "public fn direct(message: std::str::Str) -> unit { panic(message); }\n",
    );
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[(
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
        )],
        &[("std/error.ska", error)],
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
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            "import std::error;\nfn main() -> i64 { return 0; }\n",
        )],
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
    let mir = lower_hir_to_final_mir(&hir);
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
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
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
        )],
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
    let stop_function = stop.function;
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("Panic"));
    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("panic "));
    let stop = mir
        .definitions
        .get(stop_function)
        .expect("stop must have a MIR definition");
    assert!(stop.body.blocks.iter().any(|block| matches!(
        block.terminator,
        Some(crate::mir::MirTerminator::Panic { .. })
    )));
    assert!(stop.body.blocks.iter().all(|block| block
        .instructions
        .iter()
        .all(|instruction| !matches!(instruction, crate::mir::MirInstruction::Call(_)))));
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
        let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
            "app",
            &[(
                "app.ska",
                "import std::error;\nfn main() -> i64 { return 0; }\n",
            )],
            &[("std/error.ska", &error_module)],
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
