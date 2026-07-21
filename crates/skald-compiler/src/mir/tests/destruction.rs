use super::*;
use crate::{
    identity::{CallableId, DestructorId},
    passes::run_mir_pipeline,
};

fn cleanup_program() -> MirProgram {
    let mut program = lower_text(concat!(
        "class Leaf { init() {} }\n",
        "class Owner { leaf: Leaf; init() { self.leaf = Leaf(); } }\n",
        "fn main() -> i64 { var owner: Owner = Owner(); return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let storage = function
        .storage
        .iter()
        .find(|storage| storage.ty == MirType::Class(ClassId::new(1)))
        .unwrap()
        .id;
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: MirPlace::base(storage),
            target: ClassId::new(1),
            span: function.span,
        }));
    program
}

fn cleanup_mut(program: &mut MirProgram) -> &mut MirCleanup {
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup) => Some(cleanup),
            _ => None,
        })
        .unwrap()
}

fn messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn lowers_user_bodies_and_canonical_recursive_destruction_plans() {
    let program = lower_text(concat!(
        "class Leaf { value: i64; init() { self.value = 0; } destroy { self.value = 1; } }\n",
        "class Empty { init() {} }\n",
        "class Pair { left: Leaf; empty: Empty; right: Leaf; init() { self.left = Leaf(); self.empty = Empty(); self.right = Leaf(); } destroy { return; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(verify_mir(&program).is_ok());
    let leaf = program.class(ClassId::new(0)).unwrap();
    let leaf_destructor = DestructorId::new(leaf.id, 0);
    assert_eq!(
        leaf.destruction.steps,
        [MirDestructionStep::UserBody(leaf_destructor)]
    );
    assert!(program
        .member_definition(CallableId::Destructor(leaf_destructor))
        .is_some());

    let empty = program.class(ClassId::new(1)).unwrap();
    assert!(empty.destruction.steps.is_empty());

    let pair = program.class(ClassId::new(2)).unwrap();
    let pair_destructor = DestructorId::new(pair.id, 0);
    assert_eq!(
        pair.destruction.steps,
        [
            MirDestructionStep::UserBody(pair_destructor),
            MirDestructionStep::Field(FieldId::new(pair.id, 2)),
            MirDestructionStep::Field(FieldId::new(pair.id, 1)),
            MirDestructionStep::Field(FieldId::new(pair.id, 0)),
        ]
    );
    assert!(program
        .executable_definitions()
        .flat_map(|definition| definition.values())
        .all(|value| !matches!(value.ty, MirType::Class(_))));
}

#[test]
fn verifies_cleanup_of_live_owning_roots_and_deep_fields_through_the_pipeline() {
    let program = cleanup_program();
    assert!(verify_mir(&program).is_ok());
    assert!(run_mir_pipeline(program.clone()).is_ok());

    let mut deep = program;
    let cleanup = cleanup_mut(&mut deep);
    cleanup.destination = cleanup
        .destination
        .clone()
        .project_field(FieldId::new(ClassId::new(1), 0));
    cleanup.target = ClassId::new(0);
    assert!(verify_mir(&deep).is_ok());
}

#[test]
fn verifies_cleanup_of_an_initialized_empty_class() {
    let mut program = lower_text(concat!(
        "class Empty { init() {} }\n",
        "fn main() -> i64 { var empty: Empty = Empty(); return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let storage = function.storage[0].id;
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: storage.into(),
            target: ClassId::new(0),
            span: function.span,
        }));

    assert!(verify_mir(&program).is_ok());
}

#[test]
fn rejects_wrong_class_non_owning_foreign_and_scalar_cleanup_targets() {
    let mut wrong_class = cleanup_program();
    cleanup_mut(&mut wrong_class).target = ClassId::new(0);
    assert!(messages(&wrong_class)
        .iter()
        .any(|message| message.contains("wrong class type")));

    let mut non_owning = cleanup_program();
    let cleanup = cleanup_mut(&mut non_owning);
    cleanup.destination.base = MirPlaceBase::AliasParameter(cleanup.destination.base.storage());
    assert!(messages(&non_owning)
        .iter()
        .any(|message| message.contains("must be owning storage")));

    let mut foreign = cleanup_program();
    cleanup_mut(&mut foreign).destination = MirPlace::base(StorageId::new(FunctionId::new(99), 0));
    assert!(messages(&foreign)
        .iter()
        .any(|message| message.contains("is not declared in this function")));

    let mut scalar = lower_text(concat!(
        "class Empty { init() {} }\n",
        "fn main() -> i64 { var scalar: i64 = 0; var empty: Empty = Empty(); return 0; }\n",
    ));
    let function = scalar
        .definitions
        .get_mut_for_test(scalar.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: function.storage[0].id.into(),
            target: ClassId::new(0),
            span: function.span,
        }));
    assert!(messages(&scalar)
        .iter()
        .any(|message| message.contains("must have class type")));
}

