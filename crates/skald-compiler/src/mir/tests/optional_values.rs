use super::*;
use crate::identity::OptionalTypeId;

const OPTIONAL_SOURCE: &str = "fn main() -> i64 {\n\
    var value: i64? = none;\n\
    if (value is none) { value = 40; } else { value = 41; }\n\
    var copied: i64? = value;\n\
    return copied! + 2;\n\
}\n";

#[test]
fn lowers_primitive_optional_state_and_checked_access_explicitly() {
    let program = lower_text(OPTIONAL_SOURCE);
    verify_mir(&program).expect("lowered primitive optionals must verify");
    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&lower_text(OPTIONAL_SOURCE)));

    let optional = program.optional_for_payload(MirType::I64).unwrap();
    assert!(dump.contains(&format!("Optional {optional} payload i64")));
    assert!(dump.contains(&format!("local f0:l0 \"value\" : optional {optional}")));
    assert!(dump.contains("optional-initialize"));
    assert!(dump.contains("optional-assign"));
    assert!(dump.contains("optional-presence none"));
    assert!(dump.contains("optional-unwrap"));
    assert!(dump.contains("terminate optional-access-failure"));
}

#[test]
fn optional_class_array_elements_clear_and_expose_complete_payload_views() {
    let program = lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  assign(ref source: Item) { self.value = source.value; }\n",
        "}\n",
        "class Bag<T> {\n",
        "  values: T?[];\n",
        "  init() { self.values = T?[](1u); }\n",
        "  mut fn clear() -> unit { self.values[0] = none; }\n",
        "  fn get() -> T { return self.values[0]!; }\n",
        "}\n",
        "fn main() -> i64 { var bag: Bag<Item> = Bag<Item>(); return 0; }\n",
    ));

    verify_mir(&program).expect("optional class array operations must form valid closed MIR");
    let absent_assignments = program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::ClassOptionalAssign(assignment)
                if matches!(assignment.source, MirClassOptionalSource::Absent) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!absent_assignments.is_empty());
    assert!(absent_assignments.iter().all(|assignment| {
        assignment.copy_constructor.is_none() && assignment.copy_assignment.is_none()
    }));
}

#[test]
fn verifier_rejects_malformed_optional_identity_lifecycle_and_boundary_metadata() {
    let error_after = |mutate: fn(&mut MirOptionalType)| {
        let mut program = lower_text(OPTIONAL_SOURCE);
        mutate(&mut program.optional_types.entries_mut_for_test()[0]);
        verify_mir(&program)
            .expect_err("malformed optional metadata must be rejected")
            .to_string()
    };

    let errors = error_after(|optional| optional.id = OptionalTypeId::new(7));
    assert!(errors.contains("optional type table index"), "{errors}");

    let errors = error_after(|optional| {
        optional.lifecycle.cleanup = MirOptionalCleanupPlan::Shared(MirSharedTarget::Obj);
    });
    assert!(
        errors.contains("inconsistent executable lifecycle metadata"),
        "{errors}"
    );

    let errors = error_after(|optional| {
        optional.boundaries.argument = MirOptionalBoundaryPlan::MoveOnly;
    });
    assert!(
        errors.contains("inconsistent boundary lifecycle metadata"),
        "{errors}"
    );
}

#[test]
fn optional_assignment_preserves_initialized_wrapper_state_across_cfg_joins() {
    let program = lower_text(
        "fn main() -> i64 {\n\
           var value: i64? = 1;\n\
           if (value is some) { value = none; } else { value = 2; }\n\
           if (value is none) { return 7; }\n\
           return value!;\n\
         }\n",
    );

    verify_mir(&program).expect("dynamic presence may differ across an initialized-wrapper join");
}

#[test]
fn lowers_optional_array_lifecycle_and_checked_copy_out() {
    let source = "fn forward(value: i64[]?) -> i64[]? { return value; }\n\
        fn main() -> i64 {\n\
          var absent: i64[]? = none;\n\
          var present: i64[]? = some(i64[]{40, 2});\n\
          var copied: i64[]? = forward(present);\n\
          copied = copied;\n\
          present = none;\n\
          var values: i64[] = copied!;\n\
          return values[0] + values[1];\n\
        }\n";
    let program = lower_text(source);
    verify_mir(&program).expect("optional array lifecycle must verify");
    let dump = dump_mir(&program);

    assert!(dump.contains("storage InlineArray"));
    assert!(dump.contains("aggregate-optional-initialize"));
    assert!(dump.contains("aggregate-optional-assign"));
    assert!(dump.contains("aggregate-optional-cleanup"));
    assert!(!dump.contains("nested-optional-"));
    assert!(dump.contains("array-copy"));
    assert!(dump.contains("terminate optional-access-failure"));
}

