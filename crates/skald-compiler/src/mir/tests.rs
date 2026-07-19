use super::*;
use crate::{
    hir::HirProgram,
    lexer::lex,
    resolve::{resolve, FunctionId},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

fn hir_text(text: &str) -> HirProgram {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    checked.hir.unwrap()
}

fn lower_text(text: &str) -> MirProgram {
    lower_hir(&hir_text(text))
}

#[test]
fn lowers_storage_values_arithmetic_and_return_explicitly() {
    let mir = lower_text("fn main() -> i64 { var result: i64 = 1; return result + 2; }");
    assert!(super::verify_mir(&mir).is_ok());
    let function = mir.definitions.get(mir.entry_function).unwrap();

    assert_eq!(function.storage.len(), 1);
    assert_eq!(function.storage[0].kind, MirStorageKind::Local);
    assert_eq!(function.values.len(), 4);
    let block = function.block(function.body.entry).unwrap();
    assert_eq!(block.instructions.len(), 5);
    assert!(matches!(
        block.instructions[0],
        MirInstruction::Assign(MirAssignment {
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ..
            },
            ..
        })
    ));
    assert!(matches!(block.instructions[1], MirInstruction::Store(_)));
    assert!(matches!(
        block.instructions[4],
        MirInstruction::Assign(MirAssignment {
            rvalue: MirRvalue {
                kind: MirRvalueKind::Binary {
                    operation: MirBinaryOperation::AddI64,
                    ..
                },
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        block.terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn nested_call_arguments_lower_in_deterministic_left_to_right_order() {
    let mir = lower_text(concat!(
        "fn left() -> i64 { return 1; }\n",
        "fn right() -> i64 { return 2; }\n",
        "fn combine(a: i64, b: i64) -> i64 { return a + b; }\n",
        "fn main() -> i64 { return combine(left(), right()); }\n",
    ));
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();
    let calls: Vec<_> = block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(function),
                ..
            }) => Some(*function),
            _ => None,
        })
        .collect();

    assert_eq!(
        calls.iter().map(|id| id.index()).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(dump_mir(&mir).contains("call f2("));
}

#[test]
fn lowering_discards_statements_after_an_unconditional_return() {
    let mir = lower_text("fn main() -> i64 { { return 1; } return 2; }");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();

    assert_eq!(main.values.len(), 1);
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        block.terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn mir_dump_is_deterministic() {
    let mir = lower_text("fn main() -> i64 { return 42; }");

    assert_eq!(
        super::dump_mir(&mir),
        concat!(
            "MirProgram @0..31\n",
            "  Entry f0\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..31\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @0..31\n",
            "      Parameters\n",
            "      Storage\n",
            "      Values\n",
            "        f0:v0 : i64 @26..28\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @17..31\n",
            "          f0:v0 = const.i64 42 : i64 @26..28\n",
            "          return f0:v0 @19..29\n",
        )
    );
}

#[test]
fn verifier_rejects_unterminated_blocks() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("no terminator")));
}

#[test]
fn verifier_rejects_use_before_definition() {
    let mut mir = lower_text("fn main() -> i64 { return 1 + 2; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.blocks[0].instructions.swap(0, 2);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_a_value_defined_in_terms_of_itself() {
    let mut mir = lower_text("fn main() -> i64 { return 1; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected assignment");
    };
    assignment.rvalue.kind = MirRvalueKind::Unary {
        operation: MirUnaryOperation::NegateI64,
        operand: assignment.result,
    };

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_call_signature_mismatches() {
    let mut mir = lower_text(concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return one(1); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(&mut call.arguments),
            _ => None,
        })
        .unwrap();
    call.clear();

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("has 0 arguments but requires 1")));
}

#[test]
fn verifier_rejects_ids_owned_by_another_function() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let foreign = FunctionId::new(99);
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .values[0]
        .id = ValueId::new(foreign, 0);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("owned by another function")));
}

#[test]
fn verifier_accepts_an_external_declaration_without_a_definition() {
    let mut mir = lower_text(concat!(
        "fn foreign(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let foreign = FunctionId::new(0);
    mir.declarations.entries_mut_for_test()[foreign.index()].linkage =
        MirFunctionLinkage::External {
            symbol: "foreign".to_owned(),
        };
    mir.definitions.remove_for_test(foreign);

    assert!(verify_mir(&mir).is_ok());
    assert!(mir.declarations.get(foreign).is_some());
    assert!(mir.definitions.get(foreign).is_none());
}

#[test]
fn verifier_rejects_an_internal_declaration_without_a_definition() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    mir.definitions.remove_for_test(mir.entry_function);

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("entry function f0 has no definition")));
    assert!(errors.iter().any(|error| error
        .message
        .contains("internal function has no definition")));
}

#[test]
fn verifier_rejects_an_unknown_call_target() {
    let mut mir = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return helper(); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.target = MirCallTarget::Direct(FunctionId::new(99));

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("call target f99 is not declared")));
}

#[test]
fn verifier_rejects_a_missing_value_call_result() {
    let mut mir = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return helper(); }\n",
    ));
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.result = None;

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("value-returning call has no result")));
}

#[test]
fn verifier_rejects_definition_signature_mismatches() {
    let mut mir = lower_text(concat!(
        "fn helper(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return helper(1); }\n",
    ));
    mir.definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap()
        .parameters
        .clear();

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("definition has 0 parameters but declaration requires 1")));
}
