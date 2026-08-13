use super::*;
use crate::{
    hir::{
        HirCopyCapability, HirDestructionStep, HirOptionalStorageCategory, HirSharedTarget,
        HirSynthesizedFieldCopy, Type,
    },
    identity::FieldId,
    resolve::ResolvedCopyOperation,
    typeck::COPY_OPERATION_UNAVAILABLE,
};

#[test]
fn ordinary_application_sites_use_the_generated_closed_class() {
    let hir = check_generic_source(
        "class Box<T> {\n\
           value: T;\n\
           init(value: T) { self.value = value; }\n\
           fn get() -> T { return self.value; }\n\
         }\n\
         fn round_trip(value: Box<i64>) -> Box<i64> {\n\
           var result: Box<i64> = Box<i64>(value.get());\n\
           return result;\n\
         }\n\
         fn main() -> i64 {\n\
           var value: Box<i64> = Box<i64>(41);\n\
           var result: Box<i64> = round_trip(value);\n\
           return result.get();\n\
         }\n",
    );

    let generated = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Box<"))
        .expect("the closed class must be present in HIR");
    assert_eq!(generated.fields[0].ty, Type::I64);

    let round_trip = hir
        .declarations
        .iter()
        .find(|function| function.name == "round_trip")
        .unwrap();
    assert_eq!(round_trip.parameters[0].ty, Type::Class(generated.id));
    assert_eq!(round_trip.return_type, Type::Class(generated.id));

    let dump = dump_hir(&hir);
    assert!(
        dump.contains(&format!("Construct {}", generated.id)),
        "{dump}"
    );
    assert!(
        dump.contains(&format!("ObjectCall function f0 -> {}", generated.id)),
        "{dump}"
    );
}