#[test]
fn rejects_read_only_dead_and_duplicated_cleanup_targets() {
    let mut read_only = lower_text(concat!(
        "class Resource { init() {} fn inspect() -> unit {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let method = MethodId::new(ClassId::new(0), 0);
    let definition = read_only
        .member_definitions
        .get_mut_for_test(method.into())
        .unwrap();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: definition.receiver.into(),
            target: ClassId::new(0),
            span: definition.span,
        }));
    assert!(messages(&read_only)
        .iter()
        .any(|message| message.contains("requires mutable access")));

    let mut dead = cleanup_program();
    let function = dead
        .definitions
        .get_mut_for_test(dead.entry_function)
        .unwrap();
    let cleanup_index = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .unwrap();
    let cleanup = function.body.blocks[0].instructions.remove(cleanup_index);
    function.body.blocks[0].instructions.insert(0, cleanup);
    assert!(messages(&dead)
        .iter()
        .any(|message| message.contains("is not live")));

    let mut duplicated = cleanup_program();
    let function = duplicated
        .definitions
        .get_mut_for_test(duplicated.entry_function)
        .unwrap();
    let cleanup = function.body.blocks[0]
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .unwrap()
        .clone();
    function.body.blocks[0].instructions.push(cleanup);
    assert!(messages(&duplicated)
        .iter()
        .any(|message| message.contains("more than once")));
}

#[test]
fn rejects_noncanonical_destruction_order() {
    let mut program = lower_text(concat!(
        "class Leaf { init() {} }\n",
        "class Pair { left: Leaf; right: Leaf; init() { self.left = Leaf(); self.right = Leaf(); } destroy {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[1]
        .destruction
        .steps
        .swap(0, 2);

    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("user body first and class fields in reverse")));
}

#[test]
fn cleanup_dump_is_exact_and_target_independent() {
    let program = cleanup_program();
    let dump = dump_mir(&program);
    assert_eq!(
        dump,
        concat!(
            "MirProgram @0..143\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 \"Leaf\" @0..24\n",
            "      Initializer c0:init0() @13..22\n",
            "    Class c1 \"Owner\" @25..83\n",
            "      Field c1:field0 \"leaf\" : class c0 @39..50\n",
            "      Initializer c1:init0() @51..81\n",
            "      DestructionPlan\n",
            "        Field c1:field0\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @84..142\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @84..142\n",
            "      Parameters\n",
            "      Storage\n",
            "        f0:s0 local f0:l0 \"owner\" : class c1 @103..130\n",
            "      Values\n",
            "        f0:v0 : i64 @138..139\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @101..142\n",
            "          initialize f0:s0 with c1:init0() @122..129\n",
            "          f0:v0 = const.i64 0 : i64 @138..139\n",
            "          cleanup f0:s0 as c1 @84..142\n",
            "          return f0:v0 @131..140\n",
            "  MemberDefinitions\n",
            "    MemberDefinition c0:init0 @13..22\n",
            "      Receiver c0:init0:s0\n",
            "      Parameters\n",
            "      Storage\n",
            "        c0:init0:s0 receiver c0:init0:self \"self\" : class c0 @20..22\n",
            "      Values\n",
            "      EntryBlock c0:init0:b0\n",
            "      Blocks\n",
            "        c0:init0:b0 @20..22\n",
            "          return @20..22\n",
            "    MemberDefinition c1:init0 @51..81\n",
            "      Receiver c1:init0:s0\n",
            "      Parameters\n",
            "      Storage\n",
            "        c1:init0:s0 receiver c1:init0:self \"self\" : class c1 @58..81\n",
            "      Values\n",
            "      EntryBlock c1:init0:b0\n",
            "      Blocks\n",
            "        c1:init0:b0 @58..81\n",
            "          initialize c1:init0:s0.field(c1:field0) with c0:init0() @60..79\n",
            "          return @58..81\n",
        )
    );
    assert!(!dump.contains("offset"));
}
