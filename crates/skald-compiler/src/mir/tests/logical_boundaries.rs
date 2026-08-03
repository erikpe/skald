//! Bounded lifetime, failure, and enclosing-control-flow logical expressions.

use super::logical_fixtures::{
    boolean, function_id, function_id_from_mir, logical_expression, lower_fixture_logical,
    native_output, replace_return_with_logical_expressions, returned_scalar,
};
use super::*;
use crate::{
    hir::{
        HirCallArgument, HirExpression, HirExpressionKind, HirLocalInitializer,
        HirLogicalOperation, HirOptionalOperand, HirProgram, HirSharedProducer, HirSharedSource,
        HirStatement, HirViewSource, Type,
    },
    test_support::{load_module_sources_with_standard_library, type_check_source},
};

const BOUNDED_OPERANDS: &str = concat!(
    "class Item {\n",
    "  truth: bool;\n",
    "  init(truth: bool) { self.truth = truth; }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy {}\n",
    "}\n",
    "class Holder {\n",
    "  item: Item?;\n",
    "  init(item: Item?) { self.item = item; }\n",
    "  destroy {}\n",
    "}\n",
    "fn make_holder(truth: bool) -> shared Holder { return new Holder(Item(truth)); }\n",
    "fn primitive(truth: bool) -> bool? { return truth; }\n",
    "fn maybe_item(truth: bool) -> shared? Item { return new Item(truth); }\n",
    "fn payload_field() -> bool { return (*make_holder(true)).item!.truth; }\n",
    "fn payload_method() -> bool { return (*make_holder(false)).item!.read(); }\n",
    "fn primitive_unwrap() -> bool { return primitive(false)!; }\n",
    "fn shared_unwrap() -> bool { return maybe_item(true)!->read(); }\n",
    "fn left() -> bool { return true; }\n",
    "fn evaluate() -> bool { return false; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

#[test]
fn bounded_optional_views_end_inside_the_selected_operand() {
    for (right, expected) in [("payload_field", 1), ("payload_method", 0)] {
        let selected = lower_fixture_logical(
            BOUNDED_OPERANDS,
            "evaluate",
            HirLogicalOperation::And,
            "left",
            right,
        );
        verify_mir(&selected).unwrap_or_else(|errors| panic!("{right}: {errors}"));
        assert_eq!(native_output(&selected).status.code(), Some(expected));

        let evaluate = selected
            .definitions
            .get(function_id_from_mir(&selected, "evaluate"))
            .unwrap();
        let logical = &evaluate.body.logical_expressions[0];
        let right_exit = &evaluate.body.blocks[logical.right_exit.index()];
        let end = right_exit
            .instructions
            .iter()
            .position(|instruction| matches!(instruction, MirInstruction::EndOptionalView(_)))
            .expect("the immediate payload consumer must end its optional view");
        let store = right_exit
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Store(store)
                        if store.destination.base.storage() == logical.result
                )
            })
            .expect("the selected operand must publish its scalar result");
        assert!(end < store);

        let skipped = lower_fixture_logical(
            BOUNDED_OPERANDS,
            "evaluate",
            HirLogicalOperation::Or,
            "left",
            right,
        );
        verify_mir(&skipped).unwrap_or_else(|errors| panic!("{right}: {errors}"));
        assert_eq!(native_output(&skipped).status.code(), Some(1));
    }
}

#[test]
fn primitive_unwrap_copies_its_result_before_conditional_storage_cleanup() {
    let mir = lower_fixture_logical(
        BOUNDED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "primitive_unwrap",
    );
    verify_mir(&mir).unwrap();
    assert_eq!(native_output(&mir).status.code(), Some(0));

    let evaluate = mir
        .definitions
        .get(function_id_from_mir(&mir, "evaluate"))
        .unwrap();
    let logical = &evaluate.body.logical_expressions[0];
    let unwrap = evaluate
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::OptionalUnwrap)
        .expect("the primitive unwrap must use bounded payload storage");
    let loaded = evaluate.body.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if matches!(
                        &assignment.rvalue.kind,
                        MirRvalueKind::Load(place)
                            if place.base.storage() == unwrap.id
                    )
            )
        })
    });
    let ended = evaluate.body.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                MirInstruction::StorageDead(dead) if dead.storage == unwrap.id
            )
        })
    });
    assert!(loaded && ended);
    assert_eq!(
        evaluate.body.blocks[logical.right_exit.index()]
            .instructions
            .last()
            .and_then(|instruction| match instruction {
                MirInstruction::Store(store) => Some(store.destination.base.storage()),
                _ => None,
            }),
        Some(logical.result)
    );
}

