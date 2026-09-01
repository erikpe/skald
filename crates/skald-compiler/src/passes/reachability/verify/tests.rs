//! Sparse final-MIR completeness tests at the central verification boundary.

use crate::{
    identity::{CallableId, ClassId, StaticFieldId},
    mir::{verify_mir, verify_preliminary_mir, MirProgram},
    passes::{
        static_lifecycle::{
            plan_static_lifetimes, synthesize_static_lifecycle, verify_planned_mir,
        },
        verify_final_mir, VerifiedFinalMirProgram,
    },
    test_support::{lower_generic_source_to_final_mir, lower_generic_source_to_preliminary_mir},
};

use super::super::{analyze_reachability, MirDependencyEdgeKind, MirReachabilityRootReason};

fn function(program: &MirProgram, name: &str) -> CallableId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("missing function `{name}`"))
        .id
        .into()
}

fn class(program: &MirProgram, name: &str) -> ClassId {
    program
        .classes
        .iter()
        .find(|class| class.name == name)
        .unwrap_or_else(|| panic!("missing class `{name}`"))
        .id
}

fn method(program: &MirProgram, class_name: &str, name: &str) -> CallableId {
    let class = program.class(class(program, class_name)).unwrap();
    class
        .methods
        .iter()
        .find(|method| method.name == name)
        .unwrap_or_else(|| panic!("missing method `{class_name}.{name}`"))
        .id
        .into()
}

fn static_field(program: &MirProgram, class_name: &str, name: &str) -> StaticFieldId {
    program
        .classes
        .iter()
        .find(|class| class.name == class_name)
        .unwrap_or_else(|| panic!("missing class `{class_name}`"))
        .static_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("missing static field `{class_name}.{name}`"))
        .id
}

fn sparse_static_program(source: &str) -> MirProgram {
    let preliminary = lower_generic_source_to_preliminary_mir(source);
    let planned = plan_static_lifetimes(preliminary)
        .expect("test program must have an acyclic sparse lifecycle plan");
    synthesize_static_lifecycle(verify_planned_mir(planned).unwrap())
}

const SPARSE_ACCESS_SOURCE: &str = "class State {
       static live: i64 = 1;
       static inactive: i64 = 2;
       init() {}
     }
     fn dead() -> i64 { return State.inactive; }
     fn main() -> i64 { return State.live; }";

fn missing_definition_error(
    mut program: MirProgram,
    callable: CallableId,
) -> crate::mir::MirVerificationError {
    program.remove_executable_definition_for_test(callable);
    let first = verify_final_mir(program.clone()).unwrap_err();
    let second = verify_final_mir(program).unwrap_err();
    assert_eq!(
        first, second,
        "missing-definition errors must be deterministic"
    );
    let error = first
        .iter()
        .find(|error| error.callable == Some(callable))
        .unwrap_or_else(|| panic!("no missing-definition error for {callable}: {first}"))
        .clone();
    error
}

fn assert_missing_category(program: MirProgram, callable: CallableId, category: &str) {
    let error = missing_definition_error(program, callable);
    assert_eq!(error.block, None);
    assert_eq!(
        error.message,
        format!(
            "reachable callable has no retained definition; selected by dependency category `{category}`"
        )
    );
}

fn assert_reachable_graph_contains_kind(
    mut program: MirProgram,
    callable: CallableId,
    kind: MirDependencyEdgeKind,
) {
    program.remove_executable_definition_for_test(callable);
    let analysis = analyze_reachability(&program).unwrap();
    assert!(
        analysis
            .outgoing()
            .iter()
            .flat_map(|outgoing| outgoing.dependencies())
            .any(|dependency| dependency.kind() == kind),
        "the reachable graph for missing {callable} contained no {kind:?} dependency"
    );
}

fn assert_verified(program: MirProgram) -> VerifiedFinalMirProgram {
    verify_final_mir(program).expect("unreachable definitions may be absent from final MIR")
}

#[test]
fn inactive_static_access_is_allowed_only_in_unreachable_retained_code() {
    let program = sparse_static_program(SPARSE_ACCESS_SOURCE);
    let dead = function(&program, "dead");
    let verified = verify_final_mir(program).expect("unreachable access is not executable");

    assert!(verified.program().has_executable_definition(dead));
    assert!(!verified
        .reachability()
        .static_accesses()
        .iter()
        .any(|access| {
            access.target() == static_field(verified.program(), "State", "inactive")
        }));
}

