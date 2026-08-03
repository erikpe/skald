use super::logical_fixtures::{
    function_id, function_id_from_mir, lower_fixture_logical, native_output,
    replace_return_with_logical_expressions, returned_scalar, returned_scalar_mut,
};
use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::{
        HirCallArgument, HirExpression, HirExpressionKind, HirLogicalExpression,
        HirLogicalOperation, HirOptionalOperand, HirPresenceTestKind, HirProgram, Type,
    },
    test_support::{run_native_assembly, type_check_source},
};

const OBJECT_OPERANDS: &str = concat!(
    "class Trace {\n",
    "  marker: i64;\n",
    "  truth: bool;\n",
    "  init(marker: i64, truth: bool) {\n",
    "    self.marker = marker;\n",
    "    self.truth = truth;\n",
    "  }\n",
    "  copy(ref other: Trace) {\n",
    "    self.marker = other.marker;\n",
    "    self.truth = other.truth;\n",
    "  }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  static fn static_read(value: Trace) -> bool { return value.read(); }\n",
    "  destroy {}\n",
    "}\n",
    "fn inspect(value: Trace) -> bool { return value.read(); }\n",
    "fn produce(marker: i64, truth: bool) -> Trace { return Trace(marker, truth); }\n",
    "fn left() -> bool { return inspect(Trace(1, true)); }\n",
    "fn middle() -> bool { return inspect(Trace(3, true)); }\n",
    "fn right() -> bool { return Trace.static_read(produce(2, false)); }\n",
    "fn combine(flag: bool, value: Trace) -> bool { return flag; }\n",
    "fn evaluate() -> bool { return combine(false, Trace(4, true)); }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn selected_inline_object_receivers_live_until_conditional_cleanup() {
    for (operation, expected) in [(HirLogicalOperation::And, 0), (HirLogicalOperation::Or, 1)] {
        let mir = lower_fixture_logical(OBJECT_OPERANDS, "evaluate", operation, "left", "right");
        verify_mir(&mir).unwrap();
        let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
        assert_eq!(run_native_assembly(&assembly).code(), Some(expected));

        let evaluate = mir
            .definitions
            .get(function_id_from_mir(&mir, "evaluate"))
            .unwrap();
        assert_eq!(
            evaluate
                .storage
                .iter()
                .filter(|storage| {
                    storage.kind == MirStorageKind::Temporary
                        && matches!(storage.ty, MirType::Class(_))
                })
                .count(),
            2
        );
    }
}

const OBSERVABLE_OBJECT_OPERANDS: &str = concat!(
    "extern fn test_record_i64(value: i64) -> unit;\n",
    "class Trace {\n",
    "  marker: i64;\n",
    "  truth: bool;\n",
    "  init(marker: i64, truth: bool) {\n",
    "    self.marker = marker;\n",
    "    self.truth = truth;\n",
    "  }\n",
    "  copy(ref other: Trace) {\n",
    "    self.marker = other.marker;\n",
    "    self.truth = other.truth;\n",
    "  }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  static fn static_read(value: Trace) -> bool { return value.read(); }\n",
    "  destroy { test_record_i64(self.marker); }\n",
    "}\n",
    "fn inspect(value: Trace) -> bool { return value.truth; }\n",
    "fn produce(marker: i64, truth: bool) -> Trace { return Trace(marker, truth); }\n",
    "fn left() -> bool { return inspect(Trace(1, true)); }\n",
    "fn middle() -> bool { return inspect(Trace(3, true)); }\n",
    "fn right() -> bool { return Trace.static_read(produce(2, false)); }\n",
    "fn combine(flag: bool, value: Trace) -> bool { return flag; }\n",
    "fn evaluate() -> bool { return combine(false, Trace(4, true)); }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn inline_object_destruction_observes_skip_and_reverse_completion_order() {
    let selected = lower_fixture_logical(
        OBSERVABLE_OBJECT_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "right",
    );
    let output = native_output(&selected);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"1\n2\n2\n1\n");

    let skipped = lower_fixture_logical(
        OBSERVABLE_OBJECT_OPERANDS,
        "evaluate",
        HirLogicalOperation::Or,
        "left",
        "right",
    );
    let output = native_output(&skipped);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"1\n1\n");
}

