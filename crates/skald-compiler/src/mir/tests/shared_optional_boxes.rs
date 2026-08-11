use super::*;
use crate::{
    backend::Target, identity::OptionalBoxTypeId, passes::run_mir_pipeline,
    test_support::emit_assembly_without_runtime_trace as emit_assembly,
};

fn main_instructions(program: &MirProgram) -> &[MirInstruction] {
    &program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn main_instructions_mut(program: &mut MirProgram) -> &mut Vec<MirInstruction> {
    &mut program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn has_error(program: &MirProgram, needle: &str) -> bool {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains(needle))
}

fn optional_box_allocation(instruction: &MirInstruction) -> Option<&MirSharedAllocate> {
    match instruction {
        MirInstruction::SharedAllocate(allocation)
            if matches!(
                allocation.target,
                MirSharedAllocationTarget::OptionalBox { .. }
            ) =>
        {
            Some(allocation)
        }
        _ => None,
    }
}

fn primitive_box_program() -> MirProgram {
    lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var first: shared i64? = new i64?(41);\n",
        "  var second: shared i64? = first;\n",
        "  second = new i64?();\n",
        "  return 0;\n",
        "}\n",
    ))
}

#[test]
fn rejects_duplicate_exact_optional_box_descriptor_owners() {
    let mut program = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var signed: shared i64? = new i64?(1);\n",
        "  var unsigned: shared u64? = new u64?(2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    let entries = program.optional_box_types.entries_mut_for_test();
    entries[1].exact_optional = entries[0].exact_optional;

    assert!(has_error(&program, "both own exact optional"));
}

#[test]
fn lowers_and_verifies_local_optional_box_owner_lifetimes() {
    let program = primitive_box_program();
    verify_mir(&program).expect("local optional-box owner MIR must verify");
    let protocol_dump = dump_mir(&program)
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("OptionalBox box")
                || [
                    "shared-allocate",
                    "optional-initialize shared-allocation-payload",
                    "shared-publish",
                    "shared-adopt",
                    "shared-copy",
                    "shared-release",
                    "shared-move",
                ]
                .iter()
                .any(|needle| line.contains(needle))
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        protocol_dump,
        concat!(
            "    OptionalBox box0 exact o0 depth 1 view none @39..43\n",
            "          shared-allocate f0:s2 exact optional-box box0 payload=o0 from optional-box complete-with OptionalInitialize @46..49\n",
            "          optional-initialize shared-allocation-payload(f0:s2) from present f0:v0 @55..57\n",
            "          shared-publish f0:s2 @46..58\n",
            "          shared-adopt f0:s0 from f0:s2 @46..58\n",
            "          shared-copy f0:s1 from f0:s0 @88..93\n",
            "          shared-allocate f0:s4 exact optional-box box0 payload=o0 from optional-box complete-with OptionalInitialize @106..109\n",
            "          optional-initialize shared-allocation-payload(f0:s4) from absent @114..115\n",
            "          shared-publish f0:s4 @106..116\n",
            "          shared-adopt f0:s3 from f0:s4 @106..116\n",
            "          shared-release f0:s1 @97..117\n",
            "          shared-move f0:s1 from f0:s3 @97..117\n",
            "          shared-release f0:s1 @120..129\n",
            "          shared-release f0:s0 @120..129",
        )
    );
    run_mir_pipeline(program).expect("optional-box MIR must survive target-independent passes");
}