#[test]
fn optional_shared_unwrap_uses_a_secured_owner_not_a_bounded_payload_guard() {
    let mir = lower_fixture_logical(
        BOUNDED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "shared_unwrap",
    );
    verify_mir(&mir).unwrap();
    assert_eq!(native_output(&mir).status.code(), Some(1));

    let evaluate = mir
        .definitions
        .get(function_id_from_mir(&mir, "evaluate"))
        .unwrap();
    assert!(evaluate.body.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            Some(MirTerminator::OptionalSharedUnwrap { .. })
        )
    }));
    assert!(evaluate.body.blocks.iter().any(|block| {
        block
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
    }));
    assert!(!evaluate.body.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            Some(MirTerminator::BeginOptionalView { .. })
        )
    }));
}

const CONSUMER_CONTEXTS: &str = concat!(
    "class Flag {\n",
    "  truth: bool;\n",
    "  init(truth: bool) { self.truth = truth; }\n",
    "  fn read() -> bool { return self.truth; }\n",
    "  destroy {}\n",
    "}\n",
    "fn consume(value: bool) -> bool { return value; }\n",
    "fn make(truth: bool) -> shared Flag { return new Flag(truth); }\n",
    "fn erase(truth: bool) -> shared Obj { return new Flag(truth); }\n",
    "fn maybe(truth: bool) -> bool? { return truth; }\n",
    "fn flags() -> bool[] {\n",
    "  var values: bool[] = bool[](2u);\n",
    "  values[0] = true;\n",
    "  values[1] = false;\n",
    "  return values;\n",
    "}\n",
    "fn choose_index(first: bool) -> i64 {\n",
    "  if (first) { return 0; }\n",
    "  return 1;\n",
    "}\n",
    "fn call_context() -> bool { return consume(false); }\n",
    "fn receiver_context() -> bool { return make(false)->read(); }\n",
    "fn field_context() -> bool { return (*make(false)).truth; }\n",
    "fn index_context() -> bool { return flags()[choose_index(false)]; }\n",
    "fn type_context() -> bool { return *erase(false) is Flag; }\n",
    "fn presence_context() -> bool { return maybe(false) is some; }\n",
    "fn assignment_context() -> bool {\n",
    "  var value: bool = false;\n",
    "  value = false;\n",
    "  return value;\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  if (!call_context()) { return 1; }\n",
    "  if (receiver_context()) { return 2; }\n",
    "  if (!field_context()) { return 3; }\n",
    "  if (!index_context()) { return 4; }\n",
    "  if (!type_context()) { return 5; }\n",
    "  if (!presence_context()) { return 6; }\n",
    "  if (!assignment_context()) { return 7; }\n",
    "  return 0;\n",
    "}\n",
);

