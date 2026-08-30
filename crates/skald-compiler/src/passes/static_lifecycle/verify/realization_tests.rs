//! Test-only optimized shapes for the final realization trust boundary.

use crate::{
    backend::Target,
    identity::{CallableId, FunctionId},
    mir::{
        lower_preliminary_hir, MirAssignment, MirCallTarget, MirInstruction, MirPlace, MirRvalue,
        MirRvalueKind, MirStaticLifecycleIndices, MirStore, MirType, StaticLifecycleAuthority,
        ValueId,
    },
    test_support::{emit_assembly_without_runtime_trace, type_check_source},
};

use super::{
    super::{plan_static_lifetimes, synthesize_static_lifecycle},
    realization, verify_synthesized_mir, LifecycleMirView,
};

fn synthesized(source: &str) -> crate::mir::MirProgram {
    let checked = type_check_source(source);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let planned = plan_static_lifetimes(preliminary).expect("fixture must have an acyclic plan");
    synthesize_static_lifecycle(planned).expect("fixture must synthesize")
}

fn errors(program: &crate::mir::MirProgram) -> String {
    verify_synthesized_mir(program).unwrap_err().to_string()
}

fn function(program: &crate::mir::MirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn realized(program: &crate::mir::MirProgram) -> StaticLifecycleAuthority {
    let coordinator = program.static_lifecycle.as_ref().unwrap();
    realization::analyze(LifecycleMirView {
        program,
        lifecycle: coordinator.lifecycle(),
        initializers: coordinator.initializers(),
    })
    .unwrap()
}

fn baseline_fact_count(program: &crate::mir::MirProgram) -> usize {
    program
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .certificate()
        .authority()
        .roots()
        .map(|root| root.effects().len())
        .sum()
}

fn realized_fact_count(program: &crate::mir::MirProgram) -> usize {
    realized(program)
        .roots()
        .map(|root| root.effects().len())
        .sum()
}

#[test]
fn unoptimized_final_mir_realizes_baseline_authority_exactly() {
    for source in [
        "fn read() -> i64 { return State.base; }
         class State { static base: i64 = 1; static result: i64 = read(); init() {} }
         fn main() -> i64 { return 0; }",
        "class State { static observed: i64; static item: Item?; init() {} }
         class Item { init() {} destroy { var value: i64 = State.observed; } }
         fn main() -> i64 { return 0; }",
        "fn left(flag: bool) -> i64 { if (flag) { return State.base; } return right(true); }
         fn right(flag: bool) -> i64 { if (flag) { return left(true); } return 0; }
         class State { static base: i64 = 1; static result: i64 = left(false); init() {} }
         fn main() -> i64 { return 0; }",
    ] {
        let program = synthesized(source);
        let baseline = program
            .static_lifecycle
            .as_ref()
            .unwrap()
            .lifecycle()
            .certificate()
            .authority();
        assert_eq!(&realized(&program), baseline);
    }
}

#[test]
fn rejects_missing_baseline_for_a_required_final_root() {
    let mut program = synthesized(
        "class State { static value: i64 = 1; init() {} }
         fn main() -> i64 { return 0; }",
    );
    program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .clear();

    assert!(errors(&program).contains("has no baseline authority"));
}

#[test]
fn accepts_removed_unreachable_access_and_dead_effectful_call() {
    let mut unreachable = synthesized(
        "fn maybe_write(flag: bool) -> unit {
           if (flag) { State.observed = 1; }
         }
         fn initialize() -> i64 { maybe_write(false); return 1; }
         class State {
           static observed: i64;
           static result: i64 = initialize();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let maybe_write = function(&unreachable, "maybe_write");
    let definition = unreachable
        .definitions
        .get_mut_for_test(maybe_write)
        .unwrap();
    let before = definition
        .body
        .blocks
        .iter()
        .map(|block| block.instructions.len())
        .sum::<usize>();
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::Store(store)
                if store.destination.base.static_field().is_some())
        });
    }
    assert!(
        definition
            .body
            .blocks
            .iter()
            .map(|block| block.instructions.len())
            .sum::<usize>()
            < before
    );
    assert!(realized_fact_count(&unreachable) < baseline_fact_count(&unreachable));
    verify_synthesized_mir(&unreachable).unwrap();

    let mut dead_call = synthesized(
        "fn write() -> unit { State.observed = 1; }
         fn initialize() -> i64 { write(); return 1; }
         class State {
           static observed: i64;
           static result: i64 = initialize();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let write = function(&dead_call, "write");
    let initialize = function(&dead_call, "initialize");
    let definition = dead_call.definitions.get_mut_for_test(initialize).unwrap();
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(write))
        });
    }
    assert!(realized_fact_count(&dead_call) < baseline_fact_count(&dead_call));
    verify_synthesized_mir(&dead_call).unwrap();
}