#[test]
fn lowers_every_selected_optional_wrapper_initialization_family() {
    let program = lower_text(concat!(
        "class Value { init() {} }\n",
        "fn maybe_number() -> i64? { return some(9); }\n",
        "fn maybe_nested() -> i64?? { return some(some(10)); }\n",
        "fn main() -> i64 {\n",
        "  var primitive: shared i64? = new i64?(41);\n",
        "  var absent_class: shared Value? = new Value?();\n",
        "  var direct_class: shared Value? = new Value?(Value());\n",
        "  var copied_class: shared Value? = new Value?(*direct_class);\n",
        "  var array_box: shared i64[]? = new i64[]?(i64[]{1, 2});\n",
        "  var owner_box: shared (shared Value)? = new (shared Value)?(new Value());\n",
        "  var nested_box: shared i64?? = new i64??(some(some(7)));\n",
        "  var produced: shared i64? = new i64?(maybe_number());\n",
        "  var produced_nested: shared i64?? = new i64??(maybe_nested());\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("every selected optional-box wrapper plan must verify");

    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&program));
    for completion in [
        "OptionalInitialize",
        "ClassInitialize",
        "ClassPublish",
        "OptionalSharedInitialize",
        "AggregatePublish",
        "DestinationCall",
    ] {
        assert!(dump.contains(completion), "missing {completion}:\n{dump}");
    }
}

#[test]
fn preserves_exact_and_polymorphic_optional_box_targets() {
    let program = lower_text(concat!(
        "interface Marker { fn mark() -> i64; }\n",
        "class Base { init() {} virtual fn mark() -> i64 { return 1; } }\n",
        "class Derived extends Base implements Marker {\n",
        "  init() { super(); }\n",
        "  override fn mark() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var exact: shared Derived? = new Derived?(Derived());\n",
        "  var base: shared Base? = exact;\n",
        "  var marker: shared Marker? = exact;\n",
        "  var object: shared Obj? = exact;\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("polymorphic optional-box owner views must verify");

    let targets = program.optional_box_types.iter().collect::<Vec<_>>();
    assert!(targets.iter().any(|target| target.exact_optional.is_some()
        && matches!(target.object_view, Some(MirViewTarget::Class(_)))));
    assert!(targets.iter().any(|target| target.exact_optional.is_none()
        && matches!(target.object_view, Some(MirViewTarget::Interface(_)))));
    assert!(targets
        .iter()
        .any(|target| target.exact_optional.is_none()
            && target.object_view == Some(MirViewTarget::Obj)));

    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&program));
    assert!(dump.contains("OptionalBoxTypes"), "{dump}");
    assert!(dump.contains("exact view-only"), "{dump}");
    assert!(dump.contains("view interface"), "{dump}");
    assert!(dump.contains("view Obj"), "{dump}");
}

#[test]
fn evaluates_sources_before_allocating_and_publishes_once() {
    let program = lower_text(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var primitive: shared i64? = new i64?(41);\n",
        "  var owner: shared (shared Value)? = new (shared Value)?(new Value());\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("source-ordered optional-box MIR must verify");
    let instructions = main_instructions(&program);

    let primitive_source = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::ConstantI64(41),
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap();
    let first_box = instructions
        .iter()
        .position(|instruction| optional_box_allocation(instruction).is_some())
        .unwrap();
    assert!(primitive_source < first_box);
    assert!(matches!(
        &instructions[first_box..first_box + 4],
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::OptionalInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
        ]
    ));

    let owner_box = instructions
        .iter()
        .position(|instruction| {
            optional_box_allocation(instruction).is_some_and(|allocation| {
                matches!(
                    allocation.mode,
                    MirSharedAllocationMode::OptionalBox {
                        completion: MirOptionalBoxCompletion::OptionalSharedInitialize
                    }
                )
            })
        })
        .unwrap();
    let nested_owner_source = instructions[..owner_box]
        .iter()
        .rposition(|instruction| {
            matches!(
                instruction,
                MirInstruction::SharedAllocate(MirSharedAllocate {
                    target: MirSharedAllocationTarget::Class(_),
                    ..
                })
            )
        })
        .expect("nested shared owner must be complete before box allocation");
    assert!(nested_owner_source < owner_box);
}

#[test]
fn optional_box_owner_copy_and_replacement_use_explicit_secure_ordering() {
    let program = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var source: shared i64? = new i64?(1);\n",
        "  var destination: shared i64? = source;\n",
        "  destination = destination;\n",
        "  destination = new i64?(2);\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("optional-box owner operations must verify");
    let instructions = main_instructions(&program);
    assert!(instructions.windows(4).any(|window| matches!(
        window,
        [
            MirInstruction::SharedCopy(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
            MirInstruction::EndFullExpression(_),
        ]
    )));
    assert!(instructions.windows(6).any(|window| matches!(
        window,
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::OptionalInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
        ]
    )));
}