#[test]
fn reachable_inactive_static_access_has_exact_deterministic_failure() {
    let mut program = sparse_static_program(SPARSE_ACCESS_SOURCE);
    program.entry_function = match function(&program, "dead") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };
    let entry = program.entry_function;
    let inactive = static_field(&program, "State", "inactive");
    let first = verify_final_mir(program.clone()).unwrap_err();
    let second = verify_final_mir(program).unwrap_err();

    assert_eq!(first, second);
    assert!(first.iter().any(|error| {
        error.callable == Some(entry.into())
            && error
                .message
                .contains(&format!("targets inactive field {inactive}"))
            && error.message.contains("Entry")
    }));
}

#[test]
fn structural_false_branch_does_not_hide_reachable_inactive_access() {
    let mut program = sparse_static_program(
        "class State {
           static live: i64 = 1;
           static inactive: i64 = 2;
           init() {}
         }
         fn branch() -> i64 {
           if (false) { return State.inactive; }
           return 0;
         }
         fn main() -> i64 { return State.live; }",
    );
    program.entry_function = match function(&program, "branch") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };

    assert!(verify_final_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains("targets inactive field")));
}

#[test]
fn changed_entry_rebuilds_reachability_and_rejects_new_inactive_access() {
    let mut program = sparse_static_program(SPARSE_ACCESS_SOURCE);
    verify_final_mir(program.clone()).expect("original entry accesses only active storage");
    program.entry_function = match function(&program, "dead") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };

    assert!(verify_final_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains("targets inactive field")));
}

#[test]
fn active_lifecycle_remains_reachable_after_ordinary_access_disappears() {
    let mut program = sparse_static_program(
        "class State { static live: i64 = 1; init() {} }
         fn inert() -> i64 { return 0; }
         fn main() -> i64 { return State.live; }",
    );
    program.entry_function = match function(&program, "inert") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };
    let live = static_field(&program, "State", "live");
    let verified = verify_final_mir(program).expect("activation is monotone across final MIR");

    assert!(verified
        .program()
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .proof()
        .activation()
        .contains(live));
    assert!(verified
        .reachability()
        .runtime_entities()
        .contains(&super::super::MirRuntimeEntity::StaticStorage(live)));
    assert!(!verified
        .reachability()
        .static_accesses()
        .iter()
        .any(|access| access.target() == live && !access.is_lifecycle_owned()));
}

#[test]
fn final_structure_is_sparse_but_preliminary_producer_output_remains_complete() {
    let source = "fn dead() -> i64 { return 1; }
                  fn main() -> i64 { return 0; }";
    let mut final_program = lower_generic_source_to_final_mir(source);
    let dead = function(&final_program, "dead");
    final_program.remove_executable_definition_for_test(dead);

    verify_mir(&final_program).expect("final structural verification permits sparse slots");
    assert_verified(final_program);

    let mut preliminary = lower_generic_source_to_preliminary_mir(source);
    preliminary
        .program_mut()
        .remove_executable_definition_for_test(dead);
    let errors = verify_preliminary_mir(&preliminary).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.callable == Some(dead) && error.message == "internal function has no definition"
    }));
}

#[test]
fn preliminary_producer_completeness_covers_every_member_definition_kind() {
    let source = "class Complete {
                    value: i64;
                    init(value: i64) { self.value = value; }
                    copy(ref other: Complete) { self.value = other.value; }
                    assign(ref other: Complete) { self.value = other.value; }
                    destroy {}
                    fn read() -> i64 { return self.value; }
                    static fn answer() -> i64 { return 42; }
                  }
                  fn main() -> i64 { return 0; }";
    let preliminary = lower_generic_source_to_preliminary_mir(source);
    let class = preliminary
        .program()
        .class(class(preliminary.program(), "Complete"))
        .unwrap();
    let mut members = class
        .initializers
        .iter()
        .map(|item| CallableId::Initializer(item.id))
        .collect::<Vec<_>>();
    members.push(
        class
            .copy_constructor_declaration
            .as_ref()
            .unwrap()
            .id
            .into(),
    );
    members.push(
        class
            .copy_assignment_declaration
            .as_ref()
            .unwrap()
            .id
            .into(),
    );
    members.push(class.destruction.destructor.as_ref().unwrap().id.into());
    members.extend(class.methods.iter().map(|item| CallableId::Method(item.id)));

    for member in members {
        let mut sparse = preliminary.clone();
        sparse
            .program_mut()
            .remove_executable_definition_for_test(member);
        let errors = verify_preliminary_mir(&sparse).unwrap_err();
        assert!(
            errors.iter().any(|error| error.callable == Some(member)),
            "preliminary verification accepted missing member {member}: {errors}"
        );
    }
}

