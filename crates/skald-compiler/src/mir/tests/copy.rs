use super::*;
use crate::identity::CopyAssignmentId;

const COPY_SOURCE: &str = concat!(
    "class Value {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  copy(ref other: Value) { self.value = other.value; }\n",
    "  assign(ref other: Value) { self.value = other.value; }\n",
    "  destroy {}\n",
    "}\n",
    "class Pair {\n",
    "  left: Value; right: Value;\n",
    "  init(value: i64) { self.left = Value(value); self.right = Value(value); }\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var first: Value = Value(1);\n",
    "  var second: Value = first;\n",
    "  second = second;\n",
    "  var pair: Pair = Pair(2);\n",
    "  pair.left = second;\n",
    "  return 0;\n",
    "}\n",
);

#[test]
fn lowers_selected_copy_operations_and_lifecycle_definitions_structurally() {
    let mir = lower_text(COPY_SOURCE);
    assert!(verify_mir(&mir).is_ok());

    let value = mir.class(ClassId::new(0)).unwrap();
    assert_eq!(
        value.copy_constructor,
        MirCopyCapability::User(MirUserCopy {
            operation: CopyConstructorId::new(value.id, 0),
            base: None,
        })
    );
    assert_eq!(
        value.copy_assignment,
        MirCopyCapability::User(MirUserCopy {
            operation: CopyAssignmentId::new(value.id, 0),
            base: None,
        })
    );
    assert!(mir
        .member_definition(CopyConstructorId::new(value.id, 0).into())
        .is_some());
    assert!(mir
        .member_definition(CopyAssignmentId::new(value.id, 0).into())
        .is_some());

    let pair = mir.class(ClassId::new(1)).unwrap();
    let MirCopyCapability::Synthesized(construction) = &pair.copy_constructor else {
        panic!("expected synthesized pair copy construction");
    };
    assert_eq!(construction.fields.len(), 2);
    assert!(construction.fields.iter().all(|step| matches!(
        step,
        MirSynthesizedFieldCopy::Class {
            operation: MirSelectedCopyOperation::User(_),
            ..
        }
    )));

    let main = mir.definitions.get(mir.entry_function).unwrap();
    let copies: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction,
                MirInstruction::CopyConstruct(_) | MirInstruction::CopyAssign(_)
            )
        })
        .collect();
    assert_eq!(copies.len(), 3);
    let MirInstruction::CopyConstruct(construction) = copies[0] else {
        panic!("expected local copy construction first");
    };
    assert_eq!(construction.class, value.id);
    assert_eq!(
        construction.operation,
        MirSelectedCopyOperation::User(CopyConstructorId::new(value.id, 0))
    );
    let MirInstruction::CopyAssign(self_assignment) = copies[1] else {
        panic!("expected self-assignment second");
    };
    assert_eq!(self_assignment.destination, self_assignment.source);
    assert!(main.values.iter().all(|value| value.ty.is_scalar_value()));

    let dump = dump_mir(&mir);
    assert!(dump.contains("CopyConstructor\n        User c0:copy0"));
    assert!(dump.contains("copy-construct f0:s1 from f0:s0 as c0 via user c0:copy0"));
    assert!(dump.contains("copy-assign f0:s1 from f0:s1 as c0 via user c0:assign0"));
}

#[test]
fn verifier_checks_copy_constructor_identity_and_definition_independently() {
    let mut wrong_identity = lower_text(COPY_SOURCE);
    let class = ClassId::new(0);
    wrong_identity.classes.entries_mut_for_test()[0]
        .copy_constructor_declaration
        .as_mut()
        .unwrap()
        .id = CopyConstructorId::new(class, 1);
    let errors = verify_mir(&wrong_identity).unwrap_err().to_string();
    assert!(errors.contains("copy-constructor declaration contains c0:copy1"));

    let mut missing_definition = lower_text(COPY_SOURCE);
    let copy = CopyConstructorId::new(class, 0);
    missing_definition
        .member_definitions
        .remove_for_test(copy.into());
    let errors = verify_mir(&missing_definition).unwrap_err().to_string();
    assert!(errors.contains("copy constructor c0:copy0 has no member definition"));
}

