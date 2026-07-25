use super::*;

#[test]
fn copy_allocation_records_the_selected_operation_and_checked_source() {
    let output = type_check_source(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Item = Item(7);\n",
        "  var owner: shared Item = new Item(copy source);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("valid copy allocation must produce HIR");
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(owner) = &main.body.statements[1] else {
        panic!("expected shared owner local");
    };
    let HirLocalInitializer::Shared(owner) = &owner.initializer else {
        panic!("expected shared owner initialization");
    };
    let HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) = &owner.source else {
        panic!("expected copy allocation producer");
    };
    let crate::hir::HirSharedAllocationMode::Copy { source, operation } = &allocation.mode else {
        panic!("expected explicit copy-allocation mode");
    };
    assert!(matches!(
        source.as_ref(),
        crate::hir::HirObjectSource::Checked(_)
    ));
    assert_eq!(
        *operation,
        hir.class(allocation.class)
            .unwrap()
            .copy_constructor
            .selected()
            .unwrap()
    );
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("copy-allocation MIR must verify");
}

#[test]
fn copy_allocation_accepts_alias_produced_and_shared_checked_sources() {
    let output = type_check_source(concat!(
        "class Animal {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Animal) { self.value = source.value; }\n",
        "}\n",
        "class Dog extends Animal {\n",
        "  extra: i64;\n",
        "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
        "  copy(ref source: Dog) { self.extra = source.extra; }\n",
        "}\n",
        "fn from_alias(ref dog: Dog) -> shared Animal {\n",
        "  return new Animal(copy dog);\n",
        "}\n",
        "fn from_shared(animal: shared Animal) -> shared Dog {\n",
        "  return new Dog(copy animal);\n",
        "}\n",
        "fn from_inline_producer() -> shared Animal {\n",
        "  return new Animal(copy Dog(3, 4));\n",
        "}\n",
        "fn from_shared_producer() -> shared Animal {\n",
        "  return new Animal(copy new Dog(5, 6));\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("all supported copy sources must type-check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("all supported copy sources must verify");
    let copy_allocations = mir
        .definitions
        .iter()
        .flat_map(|definition| &definition.body.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::SharedAllocate(crate::mir::MirSharedAllocate {
                    mode: crate::mir::MirSharedAllocationMode::Copy { .. },
                    ..
                })
            )
        })
        .count();
    assert_eq!(copy_allocations, 4);
    assert!(mir
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .blocks
        .iter()
        .any(|block| matches!(
            block.terminator,
            Some(crate::mir::MirTerminator::CheckedCast { .. })
        )));
}