#[test]
fn unreachable_functions_and_every_member_definition_kind_may_be_absent() {
    let mut program = lower_generic_source_to_final_mir(
        "class Dormant {
           value: i64;
           init(value: i64) { self.value = value; }
           copy(ref other: Dormant) { self.value = other.value; }
           assign(ref other: Dormant) { self.value = other.value; }
           destroy {}
           fn read() -> i64 { return self.value; }
           static fn static_read() -> i64 { return 7; }
         }
         fn dead() -> i64 { return 9; }
         fn main() -> i64 { return 0; }",
    );
    let dormant = program.class(class(&program, "Dormant")).unwrap().clone();
    let mut removed = vec![function(&program, "dead")];
    removed.extend(
        dormant
            .initializers
            .iter()
            .map(|item| CallableId::Initializer(item.id)),
    );
    removed.extend(
        dormant
            .copy_constructor_declaration
            .iter()
            .map(|item| CallableId::CopyConstructor(item.id)),
    );
    removed.extend(
        dormant
            .copy_assignment_declaration
            .iter()
            .map(|item| CallableId::CopyAssignment(item.id)),
    );
    removed.extend(
        dormant
            .destruction
            .destructor
            .iter()
            .map(|item| CallableId::Destructor(item.id)),
    );
    removed.extend(
        dormant
            .methods
            .iter()
            .map(|item| CallableId::Method(item.id)),
    );
    for callable in &removed {
        program.remove_executable_definition_for_test(*callable);
    }

    let verified = assert_verified(program);
    for callable in removed {
        assert!(!verified.program().has_executable_definition(callable));
        assert!(!verified
            .reachability()
            .reachable_callables()
            .contains(&callable));
    }
}

#[test]
fn missing_entry_and_direct_static_or_instance_targets_name_their_category() {
    let entry = lower_generic_source_to_final_mir("fn main() -> i64 { return 0; }");
    let main = entry.entry_function.into();
    assert_missing_category(entry, main, "entry-root");

    let direct = lower_generic_source_to_final_mir(
        "fn leaf() -> i64 { return 1; }
         fn main() -> i64 { return leaf(); }",
    );
    let leaf = function(&direct, "leaf");
    assert_missing_category(direct, leaf, "direct-call");

    let static_call = lower_generic_source_to_final_mir(
        "class Utility { init() {} static fn answer() -> i64 { return 2; } }
         fn main() -> i64 { return Utility.answer(); }",
    );
    let answer = method(&static_call, "Utility", "answer");
    assert_missing_category(static_call, answer, "static-call");

    let instance = lower_generic_source_to_final_mir(
        "class Item {
           init() {}
           fn answer() -> i64 { return 3; }
         }
         fn main() -> i64 { var item: Item = Item(); return item.answer(); }",
    );
    let answer = method(&instance, "Item", "answer");
    assert_missing_category(instance, answer, "direct-method-call");
}