#[test]
fn rejects_wrong_optional_box_target_origin_place_and_completion() {
    let mut wrong_target = primitive_box_program();
    let allocation = main_instructions_mut(&mut wrong_target)
        .iter_mut()
        .find_map(optional_box_allocation_mut)
        .unwrap();
    let MirSharedAllocationTarget::OptionalBox { target, .. } = &mut allocation.target else {
        unreachable!()
    };
    *target = OptionalBoxTypeId::new(99);
    assert!(has_error(
        &wrong_target,
        "optional-box allocation target does not name matching exact optional metadata"
    ));

    let mut wrong_origin = primitive_box_program();
    main_instructions_mut(&mut wrong_origin)
        .iter_mut()
        .find_map(optional_box_allocation_mut)
        .unwrap()
        .origin = MirSharedAllocationOrigin::New;
    assert!(has_error(
        &wrong_origin,
        "optional-box allocation does not have optional-box origin"
    ));

    let mut wrong_place = primitive_box_program();
    let instructions = main_instructions_mut(&mut wrong_place);
    let allocation = instructions
        .iter()
        .find_map(optional_box_allocation)
        .unwrap()
        .allocation;
    let initialize = instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::OptionalInitialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.destination = MirPlace::base(allocation);
    assert!(has_error(
        &wrong_place,
        "unpublished shared allocation storage"
    ));

    let mut wrong_completion = primitive_box_program();
    main_instructions_mut(&mut wrong_completion)
        .iter_mut()
        .find_map(optional_box_allocation_mut)
        .unwrap()
        .mode = MirSharedAllocationMode::OptionalBox {
        completion: MirOptionalBoxCompletion::ClassInitialize,
    };
    assert!(has_error(
        &wrong_completion,
        "shared publication requires completed initialization"
    ));
}

fn optional_box_allocation_mut(instruction: &mut MirInstruction) -> Option<&mut MirSharedAllocate> {
    match instruction {
        MirInstruction::SharedAllocate(allocation)
            if matches!(
                allocation.target,
                MirSharedAllocationTarget::OptionalBox { .. }
            ) =>
        {
            Some(allocation)
        }
        _ => None,
    }
}

#[test]
fn rejects_optional_box_protocol_reordering_and_missing_steps() {
    let mut initialize_before_allocate = primitive_box_program();
    let instructions = main_instructions_mut(&mut initialize_before_allocate);
    let allocation = instructions
        .iter()
        .position(|instruction| optional_box_allocation(instruction).is_some())
        .unwrap();
    let initialize = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::OptionalInitialize(_)))
        .unwrap();
    instructions.swap(allocation, initialize);
    assert!(has_error(
        &initialize_before_allocate,
        "shared publication requires completed initialization"
    ));

    let mut publish_before_completion = primitive_box_program();
    let instructions = main_instructions_mut(&mut publish_before_completion);
    let initialize = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::OptionalInitialize(_)))
        .unwrap();
    let publish = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedPublish(_)))
        .unwrap();
    instructions.swap(initialize, publish);
    assert!(has_error(
        &publish_before_completion,
        "shared publication requires completed initialization"
    ));

    let mut missing_publish = primitive_box_program();
    let publish = first_instruction(&missing_publish, |instruction| {
        matches!(instruction, MirInstruction::SharedPublish(_))
    });
    main_instructions_mut(&mut missing_publish).remove(publish);
    assert!(has_error(
        &missing_publish,
        "shared adoption requires one published produced owner"
    ));

    let mut missing_adopt = primitive_box_program();
    let adopt = first_instruction(&missing_adopt, |instruction| {
        matches!(instruction, MirInstruction::SharedAdopt(_))
    });
    main_instructions_mut(&mut missing_adopt).remove(adopt);
    assert!(has_error(
        &missing_adopt,
        "shared allocation is not published and adopted"
    ));
}