#[test]
fn logical_control_flow_composes_inside_existing_expression_consumers() {
    let mut hir = type_check_source(CONSUMER_CONTEXTS).hir.unwrap();
    for (function, operation, right) in [
        ("call_context", HirLogicalOperation::Or, false),
        ("receiver_context", HirLogicalOperation::And, false),
        ("field_context", HirLogicalOperation::Or, false),
        ("index_context", HirLogicalOperation::And, true),
        ("type_context", HirLogicalOperation::And, false),
        ("presence_context", HirLogicalOperation::Or, false),
    ] {
        let function = function_id(&hir, function);
        let expression = super::logical_fixtures::returned_scalar_mut(
            hir.definitions.get_mut_for_test(function).unwrap(),
        );
        let span = expression.span;
        let replacement =
            logical_expression(operation, boolean(true, span), boolean(right, span), span);
        assert!(
            replace_first_false_expression(expression, &replacement),
            "fixture must contain one nested boolean argument"
        );
    }

    let assignment = function_id(&hir, "assignment_context");
    let definition = hir.definitions.get_mut_for_test(assignment).unwrap();
    let HirStatement::Local(local) = &mut definition.body.statements[0] else {
        panic!("assignment fixture must start with a local");
    };
    let HirLocalInitializer::Value(initializer) = &mut local.initializer else {
        panic!("assignment fixture local must have a scalar initializer");
    };
    let span = initializer.span;
    *initializer = logical_expression(
        HirLogicalOperation::And,
        boolean(true, span),
        boolean(true, span),
        span,
    );
    let HirStatement::PrimitiveAssignment(assignment) = &mut definition.body.statements[1] else {
        panic!("assignment fixture must contain a primitive reassignment");
    };
    let span = assignment.source.span;
    assignment.source = logical_expression(
        HirLogicalOperation::Or,
        boolean(false, span),
        boolean(true, span),
        span,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    assert_eq!(native_output(&mir).status.code(), Some(0));
    for function in [
        "call_context",
        "receiver_context",
        "field_context",
        "index_context",
        "type_context",
        "presence_context",
        "assignment_context",
    ] {
        assert!(
            !mir.definitions
                .get(function_id_from_mir(&mir, function))
                .unwrap()
                .body
                .logical_expressions
                .is_empty(),
            "{function}"
        );
    }
}

fn replace_first_false_expression(
    expression: &mut HirExpression,
    replacement: &HirExpression,
) -> bool {
    match &mut expression.kind {
        HirExpressionKind::Boolean(false) => {
            *expression = replacement.clone();
            true
        }
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::StaticCall { arguments, .. } => {
            replace_first_false_argument(arguments, replacement)
        }
        HirExpressionKind::FieldRead(place) => place
            .shared_view
            .as_deref_mut()
            .is_some_and(|view| replace_first_false_view_source(&mut view.source, replacement)),
        HirExpressionKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            receiver
                .shared_view
                .as_deref_mut()
                .is_some_and(|view| replace_first_false_view_source(&mut view.source, replacement))
                || replace_first_false_argument(arguments, replacement)
        }
        HirExpressionKind::TypeTest(test) => {
            replace_first_false_view_source(&mut test.source.source, replacement)
        }
        HirExpressionKind::PresenceTest { source, .. } => match source {
            HirOptionalOperand::Produced(expression)
            | HirOptionalOperand::ClassProduced(expression)
            | HirOptionalOperand::SharedProduced(expression) => {
                replace_first_false_expression(expression, replacement)
            }
            HirOptionalOperand::Place(_)
            | HirOptionalOperand::ClassPlace(_)
            | HirOptionalOperand::SharedPlace(_) => false,
        },
        HirExpressionKind::ArrayElement(element) => {
            replace_first_false_expression(&mut element.index.value, replacement)
        }
        _ => false,
    }
}

fn replace_first_false_argument(
    arguments: &mut [HirCallArgument],
    replacement: &HirExpression,
) -> bool {
    arguments.iter_mut().any(|argument| match argument {
        HirCallArgument::Value(expression) => {
            replace_first_false_expression(expression, replacement)
        }
        _ => false,
    })
}

fn replace_first_false_view_source(
    source: &mut HirViewSource,
    replacement: &HirExpression,
) -> bool {
    let HirViewSource::AnchoredShared { source, .. } = source else {
        return false;
    };
    let HirSharedSource::Produced(HirSharedProducer::Call(call)) = source.as_mut() else {
        return false;
    };
    replace_first_false_expression(call, replacement)
}

const FAILURE_OPERANDS: &str = concat!(
    "from std::error import panic;\n",
    "class Wanted {\n",
    "  init() {}\n",
    "  fn read() -> bool { return true; }\n",
    "  destroy {}\n",
    "}\n",
    "class Other { init() {} destroy {} }\n",
    "fn erased_other() -> shared Obj { return new Other(); }\n",
    "fn absent() -> bool? { return none; }\n",
    "fn optional_failure() -> bool { return absent()!; }\n",
    "fn cast_failure() -> bool { return ((Wanted) *erased_other()).read(); }\n",
    "fn empty() -> bool[] {\n",
    "  var values: bool[] = bool[](0u);\n",
    "  return values;\n",
    "}\n",
    "fn bounds_failure() -> bool { return empty()[0]; }\n",
    "fn impossible_array() -> bool[] {\n",
    "  var values: bool[] = bool[](9223372036854775807u);\n",
    "  return values;\n",
    "}\n",
    "fn allocation_failure() -> bool { return impossible_array().len() == 0u; }\n",
    "fn called_panic() -> bool { panic(\"selected panic\"); }\n",
    "fn always_true() -> bool { return true; }\n",
    "fn always_false() -> bool { return false; }\n",
    "fn evaluate() -> bool { return false; }\n",
    "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
);

fn failure_hir() -> HirProgram {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", FAILURE_OPERANDS)]);
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    checked
        .hir
        .unwrap_or_else(|| panic!("failure fixture must type-check: {:?}", checked.diagnostics))
}