#[test]
fn missing_virtual_and_interface_targets_are_required_only_when_selected() {
    let source = "interface View { fn read() -> i64; }
                  class Base implements View {
                    init() {}
                    virtual fn read() -> i64 { return 1; }
                  }
                  class Child extends Base {
                    init() { super(); }
                    override fn read() -> i64 { return 2; }
                  }
                  fn main() -> i64 { return 0; }";
    let program = lower_generic_source_to_final_mir(source);
    let child_read = method(&program, "Child", "read");
    let mut unselected = program.clone();
    unselected.remove_executable_definition_for_test(child_read);
    assert_verified(unselected);

    let virtual_call = lower_generic_source_to_final_mir(format!(
        "{source}\nfn invoke(ref value: Base) -> i64 {{ return value.read(); }}\nfn used() -> i64 {{ return invoke(Child()); }}"
    ));
    let child_read = method(&virtual_call, "Child", "read");
    // Retarget the selected entry without changing source-facing declarations.
    let mut virtual_call = virtual_call;
    virtual_call.entry_function = match function(&virtual_call, "used") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };
    assert_missing_category(virtual_call, child_read, "virtual-dispatch");

    let interface_call = lower_generic_source_to_final_mir(format!(
        "{source}\nfn invoke(ref value: View) -> i64 {{ return value.read(); }}\nfn used() -> i64 {{ return invoke(Child()); }}"
    ));
    let child_read = method(&interface_call, "Child", "read");
    let mut interface_call = interface_call;
    interface_call.entry_function = match function(&interface_call, "used") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };
    assert_missing_category(interface_call, child_read, "interface-dispatch");
}

#[test]
fn missing_addressed_and_indirect_targets_are_rejected() {
    let addressed = lower_generic_source_to_final_mir(
        "fn target() -> i64 { return 4; }
         fn main() -> i64 { var callback: fn() -> i64 = target; return 0; }",
    );
    let target = function(&addressed, "target");
    assert_missing_category(addressed, target, "callable-address-retention");

    let indirect = lower_generic_source_to_final_mir(
        "fn target() -> i64 { return 4; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn main() -> i64 { return invoke(target); }",
    );
    let target = function(&indirect, "target");
    assert_reachable_graph_contains_kind(
        indirect.clone(),
        target,
        MirDependencyEdgeKind::IndirectCall,
    );
    assert_missing_category(indirect, target, "callable-address-retention");
}

#[test]
fn missing_explicit_lifecycle_member_targets_are_rejected() {
    let source = "class Item {
                    value: i64;
                    init(value: i64) { self.value = value; }
                    copy(ref other: Item) { self.value = other.value; }
                    assign(ref other: Item) { self.value = other.value; }
                    destroy {}
                  }
                  fn main() -> i64 {
                    var first: Item = Item(1);
                    var second: Item = first;
                    first = second;
                    return 0;
                  }";
    let program = lower_generic_source_to_final_mir(source);
    let item = program.class(class(&program, "Item")).unwrap();
    let initializer = item.initializers[0].id.into();
    let copy_constructor = item
        .copy_constructor_declaration
        .as_ref()
        .unwrap()
        .id
        .into();
    let copy_assignment = item.copy_assignment_declaration.as_ref().unwrap().id.into();
    let destructor = item.destruction.destructor.as_ref().unwrap().id.into();
    assert_missing_category(program.clone(), initializer, "initializer");
    assert_reachable_graph_contains_kind(
        program.clone(),
        copy_constructor,
        MirDependencyEdgeKind::CopyConstructor,
    );
    assert_missing_category(program.clone(), copy_constructor, "user-copy-body");
    assert_reachable_graph_contains_kind(
        program.clone(),
        copy_assignment,
        MirDependencyEdgeKind::CopyAssignment,
    );
    assert_missing_category(program.clone(), copy_assignment, "user-copy-body");
    assert_missing_category(program, destructor, "user-destructor");
}

