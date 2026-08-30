use crate::{
    identity::CallableId,
    mir::{dump_mir, BlockId, MirBasicBlock, MirTerminator},
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::mir::rewrite::{
    edit::BlockPlacement,
    error::{MirReferenceFailure, MirRewriteError},
    MirLocalIdentity, MirLocalIdentitySite,
};

const COMPLETE_EXECUTABLE_SURFACE: &str = concat!(
    "extern fn foreign(value: i64) -> i64;\n",
    "fn seed() -> i64 { return 4; }\n",
    "class Item {\n",
    "  value: i64;\n",
    "  static first: i64 = seed();\n",
    "  static second: i64 = seed() + 1;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  copy(ref other: Item) { self.value = other.value; }\n",
    "  assign(ref other: Item) { self.value = other.value; }\n",
    "  destroy { self.value = 0; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  static fn twice(value: i64) -> i64 { return value + value; }\n",
    "}\n",
    "fn main() -> i64 { var item: Item = Item(3); return item.read(); }\n",
);

#[test]
fn no_op_program_rewrite_preserves_every_definition_and_exact_dump() {
    let original = lower_source_to_final_mir(COMPLETE_EXECUTABLE_SURFACE);
    let expected_dump = dump_mir(&original);
    let expected_callables = original
        .executable_definitions()
        .map(|definition| definition.callable())
        .collect::<Vec<_>>();
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::Function(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::Initializer(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::CopyConstructor(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::CopyAssignment(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::Destructor(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::Method(_))));
    assert!(expected_callables
        .iter()
        .any(|callable| matches!(callable, CallableId::StaticInitializer(_))));

    let result = rewrite_program(original.clone(), |_callable, _edit| Ok(())).unwrap();

    assert_eq!(result.program, original);
    assert_eq!(dump_mir(&result.program), expected_dump);
    assert_eq!(
        result
            .callables
            .iter()
            .map(|result| result.callable)
            .collect::<Vec<_>>(),
        expected_callables
    );
}

#[test]
fn static_initializer_rewrite_preserves_lifecycle_and_semantic_identity_order() {
    let original = lower_source_to_final_mir(COMPLETE_EXECUTABLE_SURFACE);
    let original_coordinator = original.static_lifecycle.as_ref().unwrap();
    let expected_lifecycle = original_coordinator.lifecycle().clone();
    let expected_activation = original_coordinator.activation().to_vec();
    let expected_shutdown = original_coordinator.shutdown().to_vec();
    let expected_initializer_ids = original_coordinator
        .initializers()
        .iter()
        .map(|initializer| initializer.id)
        .collect::<Vec<_>>();
    let expected_function_slots = original
        .definitions
        .indexed_slots()
        .map(|(index, definition)| (index, definition.map(|definition| definition.function)))
        .collect::<Vec<_>>();
    let expected_members = original
        .member_definitions
        .iter()
        .map(|definition| definition.callable)
        .collect::<Vec<_>>();

    let result = rewrite_program(original, |callable, edit| {
        if matches!(callable, CallableId::StaticInitializer(_)) {
            let anchor = edit.block_order()[1];
            let span = edit.block(anchor)?.span;
            edit.allocate_block(BlockPlacement::Before(anchor), |identity| {
                empty_block(identity, span)
            })?;
        }
        Ok(())
    })
    .unwrap();
    let coordinator = result.program.static_lifecycle.as_ref().unwrap();

    assert_eq!(coordinator.lifecycle(), &expected_lifecycle);
    assert_eq!(coordinator.activation(), expected_activation);
    assert_eq!(coordinator.shutdown(), expected_shutdown);
    assert_eq!(
        coordinator
            .initializers()
            .iter()
            .map(|initializer| initializer.id)
            .collect::<Vec<_>>(),
        expected_initializer_ids
    );
    assert_eq!(
        result
            .program
            .definitions
            .indexed_slots()
            .map(|(index, definition)| (index, definition.map(|definition| definition.function)))
            .collect::<Vec<_>>(),
        expected_function_slots
    );
    assert_eq!(
        result
            .program
            .member_definitions
            .iter()
            .map(|definition| definition.callable)
            .collect::<Vec<_>>(),
        expected_members
    );
}

#[test]
fn a_late_callable_failure_returns_no_partially_rewritten_program() {
    let original = lower_source_to_final_mir(COMPLETE_EXECUTABLE_SURFACE);
    let mut completed_functions = 0;
    let error = rewrite_program(original.clone(), |callable, edit| {
        if matches!(callable, CallableId::Function(_)) {
            completed_functions += 1;
            return Ok(());
        }
        let entry = edit.entry();
        edit.remove_block(entry)?;
        Ok(())
    })
    .unwrap_err();

    assert!(completed_functions > 0);
    assert!(matches!(
        error,
        MirRewriteError::InvalidReference {
            identity: MirLocalIdentity::Block(_),
            site: MirLocalIdentitySite::BodyEntry,
            failure: MirReferenceFailure::Deleted,
            ..
        }
    ));
    assert_eq!(
        dump_mir(&original),
        dump_mir(&lower_source_to_final_mir(COMPLETE_EXECUTABLE_SURFACE))
    );
}

fn empty_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}