#[test]
fn verifier_rejects_wrong_copy_selection_overlap_and_liveness() {
    let mut wrong_operation = lower_text(COPY_SOURCE);
    let function = wrong_operation
        .definitions
        .get_mut_for_test(wrong_operation.entry_function)
        .unwrap();
    let copy = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::CopyConstruct(copy) => Some(copy),
            _ => None,
        })
        .unwrap();
    copy.operation = MirSelectedCopyOperation::Synthesized(copy.class);
    let errors = verify_mir(&wrong_operation).unwrap_err().to_string();
    assert!(errors.contains("does not match the class capability"));

    let mut overlap = lower_text(COPY_SOURCE);
    let function = overlap
        .definitions
        .get_mut_for_test(overlap.entry_function)
        .unwrap();
    let copy = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::CopyConstruct(copy) => Some(copy),
            _ => None,
        })
        .unwrap();
    copy.source = copy.destination.clone();
    let errors = verify_mir(&overlap).unwrap_err().to_string();
    assert!(errors.contains("must not overlap"));
    assert!(errors.contains("copy-construction source is not live"));

    let mut assign_uninitialized = lower_text(COPY_SOURCE);
    let function = assign_uninitialized
        .definitions
        .get_mut_for_test(assign_uninitialized.entry_function)
        .unwrap();
    let position = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .unwrap();
    let MirInstruction::CopyConstruct(copy) =
        function.body.blocks[0].instructions[position].clone()
    else {
        unreachable!()
    };
    function.body.blocks[0].instructions[position] =
        MirInstruction::CopyAssign(MirCopyAssignment {
            destination: copy.destination,
            source: copy.source,
            class: copy.class,
            operation: MirSelectedCopyOperation::User(CopyAssignmentId::new(copy.class, 0)),
            span: copy.span,
        });
    let errors = verify_mir(&assign_uninitialized).unwrap_err().to_string();
    assert!(errors.contains("copy-assignment destination is not live"));
}

#[test]
fn verifies_temporary_storage_and_reverse_full_expression_cleanup() {
    let mut mir = lower_text(concat!(
        "class Value { init() {} destroy {} }\n",
        "fn main() -> i64 { var source: Value = Value(); return 0; }\n",
    ));
    let class = ClassId::new(0);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let temporary = StorageId::new(function.function, function.storage.len());
    function.storage.push(MirStorage {
        id: temporary,
        source: None,
        name: "temporary0".to_owned(),
        kind: MirStorageKind::Temporary,
        ty: MirType::Class(class),
        span,
    });
    let source = MirPlace::base(function.storage[0].id);
    let temporary_place = MirPlace::base(temporary);
    function.body.blocks[0].instructions.insert(
        1,
        MirInstruction::CopyConstruct(MirCopyConstruction {
            destination: temporary_place.clone(),
            source,
            class,
            operation: MirSelectedCopyOperation::Synthesized(class),
            span,
        }),
    );
    function.body.blocks[0].instructions.insert(
        2,
        MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: vec![MirCleanup {
                destination: temporary_place,
                target: class,
                span,
            }],
            span,
        }),
    );
    assert!(verify_mir(&mir).is_ok());
    assert!(dump_mir(&mir).contains("temporary <temporary> \"temporary0\" : class c0"));

    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let MirInstruction::EndFullExpression(end) = &mut function.body.blocks[0].instructions[2]
    else {
        unreachable!()
    };
    end.temporaries.clear();
    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("reverse completion order"));
    assert!(errors.contains("owning temporary remains live"));
}

#[test]
fn verifier_rejects_complete_receiver_replacement() {
    let mut mir = lower_text(COPY_SOURCE);
    let class = ClassId::new(0);
    let assignment = CopyAssignmentId::new(class, 0);
    let definition = mir
        .member_definitions
        .get_mut_for_test(assignment.into())
        .unwrap();
    definition.body.blocks[0].instructions.insert(
        0,
        MirInstruction::CopyAssign(MirCopyAssignment {
            destination: MirPlace::base(definition.receiver),
            source: MirPlace::alias_parameter(definition.parameters[0]),
            class,
            operation: MirSelectedCopyOperation::User(assignment),
            span: definition.span,
        }),
    );

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("cannot replace the complete receiver"));
}

#[test]
fn verifier_rejects_path_dependent_temporary_ownership() {
    let mut mir = lower_text(concat!(
        "class Value { init() {} destroy {} }\n",
        "fn main() -> i64 { var source: Value = Value(); if (true) {} return 0; }\n",
    ));
    let class = ClassId::new(0);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let temporary = StorageId::new(function.function, function.storage.len());
    function.storage.push(MirStorage {
        id: temporary,
        source: None,
        name: "temporary0".to_owned(),
        kind: MirStorageKind::Temporary,
        ty: MirType::Class(class),
        span,
    });
    let Some(MirTerminator::Branch { true_target, .. }) =
        function.body.blocks[0].terminator.as_ref()
    else {
        panic!("expected source conditional to lower to a branch");
    };
    let true_target = *true_target;
    function.body.blocks[true_target.index()]
        .instructions
        .push(MirInstruction::CopyConstruct(MirCopyConstruction {
            destination: MirPlace::base(temporary),
            source: MirPlace::base(function.storage[0].id),
            class,
            operation: MirSelectedCopyOperation::Synthesized(class),
            span,
        }));

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("temporary liveness differs across control-flow paths"));
}
