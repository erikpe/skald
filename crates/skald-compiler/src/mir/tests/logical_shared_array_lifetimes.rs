use super::logical_fixtures::{
    function_id, function_id_from_mir, lower_fixture_logical, native_output,
    replace_return_with_logical_expressions, returned_scalar,
};
use super::*;
use crate::{
    hir::{HirExpression, HirExpressionKind, HirLogicalExpression, HirLogicalOperation, Type},
    test_support::type_check_source,
};

const SHARED_OPERANDS: &str = concat!(
    "extern fn test_record_i64(value: i64) -> unit;\n",
    "class Trace {\n",
    "  marker: i64;\n",
    "  truth: bool;\n",
    "  init(marker: i64, truth: bool) {\n",
    "    self.marker = marker;\n",
    "    self.truth = truth;\n",
    "  }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy { test_record_i64(self.marker); }\n",
    "}\n",
    "fn make(marker: i64, truth: bool) -> shared Trace {\n",
    "  return new Trace(marker, truth);\n",
    "}\n",
    "fn erase(marker: i64, truth: bool) -> shared Obj {\n",
    "  return new Trace(marker, truth);\n",
    "}\n",
    "fn maybe(marker: i64, truth: bool) -> shared? Trace {\n",
    "  return new Trace(marker, truth);\n",
    "}\n",
    "fn inspect(value: shared Trace) -> bool { return value->read(); }\n",
    "fn left() -> bool { return make(1, true)->read(); }\n",
    "fn right() -> bool { return make(2, false)->read(); }\n",
    "fn right_argument() -> bool { return inspect(new Trace(2, false)); }\n",
    "fn right_type_test() -> bool { return *make(2, false) is Trace; }\n",
    "fn right_checked() -> bool { return ((Trace) *erase(2, false)).read(); }\n",
    "fn right_optional() -> bool { return maybe(2, false)!->read(); }\n",
    "fn right_presence() -> bool { return maybe(2, false) is some; }\n",
    "fn last() -> bool { return make(3, true)->read(); }\n",
    "fn evaluate() -> bool { return false; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn selected_shared_receivers_release_in_reverse_order_and_skipped_receivers_do_nothing() {
    let selected = lower_fixture_logical(
        SHARED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "right",
    );
    verify_mir(&selected).unwrap();
    let output = native_output(&selected);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"2\n1\n");

    let skipped = lower_fixture_logical(
        SHARED_OPERANDS,
        "evaluate",
        HirLogicalOperation::Or,
        "left",
        "right",
    );
    verify_mir(&skipped).unwrap();
    let output = native_output(&skipped);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"1\n");
}

#[test]
fn shared_arguments_type_tests_checked_places_and_optional_owners_are_path_sensitive() {
    for (right, expected, selected_output) in [
        ("right_argument", 0, b"2\n1\n".as_slice()),
        ("right_type_test", 1, b"2\n1\n".as_slice()),
        ("right_checked", 0, b"2\n1\n".as_slice()),
        ("right_optional", 0, b"2\n1\n".as_slice()),
        ("right_presence", 1, b"2\n1\n".as_slice()),
    ] {
        let selected = lower_fixture_logical(
            SHARED_OPERANDS,
            "evaluate",
            HirLogicalOperation::And,
            "left",
            right,
        );
        verify_mir(&selected).unwrap_or_else(|errors| panic!("{right}: {errors}"));
        let output = native_output(&selected);
        assert_eq!(output.status.code(), Some(expected), "{right}");
        assert_eq!(output.stdout, selected_output, "{right}");

        let skipped = lower_fixture_logical(
            SHARED_OPERANDS,
            "evaluate",
            HirLogicalOperation::Or,
            "left",
            right,
        );
        verify_mir(&skipped).unwrap_or_else(|errors| panic!("{right}: {errors}"));
        let output = native_output(&skipped);
        assert_eq!(output.status.code(), Some(1), "{right}");
        assert_eq!(output.stdout, b"1\n", "{right}");
    }
}

#[test]
fn nested_shared_registrations_preserve_all_selected_owners_until_reverse_cleanup() {
    let mut hir = type_check_source(SHARED_OPERANDS).hir.unwrap();
    let operand = |name| {
        returned_scalar(
            hir.definitions
                .get(function_id(&hir, name))
                .expect("operand fixture function must have a body"),
        )
        .clone()
    };
    let left = operand("left");
    let right = operand("right");
    let last = operand("last");
    let span = left.span;
    let inner = HirExpression {
        kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
            HirLogicalOperation::And,
            left,
            right,
        ))),
        ty: Type::Bool,
        span,
    };
    replace_return_with_logical_expressions(
        &mut hir,
        "evaluate",
        HirLogicalOperation::Or,
        inner,
        last,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let output = native_output(&mir);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"3\n2\n1\n");
}