#[test]
fn accepts_inlining_shaped_replacement_with_the_same_root_fact() {
    let mut program = synthesized(
        "fn read() -> i64 { return State.base; }
         class State { static base: i64 = 1; static result: i64 = read(); init() {} }
         fn main() -> i64 { return 0; }",
    );
    let base = program
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .find(|field| field.name == "base")
        .unwrap()
        .id;
    let body = &mut program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
        .iter_mut()
        .find(|body| {
            body.body.blocks.iter().any(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, MirInstruction::Call(_)))
            })
        })
        .unwrap()
        .body;
    let instruction = body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let MirInstruction::Call(call) = instruction else {
        unreachable!()
    };
    let result = call.result.expect("scalar read call must return one value");
    let span = call.span;
    *instruction = MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue {
            kind: MirRvalueKind::Load(MirPlace::static_field(base)),
            ty: MirType::I64,
        },
        span,
    });

    assert_eq!(realized_fact_count(&program), baseline_fact_count(&program));
    verify_synthesized_mir(&program).unwrap();
    emit_assembly_without_runtime_trace(Target::X86_64SysV, &program).unwrap();
}

#[test]
fn accepts_virtual_interface_and_indirect_target_narrowing() {
    let dynamic_source = "class State {
           static base: i64 = 1;
           static child: i64 = 2;
           static virtual_result: i64 = read_virtual(Base());
           static interface_result: i64 = read_interface(Base());
           init() {}
         }
         interface View { fn read() -> i64; }
         class Base implements View {
           init() {}
           virtual fn read() -> i64 { return State.base; }
         }
         class Child extends Base {
           init() { super(); }
           override fn read() -> i64 { return State.child; }
         }
         fn read_virtual(ref value: Base) -> i64 { return value.read(); }
         fn read_interface(ref value: View) -> i64 { return value.read(); }
         fn main() -> i64 { return 0; }";

    let mut virtual_program = synthesized(dynamic_source);
    for family in virtual_program.virtual_families.entries_mut_for_test() {
        family.members.truncate(1);
    }
    assert!(realized_fact_count(&virtual_program) < baseline_fact_count(&virtual_program));
    verify_synthesized_mir(&virtual_program).unwrap();

    let mut interface_program = synthesized(dynamic_source);
    let base_method = interface_program
        .classes
        .iter()
        .find(|class| class.name == "Base")
        .unwrap()
        .methods
        .iter()
        .find(|method| method.name == "read")
        .unwrap()
        .id;
    let child = interface_program
        .classes
        .entries_mut_for_test()
        .iter_mut()
        .find(|class| class.name == "Child")
        .unwrap();
    child
        .methods
        .iter_mut()
        .find(|method| method.name == "read")
        .unwrap()
        .name = "optimized-away-override".to_string();
    child.conformances[0].implementations[0].method = base_method;
    assert!(realized_fact_count(&interface_program) < baseline_fact_count(&interface_program));
    verify_synthesized_mir(&interface_program).unwrap();

    let mut indirect = synthesized(
        "fn read_left() -> i64 { return State.left; }
         fn read_right() -> i64 { return State.right; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain_only() -> unit { var callback: fn() -> i64 = read_right; }
         class State {
           static left: i64 = 10;
           static right: i64 = 20;
           static result: i64 = invoke(read_left);
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let left = CallableId::Function(function(&indirect, "read_left"));
    let right = CallableId::Function(function(&indirect, "read_right"));
    let definitions = indirect
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for definition in definitions {
        let definition = indirect.definitions.get_mut_for_test(definition).unwrap();
        for assignment in definition
            .body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .filter_map(|instruction| match instruction {
                MirInstruction::Assign(assignment) => Some(assignment),
                _ => None,
            })
        {
            if let MirRvalueKind::CallableAddress(address) = &mut assignment.rvalue.kind {
                if address.target == right {
                    address.target = left;
                }
            }
        }
    }
    assert!(realized_fact_count(&indirect) < baseline_fact_count(&indirect));
    verify_synthesized_mir(&indirect).unwrap();
}

#[test]
fn rejects_new_target_and_access_kind_facts() {
    let source = "fn read() -> i64 { return State.base; }
         class State {
           static base: i64 = 1;
           static other: i64 = 2;
           static result: i64 = read();
           init() {}
         }
         fn main() -> i64 { return 0; }";
    let mut new_target = synthesized(source);
    let other = new_target
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .find(|field| field.name == "other")
        .unwrap()
        .id;
    let read = function(&new_target, "read");
    let assignment = new_target
        .definitions
        .get_mut_for_test(read)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    assignment.rvalue.kind = MirRvalueKind::Load(MirPlace::static_field(other));
    assert!(errors(&new_target).contains("unauthorized fact"));

    let mut new_access = synthesized(source);
    let base = new_access
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .find(|field| field.name == "base")
        .unwrap()
        .id;
    let read = function(&new_access, "read");
    let definition = new_access.definitions.get_mut_for_test(read).unwrap();
    let (block, index) = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| {
            block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction, MirInstruction::Assign(assignment)
                        if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)))
                })
                .map(|index| (block, index))
        })
        .unwrap();
    let (value, span) = {
        let MirInstruction::Assign(assignment) = &mut block.instructions[index] else {
            unreachable!()
        };
        assignment.rvalue.kind = MirRvalueKind::ConstantI64(0);
        (assignment.result, assignment.span)
    };
    block.instructions.insert(
        index + 1,
        MirInstruction::Store(MirStore {
            destination: MirPlace::static_field(base),
            value,
            authorization: None,
            final_authorization: None,
            span,
        }),
    );
    assert!(errors(&new_access).contains("unauthorized fact"));
}