fn first_instruction(program: &MirProgram, predicate: impl Fn(&MirInstruction) -> bool) -> usize {
    main_instructions(program)
        .iter()
        .position(predicate)
        .unwrap()
}

#[test]
fn rejects_duplicate_publication_adoption_and_release_errors() {
    let mut duplicate_publish = primitive_box_program();
    let publish_index = first_instruction(&duplicate_publish, |instruction| {
        matches!(instruction, MirInstruction::SharedPublish(_))
    });
    let publish = main_instructions(&duplicate_publish)[publish_index].clone();
    main_instructions_mut(&mut duplicate_publish).insert(publish_index + 1, publish);
    assert!(has_error(
        &duplicate_publish,
        "shared publication requires completed initialization"
    ));

    let mut duplicate_adopt = primitive_box_program();
    let adopt_index = first_instruction(&duplicate_adopt, |instruction| {
        matches!(instruction, MirInstruction::SharedAdopt(_))
    });
    let adopt = main_instructions(&duplicate_adopt)[adopt_index].clone();
    main_instructions_mut(&mut duplicate_adopt).insert(adopt_index + 1, adopt);
    assert!(has_error(
        &duplicate_adopt,
        "shared adoption requires one published produced owner"
    ));

    let mut missing_release = primitive_box_program();
    main_instructions_mut(&mut missing_release)
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedRelease(_)));
    assert!(has_error(
        &missing_release,
        "shared owner remains live on normal return"
    ));

    let mut duplicate_release = primitive_box_program();
    let release_index = first_instruction(&duplicate_release, |instruction| {
        matches!(instruction, MirInstruction::SharedRelease(_))
    });
    let release = main_instructions(&duplicate_release)[release_index].clone();
    main_instructions_mut(&mut duplicate_release).insert(release_index + 1, release);
    assert!(has_error(&duplicate_release, "released without being live"));
}

#[test]
fn rejects_prepublication_observation_and_owner_cfg_disagreement() {
    let mut observation = primitive_box_program();
    let instructions = main_instructions_mut(&mut observation);
    let payload = instructions
        .iter()
        .find_map(optional_box_allocation)
        .map(|allocation| MirPlace::shared_allocation_payload(allocation.allocation))
        .unwrap();
    let initialize = instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::OptionalInitialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.source = MirOptionalSource::Copy(payload);
    assert!(has_error(
        &observation,
        "only valid as an unpublished initialization destination"
    ));

    let mut join = primitive_box_program();
    let function = join
        .definitions
        .get_mut_for_test(join.entry_function)
        .unwrap();
    let span = function.span;
    let original = function.body.blocks.pop().unwrap();
    let split = original
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let mut before_release = original.instructions;
    let after_release = before_release.split_off(split);
    let condition = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(condition, MirType::Bool, span));
    let entry = BlockId::new(function.function, 0);
    let released = BlockId::new(function.function, 1);
    let live = BlockId::new(function.function, 2);
    let exit = BlockId::new(function.function, 3);
    before_release.push(fixture_assign(
        condition,
        MirRvalueKind::ConstantBool(true),
        MirType::Bool,
        span,
    ));
    function.body.blocks = vec![
        fixture_block(
            entry,
            before_release,
            Some(MirTerminator::Branch {
                condition,
                true_target: released,
                false_target: live,
                span,
            }),
            span,
        ),
        fixture_block(
            released,
            after_release,
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(
            live,
            vec![],
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(exit, vec![], original.terminator, span),
    ];
    assert!(has_error(&join, "state differs across control-flow paths"));
}

#[test]
fn x86_backend_executes_verified_primitive_optional_box_mir() {
    let program = primitive_box_program();
    verify_mir(&program).expect("primitive backend fixture must be valid MIR");
    let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(assembly.matches("call ska_rt_alloc").count(), 2);
    assert_eq!(assembly.matches("call ska_rt_free").count(), 3);
    assert!(assembly.contains(".Lska_optional_box_0_metadata"));
    assert!(assembly.contains(".Lska_optional_box_0_finalize"));
}
