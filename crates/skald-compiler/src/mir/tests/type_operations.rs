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

    let error = emit_assembly(Target::X86_64SysV, &program)
        .expect_err("PM19 owns backend support for runtime type operations");
    assert!(error
        .message()
        .contains("runtime type operations are not supported"));
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
