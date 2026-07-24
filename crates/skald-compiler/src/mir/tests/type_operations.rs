use super::*;
use crate::backend::{emit_assembly, Target};

use super::type_operation_fixtures::type_operation_mir;

#[test]
fn lowers_runtime_type_operations_to_explicit_metadata_and_control_flow() {
    let program = type_operation_mir();
    verify_mir(&program).expect("lowered type operations must verify");

    let dump = dump_mir(&program);
    assert!(dump.contains("type-test"));
    assert!(dump.contains("checked-narrow"));
    assert!(dump.contains("terminate narrowing-failure"));
    assert!(dump.contains("end-narrowed"));
    assert!(dump.contains("narrowed("));

    let assembly = emit_assembly(Target::X86_64SysV, &program)
        .expect("verified type operations must reach backend lowering");
    assert!(assembly.contains("cmp "));
    assert!(assembly.contains("ud2"));
}

#[test]
fn folds_static_type_tests_and_narrowing_without_metadata_queries() {
    let program = lower_text(
        "class Base { init() {} }\n\
         class Derived extends Base { init() { super(); } }\n\
         fn inspect(ref value: Derived) -> bool {\n\
           var answer: bool = value is Base;\n\
           narrow ref base: Base = value {}\n\
           return answer;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    verify_mir(&program).expect("static type operations must verify");
    let dump = dump_mir(&program);
    assert!(dump.contains("const.bool true"));
    assert!(dump.contains("bind-narrowed"));
    assert!(!dump.contains("type-test"));
    assert!(!dump.contains("checked-narrow"));
}

#[test]
fn rejects_malformed_failure_edges_and_scoped_alias_use() {
    let mut malformed_edge = type_operation_mir();
    let inspect = malformed_edge
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .expect("inspect definition");
    let failure = inspect
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::CheckedNarrow { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .expect("runtime narrowing terminator");
    inspect.body.blocks[failure.index()].terminator = Some(MirTerminator::Return {
        value: None,
        span: inspect.span,
    });
    let errors = verify_mir(&malformed_edge).expect_err("failure edge mutation must be rejected");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("failure edge must terminate")));

    let mut escaped_alias = type_operation_mir();
    let inspect = escaped_alias
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .expect("inspect definition");
    let success = inspect
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::EndNarrowedAlias(_)))
        })
        .expect("narrowing success block");
    let end = success
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndNarrowedAlias(_)))
        .expect("scope end");
    success.instructions.swap(end - 1, end);
    let errors = verify_mir(&escaped_alias).expect_err("post-scope alias use must be rejected");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("object view source is not live")));
}

#[test]
fn rejects_invalid_type_targets_results_and_narrowed_views() {
    fn assert_error(program: &MirProgram, expected: &str) {
        let errors = verify_mir(program).expect_err("MIR mutation must be rejected");
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "expected `{expected}` in:\n{errors}"
        );
    }

    let mut target = type_operation_mir();
    let inspect = target
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let assignment = inspect
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::TypeTest { .. }) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::TypeTest {
        target: test_target,
        ..
    } = &mut assignment.rvalue.kind
    else {
        unreachable!()
    };
    *test_target = MirViewTarget::Class(ClassId::new(99));
    assert_error(&target, "type-test target is not declared");

    let mut result = type_operation_mir();
    let inspect = result
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let (result_id, assignment) = inspect
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::TypeTest { .. }) =>
            {
                Some((assignment.result, assignment))
            }
            _ => None,
        })
        .unwrap();
    assignment.rvalue.ty = MirType::I64;
    inspect.values[result_id.index()].ty = MirType::I64;
    assert_error(&result, "runtime type-test result is not `bool`");

    let mut view = type_operation_mir();
    let inspect = view
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let binding = inspect
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Some(MirTerminator::CheckedNarrow { binding, .. }) => Some(binding),
            _ => None,
        })
        .unwrap();
    binding.view.access = MirAliasAccess::Mutable;
    assert_error(
        &view,
        "narrowed alias storage does not match its selected view",
    );

    let mut provenance = type_operation_mir();
    let inspect = provenance
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let binding = inspect
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Some(MirTerminator::CheckedNarrow { binding, .. }) => Some(binding),
            _ => None,
        })
        .unwrap();
    let MirObjectOrigin::Forwarded { carrier, .. } = binding.view.origin.as_mut() else {
        unreachable!("Obj parameter must retain forwarded provenance")
    };
    *carrier = StorageId::new(FunctionId::new(1), 99);
    assert_error(&provenance, "origin carrier");
}

#[test]
fn rejects_corrupt_checked_cast_carriers_and_failure_edges() {
    fn cast_program() -> MirProgram {
        lower_text(
            "class Leaf { init() {} fn read() -> i64 { return 7; } }\n\
             class Other { init() {} }\n\
             fn inspect(ref value: Obj) -> i64 { return ((Leaf) value).read(); }\n\
             fn main() -> i64 { return 0; }\n",
        )
    }

    let mut malformed_edge = cast_program();
    let inspect = malformed_edge
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let (failure, success) = inspect
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::CheckedCast {
                failure_target,
                success_target,
                ..
            }) => Some((failure_target, success_target)),
            _ => None,
        })
        .unwrap();
    inspect.body.blocks[failure.index()].terminator = Some(MirTerminator::Goto {
        target: success,
        span: inspect.span,
    });
    let errors =
        verify_mir(&malformed_edge).expect_err("cast failure edge mutation must be rejected");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("object-cast failure")));

    let mut wrong_storage = cast_program();
    let inspect = wrong_storage
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let carrier = inspect
        .body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Some(MirTerminator::CheckedCast { binding, .. }) => Some(binding.destination),
            _ => None,
        })
        .unwrap();
    inspect.storage[carrier.index()].kind = MirStorageKind::Temporary;
    let errors =
        verify_mir(&wrong_storage).expect_err("cast carrier kind mutation must be rejected");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("checked-view binding destination")));

    let mut ended_before_copy = lower_text(
        "class Leaf { init() {} }\n\
         class Other { init() {} }\n\
         fn copied(ref value: Obj) -> Leaf { return (Leaf) value; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let copied = ended_before_copy
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let success = copied
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        })
        .expect("cast-copy success block");
    let copy = success
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .unwrap();
    let end = success
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
        .unwrap();
    success.instructions.swap(copy, end);
    let errors =
        verify_mir(&ended_before_copy).expect_err("ended checked source must fail verification");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("not live")),
        "{errors}"
    );
}