const BOOLEAN_ARRAY_OPERANDS: &str = concat!(
    "class Item {\n",
    "  truth: bool;\n",
    "  init() { self.truth = false; }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy {}\n",
    "}\n",
    "fn flags(truth: bool) -> bool[] {\n",
    "  var values: bool[] = bool[](1u);\n",
    "  values[0] = truth;\n",
    "  return values;\n",
    "}\n",
    "fn evaluate() -> bool { return flags(true)[0] == flags(false)[0]; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

const OBJECT_ARRAY_OPERANDS: &str = concat!(
    "class Item {\n",
    "  truth: bool;\n",
    "  init() { self.truth = false; }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy {}\n",
    "}\n",
    "fn items(truth: bool) -> Item[] {\n",
    "  var values: Item[] = Item[](1u);\n",
    "  values[0].truth = truth;\n",
    "  return values;\n",
    "}\n",
    "fn evaluate() -> bool { return items(true)[0].read() == items(false)[0].read(); }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

const SHARED_ARRAY_OPERANDS: &str = concat!(
    "class Item {\n",
    "  truth: bool;\n",
    "  init() { self.truth = false; }\n",
    "  init(truth: bool) { self.truth = truth; }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy {}\n",
    "}\n",
    "fn items(truth: bool) -> (shared Item)[] {\n",
    "  var values: (shared Item)[] = (shared Item)[](1u);\n",
    "  values[0] = new Item(truth);\n",
    "  return values;\n",
    "}\n",
    "fn inspect(value: shared Item) -> bool { return value->read(); }\n",
    "fn evaluate() -> bool {\n",
    "  var left: (shared Item)[] = items(true);\n",
    "  var right: (shared Item)[] = items(false);\n",
    "  return inspect(left[0]) == inspect(right[0]);\n",
    "}\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

fn lower_eager_pair_as_logical(source: &str, operation: HirLogicalOperation) -> MirProgram {
    let checked = type_check_source(source);
    let mut hir = checked
        .hir
        .unwrap_or_else(|| panic!("array fixture must type-check: {:?}", checked.diagnostics));
    let evaluate = function_id(&hir, "evaluate");
    let expression = returned_scalar(hir.definitions.get(evaluate).unwrap()).clone();
    let (left, right) = match expression.kind {
        HirExpressionKind::Binary { left, right, .. }
        | HirExpressionKind::PrimitiveComparison { left, right, .. } => (*left, *right),
        _ => panic!("array fixture must establish both operands in one eager expression"),
    };
    replace_return_with_logical_expressions(&mut hir, "evaluate", operation, left, right);
    lower_hir(&hir)
}

#[test]
fn produced_arrays_elements_and_anchors_follow_logical_selection() {
    for source in [
        BOOLEAN_ARRAY_OPERANDS,
        OBJECT_ARRAY_OPERANDS,
        SHARED_ARRAY_OPERANDS,
    ] {
        let selected = lower_eager_pair_as_logical(source, HirLogicalOperation::And);
        verify_mir(&selected).unwrap();
        assert_eq!(native_output(&selected).status.code(), Some(0));

        let skipped = lower_eager_pair_as_logical(source, HirLogicalOperation::Or);
        verify_mir(&skipped).unwrap();
        assert_eq!(native_output(&skipped).status.code(), Some(1));

        let dump = dump_mir(&selected);
        assert!(dump.contains("array-op AnchorBegin"));
        assert!(dump.contains("array-op AnchorEnd"));
        assert!(dump.contains("array-op Release"));
    }
}

const RETAINED_SHARED_OPERANDS: &str = concat!(
    "extern fn test_record_i64(value: i64) -> unit;\n",
    "class Item {\n",
    "  marker: i64;\n",
    "  init(marker: i64) { self.marker = marker; }\n",
    "  fn read() -> bool { return true; }\n",
    "  destroy { test_record_i64(self.marker); }\n",
    "}\n",
    "fn inspect(value: shared Item) -> bool { return value->read(); }\n",
    "fn evaluate() -> bool {\n",
    "  var owner: shared Item = new Item(4);\n",
    "  return owner->read() == inspect(owner);\n",
    "}\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn conditional_retained_argument_does_not_release_the_stable_owner() {
    for operation in [HirLogicalOperation::And, HirLogicalOperation::Or] {
        let mir = lower_eager_pair_as_logical(RETAINED_SHARED_OPERANDS, operation);
        verify_mir(&mir).unwrap();
        let output = native_output(&mir);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, b"4\n");
        assert!(dump_mir(&mir).contains("shared-copy"));
    }
}

#[test]
fn shared_verifier_rejects_lost_duplicate_and_early_selected_cleanup() {
    let valid = lower_fixture_logical(
        SHARED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "right_checked",
    );
    let evaluate = function_id_from_mir(&valid, "evaluate");

    let mut lost = valid.clone();
    let definition = lost.definitions.get_mut_for_test(evaluate).unwrap();
    let right_owner = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::SharedRelease(release) => Some(release.owner),
            _ => None,
        })
        .max_by_key(|storage| storage.index())
        .expect("logical right operand must produce a shared owner");
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::SharedRelease(release) if release.owner == right_owner
            )
        });
    }
    let errors = verify_mir(&lost).unwrap_err().to_string();
    assert!(errors.contains("shared owner remains live on normal return"));

    let mut duplicate = valid.clone();
    let definition = duplicate.definitions.get_mut_for_test(evaluate).unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        })
        .unwrap();
    let release = block
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap()
        .clone();
    block.instructions.push(release);
    let errors = verify_mir(&duplicate).unwrap_err().to_string();
    assert!(errors.contains("shared owner is released without being live"));

    let mut early = valid;
    let definition = early.definitions.get_mut_for_test(evaluate).unwrap();
    for block in &mut definition.body.blocks {
        block
            .instructions
            .retain(|instruction| !matches!(instruction, MirInstruction::EndCheckedView(_)));
    }
    let errors = verify_mir(&early).unwrap_err().to_string();
    assert!(
        errors.contains("shared owner is released before its checked view ends")
            || errors.contains("shared-backed checked view remains live")
    );
}