#[test]
fn nested_selected_object_registrations_clean_in_global_reverse_order() {
    let mut hir = type_check_source(OBSERVABLE_OBJECT_OPERANDS).hir.unwrap();
    let operand = |program: &HirProgram, name| {
        returned_scalar(
            program
                .definitions
                .get(function_id(program, name))
                .expect("operand fixture function must have a body"),
        )
        .clone()
    };
    let left = operand(&hir, "left");
    let middle = operand(&hir, "middle");
    let right = operand(&hir, "right");
    let span = left.span;
    let inner = HirExpression {
        kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
            HirLogicalOperation::And,
            left,
            middle,
        ))),
        ty: Type::Bool,
        span,
    };
    replace_return_with_logical_expressions(
        &mut hir,
        "evaluate",
        HirLogicalOperation::And,
        inner,
        right,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let output = native_output(&mir);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"1\n3\n2\n2\n3\n1\n");
}

#[test]
fn logical_result_survives_later_object_completion_before_cleanup() {
    let mut hir = type_check_source(OBSERVABLE_OBJECT_OPERANDS).hir.unwrap();
    let left = returned_scalar(
        hir.definitions
            .get(function_id(&hir, "left"))
            .expect("left fixture function must have a body"),
    )
    .clone();
    let right = returned_scalar(
        hir.definitions
            .get(function_id(&hir, "right"))
            .expect("right fixture function must have a body"),
    )
    .clone();
    let evaluate = function_id(&hir, "evaluate");
    let result = returned_scalar_mut(
        hir.definitions
            .get_mut_for_test(evaluate)
            .expect("evaluate fixture function must have a body"),
    );
    let HirExpressionKind::DirectCall { arguments, .. } = &mut result.kind else {
        panic!("evaluate fixture must return a direct call");
    };
    let HirCallArgument::Value(flag) = &mut arguments[0] else {
        panic!("combine flag must be a scalar value argument");
    };
    let span = flag.span;
    *flag = HirExpression {
        kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
            HirLogicalOperation::And,
            left,
            right,
        ))),
        ty: Type::Bool,
        span,
    };

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let output = native_output(&mir);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"1\n2\n4\n4\n2\n1\n");
}