#[test]
fn verifier_rejects_optional_array_metadata_and_operation_mismatches() {
    let source =
        "fn main() -> i64 { var scalar: i64? = none; var value: i64[]? = none; return 0; }\n";
    let mut metadata = lower_text(source);
    let optional = metadata
        .optional_types
        .entries_mut_for_test()
        .iter_mut()
        .find(|entry| matches!(entry.storage, MirOptionalStorage::InlineArray(_)))
        .expect("source must declare an optional array");
    optional.lifecycle.copy = Some(MirOptionalCopyPlan::Trivial);
    let errors = verify_mir(&metadata).unwrap_err().to_string();
    assert!(
        errors.contains("inconsistent executable lifecycle metadata"),
        "{errors}"
    );

    let mut operation = lower_text(source);
    let scalar = operation.optional_for_payload(MirType::I64).unwrap();
    let main = operation
        .definitions
        .get_mut_for_test(operation.entry_function)
        .unwrap();
    let initialize = main
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::AggregateOptionalInitialize(initialize) => Some(initialize),
            _ => None,
        })
        .expect("optional array local must use aggregate optional initialization");
    initialize.optional = scalar;
    let errors = verify_mir(&operation).unwrap_err().to_string();
    assert!(
        errors.contains("incompatible destination metadata"),
        "{errors}"
    );
}

#[test]
fn lowers_optional_fields_arguments_and_results_as_verified_places() {
    let source = "class Holder {\n\
        value: i64?;\n\
        init(value: i64?) { self.value = value; }\n\
        mut fn replace(value: i64?) -> i64? { self.value = value; return self.value; }\n\
    }\n\
    fn forward(value: i64?) -> i64? { return value; }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(none);\n\
        var result: i64? = holder.replace(forward(42));\n\
        return result!;\n\
    }\n";
    let program = lower_text(source);

    verify_mir(&program).expect("stored and callable primitive optionals must verify");
    let dump = dump_mir(&program);
    assert!(dump.contains("optional-initialize"));
    assert!(dump.contains("optional-argument"));
    assert!(dump.contains("optional-return"));
    assert!(dump.contains(".field"));
}

#[test]
fn verifier_rejects_uninitialized_use_and_mismatched_failure_edges() {
    let mut uninitialized = lower_text(OPTIONAL_SOURCE);
    let function = uninitialized
        .definitions
        .get_mut_for_test(uninitialized.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::OptionalInitialize(initialize) => Some(initialize.clone()),
            _ => None,
        })
        .unwrap();
    function.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::OptionalInitialize(_)));
    function.body.blocks[0].instructions.insert(
        1,
        MirInstruction::OptionalAssign(MirOptionalAssign {
            destination: initialize.destination,
            source: initialize.source,
            authorization: None,
            final_authorization: None,
            span: initialize.span,
        }),
    );
    let errors =
        verify_mir(&uninitialized).expect_err("assignment before initialization must fail");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("not definitely initialized")));

    let mut failure = lower_text(OPTIONAL_SOURCE);
    let function = failure
        .definitions
        .get_mut_for_test(failure.entry_function)
        .unwrap();
    let failure_target = function
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalUnwrap { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .unwrap();
    function.body.blocks[failure_target.index()].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::ObjectCastFailure,
        span: function.span,
    });
    let errors = verify_mir(&failure).expect_err("wrong unwrap failure reason must fail");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("optional unwrap failure edge")));
}

#[test]
fn lowers_checked_class_payload_guards_before_shared_anchor_release() {
    let program = lower_text(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
         class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
         fn make() -> shared Holder { return new Holder(Item(42)); }\n\
         fn main() -> i64 { return (*make()).item!.value; }\n",
    );
    let dump = dump_mir(&program);
    let begin = dump.find("begin-optional-view").unwrap();
    let end = dump.find("end-optional-view").unwrap();
    let release = dump.find("shared-release").unwrap();

    assert!(begin < end, "{dump}");
    assert!(end < release, "{dump}");
    assert!(dump.contains("terminate optional-guard-overflow"));
}