fn lower_failure_logical(operation: HirLogicalOperation, left: &str, right: &str) -> MirProgram {
    let mut hir = failure_hir();
    let operand = |program: &HirProgram, name: &str| {
        let function = function_id(program, name);
        if name == "called_panic" {
            let span = program
                .declarations
                .get(function)
                .expect("called panic fixture must be declared")
                .span;
            HirExpression {
                kind: HirExpressionKind::DirectCall {
                    function,
                    arguments: Vec::new(),
                },
                ty: Type::Bool,
                span,
            }
        } else {
            returned_scalar(
                program
                    .definitions
                    .get(function)
                    .expect("failure fixture must have a body"),
            )
            .clone()
        }
    };
    let left = operand(&hir, left);
    let right = operand(&hir, right);
    replace_return_with_logical_expressions(&mut hir, "evaluate", operation, left, right);
    lower_hir(&hir)
}

#[test]
fn failure_checks_exist_only_on_the_selected_right_path() {
    for failure in [
        "optional_failure",
        "cast_failure",
        "bounds_failure",
        "allocation_failure",
        "called_panic",
    ] {
        let skipped_and = lower_failure_logical(HirLogicalOperation::And, "always_false", failure);
        verify_mir(&skipped_and).unwrap_or_else(|errors| panic!("{failure}: {errors}"));
        assert_eq!(
            native_output(&skipped_and).status.code(),
            Some(0),
            "{failure}"
        );

        let skipped_or = lower_failure_logical(HirLogicalOperation::Or, "always_true", failure);
        verify_mir(&skipped_or).unwrap_or_else(|errors| panic!("{failure}: {errors}"));
        assert_eq!(
            native_output(&skipped_or).status.code(),
            Some(1),
            "{failure}"
        );

        let selected = lower_failure_logical(HirLogicalOperation::And, "always_true", failure);
        verify_mir(&selected).unwrap_or_else(|errors| panic!("{failure}: {errors}"));
        assert!(
            !native_output(&selected).status.success(),
            "{failure} must terminate when selected"
        );
    }
}

#[test]
fn a_left_failure_precedes_selection_and_never_reaches_the_right_operand() {
    let mir = lower_failure_logical(HirLogicalOperation::Or, "optional_failure", "called_panic");
    verify_mir(&mir).unwrap();
    let output = native_output(&mir);
    assert!(!output.status.success());
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("selected panic"),
        "the right-side panic must remain unreachable after a left failure"
    );
}

const CONTROL_FLOW: &str = concat!(
    "class Counter {\n",
    "  count: i64;\n",
    "  init() { self.count = 0; }\n",
    "  mut fn next() -> bool {\n",
    "    self.count = self.count + 1;\n",
    "    return self.count < 3;\n",
    "  }\n",
    "  destroy {}\n",
    "}\n",
    "fn condition() -> bool { return true; }\n",
    "fn fallback() -> bool { return true; }\n",
    "fn evaluate_if() -> i64 {\n",
    "  if (condition()) { return 1; }\n",
    "  elif (fallback()) { return 2; }\n",
    "  return 3;\n",
    "}\n",
    "fn evaluate_loop() -> i64 {\n",
    "  var counter: Counter = Counter();\n",
    "  while (counter.next()) {}\n",
    "  return counter.count;\n",
    "}\n",
    "fn main() -> i64 { return evaluate_if() + evaluate_loop(); }\n",
);

#[test]
fn logical_conditions_cleanup_before_if_elif_and_loop_successors() {
    let mut hir = type_check_source(CONTROL_FLOW).hir.unwrap();

    let evaluate_if = function_id(&hir, "evaluate_if");
    let definition = hir.definitions.get_mut_for_test(evaluate_if).unwrap();
    let HirStatement::Conditional(conditional) = &mut definition.body.statements[0] else {
        panic!("fixture must start with a conditional");
    };
    let first_span = conditional.arms[0].condition.span;
    let first = conditional.arms[0].condition.clone();
    conditional.arms[0].condition = logical_expression(
        HirLogicalOperation::And,
        first,
        boolean(false, first_span),
        first_span,
    );
    let second_span = conditional.arms[1].condition.span;
    let second = conditional.arms[1].condition.clone();
    conditional.arms[1].condition = logical_expression(
        HirLogicalOperation::And,
        boolean(true, second_span),
        second,
        second_span,
    );

    let evaluate_loop = function_id(&hir, "evaluate_loop");
    let definition = hir.definitions.get_mut_for_test(evaluate_loop).unwrap();
    let HirStatement::While(loop_statement) = &mut definition.body.statements[1] else {
        panic!("fixture must contain a while loop");
    };
    let span = loop_statement.condition.span;
    let condition = loop_statement.condition.clone();
    loop_statement.condition = logical_expression(
        HirLogicalOperation::And,
        condition,
        boolean(true, span),
        span,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    assert_eq!(native_output(&mir).status.code(), Some(5));

    for function in [evaluate_if, evaluate_loop] {
        let definition = mir.definitions.get(function).unwrap();
        for condition in &definition.body.path_conditions {
            assert!(definition.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::StorageDead(dead)
                            if dead.storage == condition.activation
                    )
                })
            }));
        }
    }
}