const OPTIONAL_OPERANDS: &str = concat!(
    "class Trace { marker: i64; init(marker: i64) { self.marker = marker; } destroy {} }\n",
    "fn present(marker: i64) -> Trace? { return Trace(marker); }\n",
    "fn absent() -> Trace? { return none; }\n",
    "fn inspect_optional(value: Trace?) -> bool { return value is some; }\n",
    "fn argument_present() -> bool { return inspect_optional(Trace(2)); }\n",
    "fn argument_absent() -> bool { return inspect_optional(none); }\n",
    "fn always_true() -> bool { return true; }\n",
    "fn evaluate() -> bool { return false; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

fn optional_presence_call(
    program: &HirProgram,
    function: &str,
    marker: Option<i64>,
) -> HirExpression {
    let function = function_id(program, function);
    let class = program
        .classes
        .iter()
        .find(|class| class.name == "Trace")
        .expect("fixture class must be declared")
        .id;
    let span = program
        .declarations
        .get(function)
        .expect("fixture function must be declared")
        .span;
    let arguments = marker
        .map(|marker| {
            vec![HirCallArgument::Value(HirExpression {
                kind: HirExpressionKind::I64(marker),
                ty: Type::I64,
                span,
            })]
        })
        .unwrap_or_default();
    HirExpression {
        kind: HirExpressionKind::PresenceTest {
            source: HirOptionalOperand::ClassProduced(Box::new(HirExpression {
                kind: HirExpressionKind::DirectCall {
                    function,
                    arguments,
                },
                ty: Type::OptionalClass(class),
                span,
            })),
            kind: HirPresenceTestKind::Some,
        },
        ty: Type::Bool,
        span,
    }
}

fn lower_optional_logical(
    source: &str,
    operation: HirLogicalOperation,
    right: Option<i64>,
) -> MirProgram {
    let mut hir = type_check_source(source).hir.unwrap();
    let left = optional_presence_call(&hir, "present", Some(1));
    let right = match right {
        Some(marker) => optional_presence_call(&hir, "present", Some(marker)),
        None => optional_presence_call(&hir, "absent", None),
    };
    replace_return_with_logical_expressions(&mut hir, "evaluate", operation, left, right);
    lower_hir(&hir)
}

const OBSERVABLE_OPTIONAL_OPERANDS: &str = concat!(
    "extern fn test_record_i64(value: i64) -> unit;\n",
    "class Trace {\n",
    "  marker: i64;\n",
    "  init(marker: i64) { self.marker = marker; }\n",
    "  destroy { test_record_i64(self.marker); }\n",
    "}\n",
    "fn present(marker: i64) -> Trace? { return Trace(marker); }\n",
    "fn absent() -> Trace? { return none; }\n",
    "fn inspect_optional(value: Trace?) -> bool { return value is some; }\n",
    "fn argument_present() -> bool { return inspect_optional(Trace(2)); }\n",
    "fn argument_absent() -> bool { return inspect_optional(none); }\n",
    "fn always_true() -> bool { return true; }\n",
    "fn evaluate() -> bool { return false; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn selected_class_optional_results_remain_initialized_until_cleanup() {
    for (operation, right, expected) in [
        (HirLogicalOperation::And, Some(2), 1),
        (HirLogicalOperation::Or, Some(2), 1),
        (HirLogicalOperation::And, None, 0),
    ] {
        let mir = lower_optional_logical(OPTIONAL_OPERANDS, operation, right);
        verify_mir(&mir).unwrap();
        let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
        assert_eq!(run_native_assembly(&assembly).code(), Some(expected));
    }
}

#[test]
fn class_optional_destruction_observes_presence_skip_and_reverse_order() {
    for (operation, right, status, expected) in [
        (HirLogicalOperation::And, Some(2), 1, b"2\n1\n".as_slice()),
        (HirLogicalOperation::Or, Some(2), 1, b"1\n".as_slice()),
        (HirLogicalOperation::And, None, 0, b"1\n".as_slice()),
    ] {
        let mir = lower_optional_logical(OBSERVABLE_OPTIONAL_OPERANDS, operation, right);
        verify_mir(&mir).unwrap();
        let output = native_output(&mir);
        assert_eq!(output.status.code(), Some(status));
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn conditional_class_optional_arguments_publish_only_on_the_selected_path() {
    for (operation, right, status, expected) in [
        (
            HirLogicalOperation::And,
            "argument_present",
            1,
            b"2\n".as_slice(),
        ),
        (
            HirLogicalOperation::Or,
            "argument_present",
            1,
            b"".as_slice(),
        ),
        (
            HirLogicalOperation::And,
            "argument_absent",
            0,
            b"".as_slice(),
        ),
    ] {
        let mir = lower_fixture_logical(
            OBSERVABLE_OPTIONAL_OPERANDS,
            "evaluate",
            operation,
            "always_true",
            right,
        );
        verify_mir(&mir).unwrap();
        let output = native_output(&mir);
        assert_eq!(output.status.code(), Some(status));
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn verifier_rejects_lost_duplicate_and_incompatible_conditional_optional_state() {
    let valid = lower_optional_logical(OPTIONAL_OPERANDS, HirLogicalOperation::And, Some(2));
    let evaluate = function_id_from_mir(&valid, "evaluate");

    let mut lost = valid.clone();
    let definition = lost.definitions.get_mut_for_test(evaluate).unwrap();
    let cleanup_block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::ClassOptionalCleanup(_)))
        })
        .expect("conditional optional fixture must contain cleanup");
    cleanup_block
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::ClassOptionalCleanup(_)));
    let errors = verify_mir(&lost).unwrap_err().to_string();
    assert!(errors.contains(
        "initialized class optional reaches storage-dead without cleanup or ownership transfer"
    ));

    let mut duplicate = valid.clone();
    let definition = duplicate.definitions.get_mut_for_test(evaluate).unwrap();
    let cleanup_block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::ClassOptionalCleanup(_)))
        })
        .unwrap();
    let cleanup = cleanup_block
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::ClassOptionalCleanup(_)))
        .unwrap()
        .clone();
    cleanup_block.instructions.push(cleanup);
    let errors = verify_mir(&duplicate).unwrap_err().to_string();
    assert!(errors.contains("class optional cleanup destination is not definitely initialized"));

    let mut incompatible = valid;
    let definition = incompatible.definitions.get_mut_for_test(evaluate).unwrap();
    let right_storage = definition
        .storage
        .iter()
        .filter(|storage| {
            storage.kind == MirStorageKind::Temporary
                && matches!(storage.ty, MirType::OptionalClass(_))
        })
        .map(|storage| storage.id)
        .max_by_key(|storage| storage.index())
        .expect("logical right operand must produce optional storage");
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| match instruction {
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                cleanup.destination.base.storage() != right_storage
            }
            MirInstruction::StorageDead(operation) => operation.storage != right_storage,
            _ => true,
        });
    }
    let errors = verify_mir(&incompatible).unwrap_err().to_string();
    assert!(
        errors.contains("conditional optional initialization state remains when path condition")
    );
}