#[test]
fn optional_shared_array_and_static_roots_retain_their_transitive_bodies() {
    let aggregate = lower_generic_source_to_final_mir(
        "class Item {
           init() {}
           copy(ref other: Item) {}
           assign(ref other: Item) {}
           destroy {}
         }
         fn main() -> i64 {
           var optional: Item? = Item();
           var items: Item[] = Item[]{Item()};
           return 0;
         }",
    );
    let destructor = aggregate
        .class(class(&aggregate, "Item"))
        .unwrap()
        .destruction
        .destructor
        .as_ref()
        .unwrap()
        .id
        .into();
    assert_reachable_graph_contains_kind(
        aggregate.clone(),
        destructor,
        MirDependencyEdgeKind::OptionalLifecycle,
    );
    assert_reachable_graph_contains_kind(
        aggregate.clone(),
        destructor,
        MirDependencyEdgeKind::ArrayDestruction,
    );
    assert_missing_category(aggregate, destructor, "user-destructor");

    let shared = lower_generic_source_to_final_mir(
        "class Item { init() {} destroy {} }
         fn main() -> i64 { var owner: shared Item = new Item(); return 0; }",
    );
    let destructor = shared
        .class(class(&shared, "Item"))
        .unwrap()
        .destruction
        .destructor
        .as_ref()
        .unwrap()
        .id
        .into();
    assert_reachable_graph_contains_kind(
        shared.clone(),
        destructor,
        MirDependencyEdgeKind::SharedFinalizer,
    );
    assert_missing_category(shared, destructor, "user-destructor");

    let static_program = lower_generic_source_to_final_mir(
        "class State { static value: i64 = 7; init() {} }
         fn main() -> i64 { return State.value; }",
    );
    let initializer = static_program
        .static_lifecycle
        .as_ref()
        .unwrap()
        .initializers()[0]
        .callable();
    let mut missing_initializer = static_program;
    missing_initializer.remove_executable_definition_for_test(initializer);
    let analysis = analyze_reachability(&missing_initializer).unwrap();
    assert!(analysis.roots().iter().any(|root| matches!(
        root.reason(),
        MirReachabilityRootReason::StaticActivation(_)
    )));
    let errors = verify_final_mir(missing_initializer).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("initializer bodies do not exactly cover active explicit fields")));

    let static_shutdown = lower_generic_source_to_final_mir(
        "class Item { init() {} destroy {} }
         class State { static item: Item = Item(); init() {} }
         fn main() -> i64 { State.item = Item(); return 0; }",
    );
    let destructor = static_shutdown
        .class(class(&static_shutdown, "Item"))
        .unwrap()
        .destruction
        .destructor
        .as_ref()
        .unwrap()
        .id
        .into();
    let mut sparse_shutdown = static_shutdown.clone();
    sparse_shutdown.remove_executable_definition_for_test(destructor);
    let analysis = analyze_reachability(&sparse_shutdown).unwrap();
    assert!(analysis
        .roots()
        .iter()
        .any(|root| matches!(root.reason(), MirReachabilityRootReason::StaticShutdown(_))));
    assert_missing_category(static_shutdown, destructor, "user-destructor");
}

#[test]
fn retained_unreachable_bodies_are_still_fully_verified() {
    let mut program = lower_generic_source_to_final_mir(
        "fn dead() -> i64 { return 1; }
         fn main() -> i64 { return 0; }",
    );
    let dead = match function(&program, "dead") {
        CallableId::Function(function) => function,
        _ => unreachable!(),
    };
    program
        .definitions
        .get_mut_for_test(dead)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let errors = verify_final_mir(program).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.callable == Some(dead.into()) && error.message.contains("block has no terminator")
    }));
}

#[test]
fn sparse_unrelated_definitions_preserve_static_lifecycle_authority() {
    let mut program = lower_generic_source_to_final_mir(
        "class State { static value: i64 = 7; init() {} }
         fn dead() -> i64 { return State.value; }
         fn main() -> i64 { return State.value; }",
    );
    let dead = function(&program, "dead");
    program.remove_executable_definition_for_test(dead);

    let verified = assert_verified(program);
    assert!(!verified.program().has_executable_definition(dead));
    assert!(verified.program().static_lifecycle.is_some());
}

#[test]
fn multiple_missing_targets_have_canonical_error_order() {
    let mut program = lower_generic_source_to_final_mir(
        "fn first() -> i64 { return 1; }
         fn second() -> i64 { return 2; }
         fn main() -> i64 { return second() + first(); }",
    );
    let first = function(&program, "first");
    let second = function(&program, "second");
    program.remove_executable_definition_for_test(first);
    program.remove_executable_definition_for_test(second);

    let errors = verify_final_mir(program.clone()).unwrap_err();
    assert_eq!(errors, verify_final_mir(program).unwrap_err());
    assert_eq!(
        errors
            .iter()
            .map(|error| error.callable.unwrap())
            .collect::<Vec<_>>(),
        [first, second]
    );
}
