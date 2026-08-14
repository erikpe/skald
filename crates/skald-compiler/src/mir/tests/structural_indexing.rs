//! Ownership and verifier coverage for calls selected by structural brackets.

use super::*;

const OWNED_CALL_SOURCE: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  copy(ref source: Item) { self.value = source.value; }\n",
    "  assign(ref source: Item) { self.value = source.value; }\n",
    "  destroy {}\n",
    "}\n",
    "class Values {\n",
    "  item: Item;\n",
    "  init(value: Item) { self.item = value; }\n",
    "  fn index_get(key: i64) -> Item { return self.item; }\n",
    "  mut fn index_set(key: i64, replacement: Item) -> unit { self.item = replacement; }\n",
    "}\n",
    "fn discard(value: Item) -> unit {}\n",
    "fn main() -> i64 {\n",
    "  var values: Values = Values(Item(1));\n",
    "  var named: Item = Item(2);\n",
    "  values[0] = named;\n",
    "  values[1] = Item(3);\n",
    "  values[2] = values[3];\n",
    "  var result: Item = values[4];\n",
    "  discard(values[5]);\n",
    "  return result.value;\n",
    "}\n",
);

fn structural_method_calls(definition: &MirFunctionDefinition) -> Vec<&MirCall> {
    definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if matches!(
                    call.target,
                    MirCallTarget::Method(MirMethodCallTarget::Direct(method))
                        if method.class() == ClassId::new(1)
                ) =>
            {
                Some(call)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn structural_calls_reuse_owned_argument_result_and_cleanup_lowering() {
    let checked = type_check_source(OWNED_CALL_SOURCE);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked
        .hir
        .expect("valid structural ownership source has HIR");
    let preliminary = crate::mir::lower_preliminary_hir(&hir);
    crate::mir::verify_preliminary_mir(&preliminary)
        .expect("ordinary preliminary call ownership must verify");

    let program = crate::test_support::lower_hir_to_final_mir(&hir);
    verify_mir(&program).expect("ordinary final call ownership must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let calls = structural_method_calls(main);
    assert_eq!(calls.len(), 6);

    let getter = MethodId::new(ClassId::new(1), 0);
    let setter = MethodId::new(ClassId::new(1), 1);
    let getters = calls
        .iter()
        .copied()
        .filter(|call| call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(getter)))
        .collect::<Vec<_>>();
    let setters = calls
        .iter()
        .copied()
        .filter(|call| call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(setter)))
        .collect::<Vec<_>>();
    assert_eq!(getters.len(), 3);
    assert_eq!(setters.len(), 3);

    assert!(getters.iter().all(|call| {
        call.result.is_none()
            && call.shared_result.is_none()
            && call.destination.as_ref().is_some_and(|destination| {
                main.storage(destination.base.expect_local_storage())
                    .is_some_and(|storage| {
                        matches!(
                            storage.kind,
                            MirStorageKind::Argument
                                | MirStorageKind::Local
                                | MirStorageKind::Temporary
                        ) && storage.ty == MirType::Class(ClassId::new(0))
                    })
            })
    }));
    assert!(setters.iter().all(|call| {
        matches!(
            call.arguments.as_slice(),
            [MirArgument::Value(_), MirArgument::OwnedPlace(_)]
        ) && call.result.is_none()
            && call.shared_result.is_none()
            && call.destination.is_none()
    }));

    let instructions = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect::<Vec<_>>();
    let named_copy = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .expect("named setter replacement must be copied into argument storage");
    let first_setter = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call) if call.target
                == MirCallTarget::Method(MirMethodCallTarget::Direct(setter)))
        })
        .unwrap();
    assert!(named_copy < first_setter);
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_))));
}