#[test]
fn verifier_rejects_mismatched_leaked_and_misrouted_optional_guards() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
        class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
        fn main() -> i64 { var holder: Holder = Holder(Item(42)); return holder.item!.value; }\n";
    let program = lower_text(source);

    let mut unguarded_use = program.clone();
    let main = unguarded_use
        .definitions
        .get_mut_for_test(unguarded_use.entry_function)
        .unwrap();
    let block = main
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::EndOptionalView(_)))
        })
        .unwrap();
    let end = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndOptionalView(_)))
        .unwrap();
    let end = block.instructions.remove(end);
    block.instructions.insert(0, end);
    assert!(verify_mir(&unguarded_use)
        .unwrap_err()
        .to_string()
        .contains("used without its matching active guard"));

    let mut leaked = program.clone();
    let main = leaked
        .definitions
        .get_mut_for_test(leaked.entry_function)
        .unwrap();
    for block in &mut main.body.blocks {
        block
            .instructions
            .retain(|instruction| !matches!(instruction, MirInstruction::EndOptionalView(_)));
    }
    assert!(verify_mir(&leaked)
        .unwrap_err()
        .to_string()
        .contains("optional payload guard remains active on normal return"));

    let mut mismatched = program.clone();
    let main = mismatched
        .definitions
        .get_mut_for_test(mismatched.entry_function)
        .unwrap();
    let end = main
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndOptionalView(end) => Some(end),
            _ => None,
        })
        .unwrap();
    end.payload = MirType::Class(ClassId::new(1));
    assert!(verify_mir(&mismatched)
        .unwrap_err()
        .to_string()
        .contains("optional-view end has an incompatible guard root"));

    let mut misrouted = program;
    let main = misrouted
        .definitions
        .get_mut_for_test(misrouted.entry_function)
        .unwrap();
    let overflow = main
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::BeginOptionalView {
                overflow_target, ..
            }) => Some(overflow_target),
            _ => None,
        })
        .unwrap();
    main.body.blocks[overflow.index()].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::OptionalAccessFailure,
        span: main.body.blocks[overflow.index()].span,
    });
    assert!(verify_mir(&misrouted)
        .unwrap_err()
        .to_string()
        .contains("optional-view overflow edge must terminate with optional-guard overflow"));
}

#[test]
fn verifier_rejects_reordered_nested_optional_guards() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
        class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
        fn sum(ref left: Item, ref right: Item) -> i64 { return left.value + right.value; }\n\
        fn main() -> i64 {\n\
            var holder: Holder = Holder(Item(21));\n\
            return sum(holder.item!, holder.item!);\n\
        }\n";
    let mut program = lower_text(source);
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let mut ends = main
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::EndOptionalView(end) => Some(end),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ends.len(), 2);
    let first_guard = ends[0].guard;
    ends[0].guard = ends[1].guard;
    ends[1].guard = first_guard;

    assert!(verify_mir(&program)
        .unwrap_err()
        .to_string()
        .contains("reverse begin order"));
}

#[test]
fn lowers_class_optional_lifecycle_fields_arguments_results_and_cleanup() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
        class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
        fn forward(item: Item?) -> Item? { return item; }\n\
        fn main() -> i64 {\n\
          var item: Item? = Item(7);\n\
          var holder: Holder = Holder(item);\n\
          item = forward(none);\n\
          if (item is none) { return 42; }\n\
          return 0;\n\
        }\n";
    let program = lower_text(source);
    verify_mir(&program).expect("owning class optionals must verify");
    let dump = dump_mir(&program);
    assert!(dump.contains("class-optional-initialize"));
    assert!(dump.contains("class-optional-assign"));
    assert!(dump.contains("class-optional-cleanup"));
    assert!(dump.contains("OptionalClassField"));
}

#[test]
fn verifier_requires_nested_optional_publication_after_payload_construction() {
    let mut program =
        lower_text("fn main() -> i64 { var value: i64?? = some(some(42)); return 0; }\n");
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    for block in &mut main.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::AggregateOptionalPublish(_))
        });
    }
    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(
        errors.contains("nested optional cleanup destination is not definitely initialized"),
        "{errors}"
    );
}

#[test]
fn verifier_rejects_nested_payload_copy_on_the_absent_presence_edge() {
    let mut program = lower_text(
        "fn take(value: i64??) -> i64? { return value!; }\n\
         fn main() -> i64 { return take(some(some(42)))!; }\n",
    );
    let take = FunctionId::new(0);
    let outer_storage = program
        .definitions
        .get(take)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.name == "value")
        .expect("source parameter must retain its storage name")
        .id;
    let main = program.definitions.get_mut_for_test(take).unwrap();
    let assignment = main
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    &assignment.rvalue.kind,
                    MirRvalueKind::OptionalPresence { source, .. }
                        if source.base.local_storage() == Some(outer_storage)
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("nested unwrap must test outer presence");
    let MirRvalueKind::OptionalPresence { kind, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    *kind = MirPresenceTestKind::None;

    let errors = verify_mir(&program)
        .expect_err("the absent edge must not make nested payload bytes readable")
        .to_string();
    assert!(errors.contains("not definitely initialized"), "{errors}");
}