#[test]
fn logical_return_result_is_secured_until_after_conditional_cleanup() {
    let mut hir = type_check_source(BOUNDED_OPERANDS).hir.unwrap();
    let left = returned_scalar(
        hir.definitions
            .get(function_id(&hir, "payload_field"))
            .expect("left fixture must have a body"),
    )
    .clone();
    let right = returned_scalar(
        hir.definitions
            .get(function_id(&hir, "payload_method"))
            .expect("right fixture must have a body"),
    )
    .clone();
    replace_return_with_logical_expressions(
        &mut hir,
        "evaluate",
        HirLogicalOperation::Or,
        left,
        right,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    assert_eq!(native_output(&mir).status.code(), Some(1));

    let evaluate = mir
        .definitions
        .get(function_id_from_mir(&mir, "evaluate"))
        .unwrap();
    let return_block = evaluate
        .body
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(MirTerminator::Return { .. })))
        .expect("logical result must eventually return");
    let return_value = match return_block.terminator {
        Some(MirTerminator::Return {
            value: Some(value), ..
        }) => value,
        _ => unreachable!(),
    };
    assert!(return_block.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::Assign(assignment) if assignment.result == return_value
        )
    }));
}

#[test]
fn verifiers_reject_leaked_guards_and_returning_failure_edges() {
    let valid = lower_fixture_logical(
        BOUNDED_OPERANDS,
        "evaluate",
        HirLogicalOperation::And,
        "left",
        "payload_field",
    );
    let evaluate = function_id_from_mir(&valid, "evaluate");

    let mut leaked = valid;
    let definition = leaked.definitions.get_mut_for_test(evaluate).unwrap();
    for block in &mut definition.body.blocks {
        block
            .instructions
            .retain(|instruction| !matches!(instruction, MirInstruction::EndOptionalView(_)));
    }
    let errors = verify_mir(&leaked).unwrap_err().to_string();
    assert!(
        errors.contains("optional payload guard")
            || errors.contains("optional guard state differs across control-flow paths"),
        "{errors}"
    );

    let mut returning_failure =
        lower_failure_logical(HirLogicalOperation::And, "always_true", "optional_failure");
    let evaluate = function_id_from_mir(&returning_failure, "evaluate");
    let definition = returning_failure
        .definitions
        .get_mut_for_test(evaluate)
        .unwrap();
    let failure = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalUnwrap { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .expect("fixture must contain an optional failure edge");
    let span = definition.body.blocks[failure.index()].span;
    definition.body.blocks[failure.index()].terminator =
        Some(MirTerminator::Return { value: None, span });
    assert!(verify_mir(&returning_failure)
        .unwrap_err()
        .to_string()
        .contains("optional unwrap failure edge must terminate"));

    let mut externally_reachable_failure =
        lower_failure_logical(HirLogicalOperation::And, "always_true", "bounds_failure");
    let evaluate = function_id_from_mir(&externally_reachable_failure, "evaluate");
    let definition = externally_reachable_failure
        .definitions
        .get_mut_for_test(evaluate)
        .unwrap();
    let failure = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::ArrayPositionCheck { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .expect("fixture must contain an array bounds failure edge");
    let external = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
                && matches!(block.terminator, Some(MirTerminator::Goto { .. }))
        })
        .expect("conditional cleanup must contain an external edge");
    let Some(MirTerminator::Goto { target, .. }) = &mut external.terminator else {
        unreachable!()
    };
    *target = failure;
    let errors = verify_mir(&externally_reachable_failure)
        .unwrap_err()
        .to_string();
    assert!(
        errors.contains("has an incoming edge outside its selected region"),
        "{errors}"
    );
}