#[test]
fn structural_calls_verify_every_generic_owning_value_family() {
    let source = concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  assign(ref source: Item) { self.value = source.value; }\n",
        "  destroy {}\n",
        "}\n",
        "class Cell<T> {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn index_get(key: i64) -> T { return self.value; }\n",
        "  mut fn index_set(key: i64, replacement: T) -> unit { self.value = replacement; }\n",
        "  fn slice_get(start: i64?, end: i64?) -> T { return self.value; }\n",
        "  mut fn slice_set(start: i64?, end: i64?, replacement: T) -> unit { self.value = replacement; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var primitive: Cell<i64> = Cell<i64>(1); primitive[0] = 2; primitive[:] = 3;\n",
        "  var item: Item = Item(4);\n",
        "  var objects: Cell<Item> = Cell<Item>(item); objects[0] = item; objects[:] = Item(5);\n",
        "  var object_result: Item = objects[0];\n",
        "  var arrays: Cell<i64[]> = Cell<i64[]>(i64[]{6, 7});\n",
        "  var named_array: i64[] = i64[]{8, 9}; arrays[0] = named_array; arrays[:] = i64[]{10};\n",
        "  var array_result: i64[] = arrays[:];\n",
        "  var optionals: Cell<Item?> = Cell<Item?>(Item(11)); optionals[0] = none; optionals[:] = item;\n",
        "  var optional_result: Item? = optionals[0];\n",
        "  var owner: shared Item = new Item(12);\n",
        "  var owners: Cell<shared Item> = Cell<shared Item>(owner); owners[0] = owner; owners[:] = new Item(13);\n",
        "  var owner_result: shared Item = owners[:];\n",
        "  var inner: Cell<Item> = Cell<Item>(Item(14));\n",
        "  var nested: Cell<Cell<Item>> = Cell<Cell<Item>>(inner); nested[0] = inner; nested[:] = Cell<Item>(Item(15));\n",
        "  var nested_result: Cell<Item> = nested[0];\n",
        "  var nested_item: Item = nested_result[0];\n",
        "  return primitive[0] + object_result.value + array_result[0] + optional_result!.value",
        " + owner_result->value + nested_item.value;\n",
        "}\n",
    );

    let preliminary = crate::test_support::lower_generic_source_to_preliminary_mir(source);
    crate::mir::verify_preliminary_mir(&preliminary)
        .expect("generic structural ownership must verify before static planning");
    let final_program = crate::test_support::lower_generic_source_to_final_mir(source);
    verify_mir(&final_program).expect("generic structural ownership must verify after planning");
    let dump = dump_mir(&final_program);
    assert!(!dump.contains("Structural"));
    assert_eq!(dump, dump_mir(&final_program));
}

#[test]
fn checked_shared_brackets_retain_receiver_and_argument_owners_through_calls() {
    let program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Table {\n",
        "  item: shared Item;\n",
        "  init(value: i64) { self.item = new Item(value); }\n",
        "  fn index_get(key: i64) -> shared Item { return self.item; }\n",
        "  mut fn index_set(key: i64, replacement: shared Item) -> unit { self.item = replacement; }\n",
        "}\n",
        "fn effect() -> i64 { return 0; }\n",
        "fn checked(owner: shared Obj, replacement: shared Item) -> shared Item {\n",
        "  ((shared Table) owner)->[effect()] = replacement;\n",
        "  return ((shared Table) owner)->[effect()];\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var table: shared Table = new Table(1);\n",
        "  var erased: shared Obj = table;\n",
        "  var replacement: shared Item = new Item(2);\n",
        "  var result: shared Item = checked(erased, replacement);\n",
        "  return result->value;\n",
        "}\n",
    ));
    verify_mir(&program).expect("checked shared structural calls must verify");
    let checked = program.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(
        checked
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::SharedAnchor)
            .count(),
        2
    );

    let instructions = checked
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect::<Vec<_>>();
    let getter = MethodId::new(ClassId::new(1), 0);
    let setter = MethodId::new(ClassId::new(1), 1);
    let setter_call = instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(setter)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("checked structural setter remains an ordinary method call");
    assert!(matches!(
        setter_call.arguments.as_slice(),
        [MirArgument::Value(_), MirArgument::SharedOwner(_)]
    ));
    let getter_index = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(getter))
                    && call.shared_result.is_some())
        })
        .expect("checked structural getter has an ordinary shared result");
    let later_anchor_release = instructions
        .iter()
        .enumerate()
        .skip(getter_index + 1)
        .find(|(_, instruction)| {
            matches!(instruction, MirInstruction::SharedRelease(release)
                if checked.storage[release.owner.index()].kind == MirStorageKind::SharedAnchor)
        })
        .map(|(index, _)| index)
        .expect("checked receiver anchor is released after the getter result is secured");
    assert!(getter_index < later_anchor_release);
}

#[test]
fn verifier_rejects_malformed_ordinary_calls_originating_from_brackets() {
    let mut non_owned_setter = lower_text(OWNED_CALL_SOURCE);
    let main = non_owned_setter
        .definitions
        .get_mut_for_test(non_owned_setter.entry_function)
        .unwrap();
    let setter = MethodId::new(ClassId::new(1), 1);
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(setter)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    let MirArgument::OwnedPlace(replacement) = &call.arguments[1] else {
        panic!("structural setter must transfer one owned replacement");
    };
    call.arguments[1] = MirArgument::Place(replacement.clone());
    let errors = verify_mir(&non_owned_setter).unwrap_err().to_string();
    assert!(errors.contains("must be a scalar value or owned place"));

    let mut projected_getter_result = lower_text(OWNED_CALL_SOURCE);
    let main = projected_getter_result
        .definitions
        .get_mut_for_test(projected_getter_result.entry_function)
        .unwrap();
    let getter = MethodId::new(ClassId::new(1), 0);
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Method(MirMethodCallTarget::Direct(getter)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.destination = Some(
        call.destination
            .clone()
            .expect("class getter has a caller destination")
            .project_field(FieldId::new(ClassId::new(0), 0)),
    );
    let errors = verify_mir(&projected_getter_result)
        .unwrap_err()
        .to_string();
    assert!(errors.contains("complete exact-class local or temporary destination storage"));
}