#[test]
fn optional_shared_verifier_rejects_lost_and_duplicate_selected_cleanup() {
    let valid = lower_fixture_logical(
        SHARED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "right_presence",
    );
    let evaluate = function_id_from_mir(&valid, "evaluate");

    let mut lost = valid.clone();
    let definition = lost.definitions.get_mut_for_test(evaluate).unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::OptionalSharedCleanup(_)))
        })
        .expect("optional shared logical operand must have selected cleanup");
    block
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::OptionalSharedCleanup(_)));
    let errors = verify_mir(&lost).unwrap_err().to_string();
    assert!(errors.contains(
        "initialized optional shared reaches storage-dead without cleanup or ownership transfer"
    ));

    let mut duplicate = valid;
    let definition = duplicate.definitions.get_mut_for_test(evaluate).unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::OptionalSharedCleanup(_)))
        })
        .unwrap();
    let cleanup = block
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::OptionalSharedCleanup(_)))
        .unwrap()
        .clone();
    block.instructions.push(cleanup);
    let errors = verify_mir(&duplicate).unwrap_err().to_string();
    assert!(errors.contains("optional shared cleanup destination is not definitely initialized"));
}

#[test]
fn array_verifier_rejects_lost_selected_cleanup_and_anchor_end() {
    let valid = lower_eager_pair_as_logical(OBJECT_ARRAY_OPERANDS, HirLogicalOperation::And);
    let evaluate = function_id_from_mir(&valid, "evaluate");

    let mut lost_array = valid.clone();
    let definition = lost_array.definitions.get_mut_for_test(evaluate).unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::Release { .. })
                )
            })
        })
        .unwrap();
    block.instructions.retain(|instruction| {
        !matches!(
            instruction,
            MirInstruction::Array(MirArrayInstruction::Release { .. })
        )
    });
    let errors = verify_mir(&lost_array).unwrap_err().to_string();
    assert!(
        errors.contains("array owner state remains active at storage-dead")
            || errors.contains("produced array storage")
    );

    let mut lost_anchor = valid;
    let definition = lost_anchor.definitions.get_mut_for_test(evaluate).unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::AnchorEnd { .. })
                )
            })
        })
        .unwrap();
    block.instructions.retain(|instruction| {
        !matches!(
            instruction,
            MirInstruction::Array(MirArrayInstruction::AnchorEnd { .. })
        )
    });
    let errors = verify_mir(&lost_anchor).unwrap_err().to_string();
    assert!(errors.contains("array anchor") && errors.contains("not ended"));
}