#[test]
fn specialized_fields_select_complete_copy_and_destruction_plans() {
    let hir = check_generic_source(
        "class Leaf {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           copy(ref source: Leaf) { self.value = source.value; }\n\
           assign(ref source: Leaf) { self.value = source.value; }\n\
           destroy {}\n\
         }\n\
         class Base { init() {} destroy {} }\n\
         class Envelope<T> extends Base {\n\
           direct: T;\n\
           nested: T??;\n\
           values: T?[];\n\
           matrix: T?[][];\n\
           owner: shared T;\n\
           maybe_owner: (shared T)?;\n\
           box: shared T?;\n\
           static cached: T?;\n\
           init(ref value: T, owner: shared T, maybe_owner: (shared T)?, box: shared T?) {\n\
             super();\n\
             self.direct = value;\n\
             self.nested = some(some(value));\n\
             self.values = T?[]{some(value), none};\n\
             self.matrix = T?[][]{T?[]{some(value)}};\n\
             self.owner = owner;\n\
             self.maybe_owner = maybe_owner;\n\
             self.box = box;\n\
           }\n\
         }\n\
         fn inspect(ref value: Envelope<Leaf>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let leaf = hir
        .classes
        .iter()
        .find(|class| class.name == "Leaf")
        .unwrap();
    let base = hir
        .classes
        .iter()
        .find(|class| class.name == "Base")
        .unwrap();
    let envelope = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Envelope<"))
        .unwrap();
    assert_eq!(envelope.direct_base.as_ref().unwrap().class, base.id);
    assert_eq!(envelope.fields[0].ty, Type::Class(leaf.id));

    let Type::Optional(outer_optional) = envelope.fields[1].ty else {
        panic!("the recursive optional field must stay optional")
    };
    let outer = hir.optional_type(outer_optional).unwrap();
    let HirOptionalStorageCategory::Nested(inner_optional) = outer.storage else {
        panic!("T?? must retain both optional layers")
    };
    assert_eq!(
        hir.optional_type(inner_optional).unwrap().payload,
        Type::Class(leaf.id)
    );

    let Type::Shared(HirSharedTarget::Class(owner_target)) = envelope.fields[4].ty else {
        panic!("shared T must close to an exact shared owner")
    };
    assert_eq!(owner_target, leaf.id);
    let Type::Optional(optional_owner) = envelope.fields[5].ty else {
        panic!("(shared T)? must remain an optional owner")
    };
    assert_eq!(
        hir.optional_type(optional_owner).unwrap().storage,
        HirOptionalStorageCategory::SharedOwner(HirSharedTarget::Class(leaf.id))
    );
    assert!(matches!(
        envelope.fields[6].ty,
        Type::Shared(HirSharedTarget::OptionalBox(_))
    ));
    assert!(matches!(envelope.static_fields[0].ty, Type::Optional(_)));

    let HirCopyCapability::Synthesized(copy) = &envelope.copy_constructor else {
        panic!("the closed class must receive synthesized copy construction")
    };
    assert_eq!(copy.class, envelope.id);
    assert_eq!(copy.base.as_ref().unwrap().base, base.id);
    assert!(matches!(
        copy.fields[0],
        HirSynthesizedFieldCopy::Class { .. }
    ));
    assert!(matches!(
        copy.fields[1],
        HirSynthesizedFieldCopy::Optional { .. }
    ));
    assert!(matches!(
        copy.fields[2],
        HirSynthesizedFieldCopy::Array { .. }
    ));
    assert!(matches!(
        copy.fields[3],
        HirSynthesizedFieldCopy::Array { .. }
    ));
    assert!(matches!(
        copy.fields[4],
        HirSynthesizedFieldCopy::Shared { .. }
    ));
    assert!(matches!(
        copy.fields[5],
        HirSynthesizedFieldCopy::OptionalShared { .. }
    ));
    assert!(matches!(
        copy.fields[6],
        HirSynthesizedFieldCopy::Shared { .. }
    ));

    let HirCopyCapability::Synthesized(assignment) = &envelope.copy_assignment else {
        panic!("the closed class must receive synthesized copy assignment")
    };
    assert_eq!(assignment.fields.len(), copy.fields.len());
    assert_eq!(
        envelope.destruction.steps,
        [
            HirDestructionStep::SharedField(FieldId::new(envelope.id, 6)),
            HirDestructionStep::OptionalSharedField(FieldId::new(envelope.id, 5)),
            HirDestructionStep::SharedField(FieldId::new(envelope.id, 4)),
            HirDestructionStep::ArrayField(FieldId::new(envelope.id, 3)),
            HirDestructionStep::ArrayField(FieldId::new(envelope.id, 2)),
            HirDestructionStep::OptionalField {
                field: FieldId::new(envelope.id, 1),
                optional: outer_optional,
            },
            HirDestructionStep::Field(FieldId::new(envelope.id, 0)),
            HirDestructionStep::Base(base.id),
        ]
    );
}

#[test]
fn vector_storage_composes_optional_arrays_with_every_owning_argument_family() {
    let hir = check_generic_source(
        "interface View { fn read() -> i64; }\n\
         class Base { init() {} }\n\
         class Leaf extends Base implements View {\n\
           value: i64;\n\
           init(value: i64) { super(); self.value = value; }\n\
           fn read() -> i64 { return self.value; }\n\
         }\n\
         class Vec<T> {\n\
           storage: T?[];\n\
           static cached: T?;\n\
           init() { self.storage = T?[](); }\n\
           mut fn set(index: i64, value: T) -> unit {\n\
             self.storage[index] = some(value);\n\
           }\n\
           fn get(index: i64) -> T? { return self.storage[index]; }\n\
           fn require(index: i64) -> T {\n\
             var value: T? = self.storage[index];\n\
             return value!;\n\
           }\n\
         }\n\
         fn primitive(ref value: Vec<i64>) -> unit {}\n\
         fn exact(ref value: Vec<Leaf>) -> unit {}\n\
         fn recursive(ref value: Vec<Leaf?>) -> unit {}\n\
         fn exact_owner(ref value: Vec<shared Leaf>) -> unit {}\n\
         fn base_owner(ref value: Vec<shared Base>) -> unit {}\n\
         fn view_owner(ref value: Vec<shared View>) -> unit {}\n\
         fn object_owner(ref value: Vec<shared Obj>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let vectors = hir
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Vec<"))
        .collect::<Vec<_>>();
    assert_eq!(vectors.len(), 7);

    let mut scalar = 0;
    let mut inline_class = 0;
    let mut nested = 0;
    let mut shared_owner = 0;
    for vector in vectors {
        let Type::Array(storage) = vector.fields[0].ty else {
            panic!("Vec storage must close to an array")
        };
        let Type::Optional(element) = hir.array_types.get(storage).unwrap().element else {
            panic!("Vec storage elements must add exactly one optional layer")
        };
        let optional = hir.optional_type(element).unwrap();
        match optional.storage {
            HirOptionalStorageCategory::Scalar => scalar += 1,
            HirOptionalStorageCategory::InlineClass(_) => inline_class += 1,
            HirOptionalStorageCategory::Nested(_) => nested += 1,
            HirOptionalStorageCategory::SharedOwner(_) => shared_owner += 1,
            HirOptionalStorageCategory::InlineArray(_) => {
                panic!("this matrix has no array type argument")
            }
        }
        assert_eq!(vector.static_fields[0].ty, Type::Optional(element));
        assert_eq!(vector.methods[0].parameters[1].ty, optional.payload);
        assert_eq!(vector.methods[1].return_type, Type::Optional(element));
        assert_eq!(vector.methods[2].return_type, optional.payload);
    }
    assert_eq!((scalar, inline_class, nested, shared_owner), (1, 1, 1, 4));

    let dump = dump_hir(&hir);
    for operation in [
        "ArrayElementAssignment",
        "ArrayElementPlace",
        "optional-class",
        "shared?",
        "AggregateOptionalInitialization",
        "ReceiverThenIndex",
        "failure=IndexOutOfBoundsTerminate",
        "anchor=InlineOwner",
    ] {
        assert!(dump.contains(operation), "missing `{operation}`:\n{dump}");
    }
}

#[test]
fn optional_owners_and_shared_optional_boxes_remain_distinct_after_substitution() {
    let hir = check_generic_source(
        "interface View { fn read() -> i64; }\n\
         class Base { init() {} }\n\
         class Leaf extends Base implements View {\n\
           init() { super(); }\n\
           fn read() -> i64 { return 1; }\n\
         }\n\
         class Ownership<T> {\n\
           owner: shared T;\n\
           optional_owner: (shared T)?;\n\
           box: shared T?;\n\
           optional_box_owner: (shared T?)?;\n\
           init(\n\
             owner: shared T,\n\
             optional_owner: (shared T)?,\n\
             box: shared T?,\n\
             optional_box_owner: (shared T?)?\n\
           ) {\n\
             self.owner = owner;\n\
             self.optional_owner = optional_owner;\n\
             self.box = box;\n\
             self.optional_box_owner = optional_box_owner;\n\
           }\n\
         }\n\
         fn exact(ref value: Ownership<Leaf>) -> unit {}\n\
         fn base(ref value: Ownership<Base>) -> unit {}\n\
         fn interface(ref value: Ownership<View>) -> unit {}\n\
         fn object(ref value: Ownership<Obj>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let ownerships = hir
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Ownership<"))
        .collect::<Vec<_>>();
    assert_eq!(ownerships.len(), 4);

    let mut exact_boxes = 0;
    let mut view_boxes = 0;
    for ownership in ownerships {
        let Type::Shared(owner_target) = ownership.fields[0].ty else {
            panic!("shared T must remain a shared owner")
        };
        let Type::Optional(optional_owner) = ownership.fields[1].ty else {
            panic!("(shared T)? must remain an optional owner")
        };
        assert_eq!(
            hir.optional_type(optional_owner).unwrap().storage,
            HirOptionalStorageCategory::SharedOwner(owner_target)
        );

        let Type::Shared(HirSharedTarget::OptionalBox(box_target)) = ownership.fields[2].ty else {
            panic!("shared T? must remain a shared optional box")
        };
        let Type::Optional(optional_box_owner) = ownership.fields[3].ty else {
            panic!("(shared T?)? must retain its outer optional-owner layer")
        };
        assert_eq!(
            hir.optional_type(optional_box_owner).unwrap().storage,
            HirOptionalStorageCategory::SharedOwner(HirSharedTarget::OptionalBox(box_target))
        );

        if hir.optional_box_type(box_target).unwrap().is_exact() {
            exact_boxes += 1;
        } else {
            view_boxes += 1;
        }
        assert_eq!(
            ownership.destruction.steps,
            [
                HirDestructionStep::OptionalSharedField(FieldId::new(ownership.id, 3)),
                HirDestructionStep::SharedField(FieldId::new(ownership.id, 2)),
                HirDestructionStep::OptionalSharedField(FieldId::new(ownership.id, 1)),
                HirDestructionStep::SharedField(FieldId::new(ownership.id, 0)),
            ]
        );
    }
    assert_eq!((exact_boxes, view_boxes), (2, 2));
}

#[test]
fn unused_type_capabilities_are_not_imposed_on_the_whole_specialization() {
    let mut program = resolve_generic_source(
        "class Resource { init() {} }\n\
         class Observer<T> {\n\
           init() {}\n\
           fn inspect(ref value: T) -> unit {}\n\
         }\n\
         class Owner<T> {\n\
           value: shared T;\n\
           init(value: shared T) { self.value = value; }\n\
         }\n\
         fn use(ref observer: Observer<Resource>, ref owner: Owner<Resource>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    program.classes.entries_mut_for_test()[0].copy_constructor = ResolvedCopyOperation::Unavailable;
    program.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let checked = crate::typeck::type_check(&program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let observer = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Observer<"))
        .unwrap();
    let owner = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Owner<"))
        .unwrap();
    assert!(matches!(
        observer.copy_constructor,
        HirCopyCapability::Synthesized(ref copy) if copy.fields.is_empty()
    ));
    assert!(matches!(
        owner.copy_constructor,
        HirCopyCapability::Synthesized(ref copy)
            if matches!(copy.fields.as_slice(), [HirSynthesizedFieldCopy::Shared { .. }])
    ));
}

#[test]
fn specialized_static_defaults_and_explicit_initializers_use_substituted_types() {
    let hir = check_generic_source(
        "class Item { init() {} }
         class Storage<T> {
           static zero_optional: T?;
           static zero_array: T[];
           static explicit_optional: T? = none;
           static explicit_array: T[] = T[]();
           init() {}
         }
         fn main() -> i64 {
           if (Storage<Item>::zero_optional is some) { return 1; }
           if (Storage<Item?>::zero_optional is some) { return 2; }
           if (Storage<shared Item>::zero_optional is some) { return 3; }
           return 0;
         }",
    );
    let specializations = hir
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Storage<"))
        .collect::<Vec<_>>();

    assert_eq!(specializations.len(), 3);
    assert_eq!(
        specializations
            .iter()
            .map(|class| class.name.as_str())
            .collect::<Vec<_>>(),
        ["Storage<Item>", "Storage<Item?>", "Storage<shared Item>"]
    );
    for class in specializations {
        assert_eq!(class.static_fields.len(), 4);
        assert!(matches!(class.static_fields[0].ty, Type::Optional(_)));
        assert!(matches!(class.static_fields[1].ty, Type::Array(_)));
        assert_eq!(class.static_fields[0].ty, class.static_fields[2].ty);
        assert_eq!(class.static_fields[1].ty, class.static_fields[3].ty);
        assert!(class.static_fields[0].initializer.is_none());
        assert!(class.static_fields[1].initializer.is_none());
        assert!(class.static_fields[2].initializer.is_some());
        assert!(class.static_fields[3].initializer.is_some());
    }
}

#[test]
fn zero_default_validation_runs_after_static_type_substitution() {
    let resolved = crate::test_support::resolve_source(
        "class Item { init() {} }
         class Exact<T> { static value: T; init() {} }
         fn use(ref value: Exact<Item>) -> unit {}
         fn main() -> i64 { return 0; }",
    );
    let diagnostic = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == crate::resolve::UNSATISFIED_GENERIC_REQUIREMENT)
        .expect("substituting an exact class must reject zero-default static storage");

    assert!(diagnostic.labels.iter().any(|label| label
        .message
        .contains("zero initialization of static member")));
}

#[test]
fn closed_members_require_copy_operations_only_where_they_are_used() {
    let mut program = resolve_generic_source(
        "class Resource { init() {} }\n\
         class Box<T> {\n\
           value: T;\n\
           init(ref value: T) { self.value = value; }\n\
           mut fn replace(ref value: T) -> unit { self.value = value; }\n\
         }\n\
         fn use(ref value: Box<Resource>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    program.classes.entries_mut_for_test()[0].copy_constructor = ResolvedCopyOperation::Unavailable;
    program.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let checked = crate::typeck::type_check(&program);
    assert!(checked.hir.is_none());
    let copy_failures = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE)
        .collect::<Vec<_>>();
    assert_eq!(copy_failures.len(), 2, "{:?}", checked.diagnostics);
    assert!(copy_failures
        .iter()
        .any(|diagnostic| diagnostic.message.contains("copy construction")));
    assert!(copy_failures
        .iter()
        .any(|diagnostic| diagnostic.message.contains("copy assignment")));
}