#[test]
fn rejects_static_access_moved_across_publication() {
    let mut program = synthesized(
        "class State { static base: i64 = 1; static result: i64 = State.base; init() {} }
         fn main() -> i64 { return 0; }",
    );
    let coordinator = program.static_lifecycle.as_mut().unwrap();
    let body = coordinator
        .initializers_mut_for_test()
        .iter_mut()
        .find(|body| {
            body.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction, MirInstruction::Assign(assignment)
                        if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)))
                })
            })
        })
        .unwrap();
    let (block_index, instruction_index) = body
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction, MirInstruction::Assign(assignment)
                        if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)))
                })
                .map(|instruction_index| (block_index, instruction_index))
        })
        .unwrap();
    let moved = match &mut body.body.blocks[block_index].instructions[instruction_index] {
        MirInstruction::Assign(assignment) => {
            let moved = assignment.clone();
            assignment.rvalue.kind = MirRvalueKind::ConstantI64(0);
            moved
        }
        _ => unreachable!(),
    };
    let new_result = ValueId::new(body.callable(), body.values.len());
    let mut metadata = body.values[moved.result.index()].clone();
    metadata.id = new_result;
    body.values.push(metadata);
    let mut moved = moved;
    moved.result = new_result;
    body.body.blocks[body.publication.cleanup_entry.index()]
        .instructions
        .insert(0, MirInstruction::Assign(moved));

    let message = errors(&program);
    assert!(message.contains("unauthorized fact"), "{message}");
    assert!(message.contains("InitializerAfterPublication"), "{message}");
}

#[test]
fn rejects_realized_dependency_that_violates_a_corrupted_frozen_order() {
    let mut program = synthesized(
        "fn read_base() -> i64 { return State.base; }
         class State { static result: i64 = read_base(); static base: i64 = 1; init() {} }
         fn main() -> i64 { return 0; }",
    );
    {
        let coordinator = program.static_lifecycle.as_mut().unwrap();
        coordinator.activation_mut_for_test().swap(0, 1);
        coordinator.shutdown_mut_for_test().swap(0, 1);
        let lifecycle = coordinator.lifecycle_mut_for_test();
        lifecycle
            .plan_mut_for_test()
            .activation_mut_for_test()
            .reverse();
        lifecycle
            .plan_mut_for_test()
            .shutdown_mut_for_test()
            .reverse();
        lifecycle.activation_mut_for_test().rotate_left(2);
        lifecycle.shutdown_mut_for_test().rotate_left(2);

        let activation = lifecycle.plan().activation().to_vec();
        let field_count = activation.len();
        for definition in lifecycle.definitions_mut_for_test() {
            let activation = activation
                .iter()
                .position(|field| *field == definition.field)
                .unwrap();
            definition.indices = MirStaticLifecycleIndices {
                activation,
                shutdown: field_count - activation - 1,
            };
        }
    }
    let indices = program
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .definitions()
        .iter()
        .map(|definition| (definition.field, definition.indices))
        .collect::<std::collections::BTreeMap<_, _>>();
    for field in program
        .classes
        .entries_mut_for_test()
        .iter_mut()
        .flat_map(|class| &mut class.static_fields)
    {
        field.lifecycle = Some(indices[&field.id]);
    }

    let message = errors(&program);
    assert!(
        message.contains("final static-lifecycle dependency")
            && message.contains("violates activation order"),
        "{message}"
    );
}

#[test]
fn rejects_missing_surviving_indirect_target() {
    let mut program = synthesized(
        "fn read() -> i64 { return State.value; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         class State { static value: i64 = 1; static result: i64 = invoke(read); init() {} }
         fn main() -> i64 { return 0; }",
    );
    let foreign = CallableId::Function(FunctionId::new(usize::MAX));
    let assignment = program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
        .iter_mut()
        .flat_map(|body| &mut body.body.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::CallableAddress(_)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::CallableAddress(address) = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    address.target = foreign;
    assert!(errors(&program).contains("callable address target"));
}
