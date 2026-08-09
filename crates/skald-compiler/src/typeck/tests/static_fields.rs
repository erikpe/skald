use super::*;
use crate::{
    hir::{
        HirArrayTransfer, HirCallArgument, HirClassOptionalDestinationInitialization,
        HirExpressionKind, HirObjectDestinationInitialization, HirOptionalSharedSource,
        HirOptionalSource, HirOptionalStorage, HirOwnerTransfer, HirPrimitiveStorage,
        HirStoredValueInitialization,
    },
    identity::{ClassId, MethodId, StaticFieldId, StaticInitializerId},
    resolve::resolve_module_graph,
    test_support::load_module_sources_with_standard_library,
    typeck::INVALID_STATIC_FIELD_TYPE,
};

#[test]
fn lowers_inline_optional_operations_to_static_places() {
    let output = check_text(concat!(
        "fn replace(mut ref value: i64?, next: i64?) -> unit { value = next; }\n",
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Base { static maybe_count: i64?; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "class State {\n",
        "  private static maybe_item: Item?; init() {}\n",
        "  static fn read() -> i64 {\n",
        "    State.maybe_item = Item(1);\n",
        "    if (State.maybe_item is some) { return State.maybe_item!.value; }\n",
        "    return 0;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  replace(Derived.maybe_count, 41);\n",
        "  if (Base.maybe_count is some) { return Base.maybe_count! + State.read(); }\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Call(call) = &main.body.statements[0] else {
        panic!("expected optional alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &call.call.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::OptionalPlace(crate::hir::HirOptionalAliasPlace::Primitive(place)) =
        &arguments[0]
    else {
        panic!("expected primitive optional place argument");
    };
    assert!(matches!(place.storage, HirOptionalStorage::Static(_)));

    let dump = dump_hir(&hir);
    assert!(dump.contains("OptionalStaticPlace c1:static0"), "{dump}");
    assert!(
        dump.contains("ClassOptionalStaticPlace c3:static0"),
        "{dump}"
    );
    assert!(
        dump.contains("ObjectPlace c3:method0:self : class c0 mutable"),
        "{dump}"
    );
}

#[test]
fn lowers_optional_shared_static_sources_destinations_and_unwraps() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class State { static owner: shared? Item; init() {} }\n",
        "fn forward(value: shared? Item) -> shared? Item { return value; }\n",
        "fn main() -> i64 {\n",
        "  if (State.owner is some) { return 1; }\n",
        "  State.owner = new Item(42);\n",
        "  State.owner = forward(State.owner);\n",
        "  return State.owner!->value;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(
        dump.contains("OptionalSharedStaticPlace c1:static0"),
        "{dump}"
    );
    assert!(dump.contains("OptionalSharedProduced"), "{dump}");
}

#[test]
fn lowers_primitive_reads_writes_and_aliases_to_identity_based_places() {
    let output = check_text(concat!(
        "fn observe(ref value: i64) -> i64 { return value; }\n",
        "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
        "class State {\n",
        "  static signed: i64;\n",
        "  static unsigned: u64;\n",
        "  static byte: u8;\n",
        "  static float: f64;\n",
        "  static flag: bool;\n",
        "  init() {}\n",
        "  static fn exercise() -> i64 {\n",
        "    State.signed = observe(State.signed) + 1;\n",
        "    State.unsigned = (u64) State.signed;\n",
        "    State.byte = (u8) State.unsigned;\n",
        "    State.float = (f64) State.byte;\n",
        "    State.flag = State.signed < 10 && !State.flag;\n",
        "    increment(State.signed);\n",
        "    return State.signed;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return State.exercise(); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class
            .static_fields
            .iter()
            .map(|field| field.id)
            .collect::<Vec<_>>(),
        (0..5)
            .map(|index| StaticFieldId::new(class.id, index))
            .collect::<Vec<_>>()
    );

    let method = hir
        .member_definition(MethodId::new(class.id, 0).into())
        .unwrap();
    let static_assignments = method
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::PrimitiveAssignment(assignment) => {
                let HirPrimitiveStorage::Static(place) = assignment.destination.storage else {
                    return None;
                };
                Some(place.field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        static_assignments,
        (0..5)
            .map(|index| StaticFieldId::new(class.id, index))
            .collect::<Vec<_>>()
    );

    let HirStatement::Call(increment) = &method.body.statements[5] else {
        panic!("expected primitive alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &increment.call.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::PrimitivePlace(place) = &arguments[0] else {
        panic!("expected primitive-place alias argument");
    };
    assert_eq!(
        place.storage,
        HirPrimitiveStorage::Static(crate::hir::HirStaticPlace {
            field: StaticFieldId::new(class.id, 0),
            span: place.span,
        })
    );

    let dump = dump_hir(&hir);
    assert!(dump.contains("StaticField c0:static0 \"signed\" : i64"));
    assert!(dump.contains("PrimitiveStaticAssignment c0:static0"));
    assert!(dump.contains("StaticRead c0:static0 : i64"));
    assert!(dump.contains("PrimitivePlaceArgument static c0:static0"));
}

#[test]
fn accepts_the_complete_zero_default_declaration_set() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "class State {\n",
        "  static signed: i64; static unsigned: u64; static byte: u8;\n",
        "  static float: f64; static flag: bool;\n",
        "  static maybe_signed: i64?; static maybe_item: Item?;\n",
        "  static maybe_owner: shared? Item; static items: Item[];\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        output
            .hir
            .unwrap()
            .class(ClassId::new(1))
            .unwrap()
            .static_fields
            .len(),
        9
    );
}

#[test]
fn rejects_each_non_zero_default_type_at_its_declaration() {
    let output = check_text(concat!(
        "interface View { fn read() -> i64; }\n",
        "class Item { init() {} }\n",
        "class Invalid {\n",
        "  static item: Item;\n",
        "  static owner: shared Item;\n",
        "  static owner_array: shared Item[];\n",
        "  static view: View;\n",
        "  static erased: Obj;\n",
        "  static empty: unit;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_STATIC_FIELD_TYPE)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 6, "{:?}", output.diagnostics);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("all-zero live value"))));
}

#[test]
fn retains_typed_primitive_and_exact_object_static_initializers() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "class State {\n",
        "  static count: i64 = 1;\n",
        "  static item: Item = Item();\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let state = hir.class(ClassId::new(1)).unwrap();
    assert!(matches!(
        state.static_fields[0].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Primitive(_)
    ));
    assert!(matches!(
        state.static_fields[1].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(HirObjectDestinationInitialization::Direct { .. })
    ));
}

#[test]
fn static_initializer_hir_dump_is_exact_and_retains_one_typed_evaluation() {
    let output = check_text(concat!(
        "class State { static value: i64 = 1 + produce(); init() {} }\n",
        "fn produce() -> i64 { return 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..126\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f1\n",
            "  Classes\n",
            "    Class c0 module m0 \"State\" @0..60\n",
            "      Fields\n",
            "      StaticFields\n",
            "        StaticField c0:static0 \"value\" : i64 @14..48\n",
            "          DeclarationInitializer c0:static0:initializer destination i64 @32..47\n",
            "            Equal @32..33\n",
            "            PrimitiveInitialization\n",
            "              Binary AddI64 : i64 @34..47\n",
            "                Integer 1 : i64 @34..35\n",
            "                DirectCall f0 : i64 @38..47\n",
            "      Initializers\n",
            "        Initializer c0:init0 @49..58\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Methods\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..60\n",
            "      MemberDefinition c0:init0 @49..58\n",
            "        Locals\n",
            "        Block @56..58\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"produce\" internal @61..94\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "    Declaration f1 module m0 \"main\" internal @95..125\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @61..94\n",
            "      Locals\n",
            "      Block @81..94\n",
            "        Return @83..92\n",
            "          Integer 2 : i64 @90..91\n",
            "    Definition f1 @95..125\n",
            "      Locals\n",
            "      Block @112..125\n",
            "        Return @114..123\n",
            "          Integer 0 : i64 @121..122\n",
        )
    );
}

#[test]
fn selects_the_complete_stored_value_initialization_matrix() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "}\n",
        "fn make_item(value: i64) -> Item { return Item(value); }\n",
        "class State {\n",
        "  static signed: i64 = 1;\n",
        "  static unsigned: u64 = 2u;\n",
        "  static byte: u8 = 3u8;\n",
        "  static float: f64 = 4.0;\n",
        "  static flag: bool = true;\n",
        "  static direct: Item = Item(5);\n",
        "  static called: Item = make_item(6);\n",
        "  static copied: Item = (Item(7));\n",
        "  static maybe_signed: i64? = 8;\n",
        "  static no_signed: i64? = none;\n",
        "  static maybe_item: Item? = Item(9);\n",
        "  static no_item: Item? = none;\n",
        "  static owner: shared Item = new Item(10);\n",
        "  static maybe_owner: shared? Item = new Item(11);\n",
        "  static no_owner: shared? Item = none;\n",
        "  static values: i64[] = i64[]{12, 13};\n",
        "  static items: Item[] = Item[]{Item(14), Item(15)};\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let state = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(state.static_fields.len(), 17);
    assert_eq!(hir.static_initializers().count(), 17);
    assert_eq!(
        hir.static_initializer(StaticInitializerId::from(state.static_fields[5].id))
            .unwrap()
            .id,
        StaticInitializerId::from(state.static_fields[5].id)
    );

    for field in &state.static_fields[..5] {
        assert!(matches!(
            field.initializer.as_ref().unwrap().value,
            HirStoredValueInitialization::Primitive(_)
        ));
    }
    assert!(matches!(
        state.static_fields[5].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(HirObjectDestinationInitialization::Direct { .. })
    ));
    assert!(matches!(
        state.static_fields[6].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(HirObjectDestinationInitialization::Direct { .. })
    ));
    assert!(matches!(
        state.static_fields[7].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(HirObjectDestinationInitialization::Copy { .. })
    ));
    assert!(matches!(
        state.static_fields[8].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalPrimitive {
            source: HirOptionalSource::Present(_),
            ..
        }
    ));
    assert!(matches!(
        state.static_fields[9].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalPrimitive {
            source: HirOptionalSource::Absent { .. },
            ..
        }
    ));
    assert!(matches!(
        state.static_fields[10].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalClass(
            HirClassOptionalDestinationInitialization::Direct { .. }
        )
    ));
    assert!(matches!(
        state.static_fields[11].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalClass(
            HirClassOptionalDestinationInitialization::Absent { .. }
        )
    ));
    assert!(matches!(
        state.static_fields[12].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Shared(ref transfer)
            if transfer.operation == HirOwnerTransfer::Adopt
    ));
    assert!(matches!(
        state.static_fields[13].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalShared(ref value)
            if matches!(value.source, HirOptionalSharedSource::Present(_))
    ));
    assert!(matches!(
        state.static_fields[14].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalShared(ref value)
            if matches!(value.source, HirOptionalSharedSource::Absent { .. })
    ));
    for field in &state.static_fields[15..] {
        assert!(matches!(
            field.initializer.as_ref().unwrap().value,
            HirStoredValueInitialization::Array(ref value)
                if value.operation == HirArrayTransfer::Adopt
        ));
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("DeclarationInitializer").count(), 17, "{dump}");
    assert!(dump.contains("ClassInitialization direct"), "{dump}");
    assert!(dump.contains("ClassInitialization copy"), "{dump}");
    assert!(dump.contains("ArrayInitialization adopt"), "{dump}");
}

#[test]
fn selects_copy_operations_for_named_static_sources() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "}\n",
        "class State {\n",
        "  static owner: shared Item = new Item(1);\n",
        "  static owner_copy: shared Item = State.owner;\n",
        "  static maybe_owner: shared? Item = new Item(2);\n",
        "  static maybe_owner_copy: shared? Item = State.maybe_owner;\n",
        "  static values: i64[] = i64[]{3};\n",
        "  static values_copy: i64[] = State.values;\n",
        "  static maybe_number: i64? = 4;\n",
        "  static maybe_number_copy: i64? = State.maybe_number;\n",
        "  static maybe_item: Item? = Item(5);\n",
        "  static maybe_item_copy: Item? = State.maybe_item;\n",
        "  static item: Item = Item(6);\n",
        "  static item_copy: Item = State.item;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let fields = &hir.class(ClassId::new(1)).unwrap().static_fields;
    assert!(matches!(
        fields[1].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Shared(ref transfer)
            if transfer.operation == HirOwnerTransfer::Copy
    ));
    assert!(matches!(
        fields[3].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalShared(ref value)
            if matches!(value.source, HirOptionalSharedSource::Copy(_))
    ));
    assert!(matches!(
        fields[5].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Array(ref value)
            if matches!(value.operation, HirArrayTransfer::DeepCopy(_))
    ));
    assert!(matches!(
        fields[7].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalPrimitive {
            source: HirOptionalSource::Copy(_),
            ..
        }
    ));
    assert!(matches!(
        fields[9].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::OptionalClass(
            HirClassOptionalDestinationInitialization::Copy { .. }
        )
    ));
    assert!(matches!(
        fields[11].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(crate::hir::HirObjectDestinationInitialization::Copy {
            source: crate::hir::HirObjectSource::Static { .. },
            ..
        })
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("SharedStatic c1:static0"), "{dump}");
    assert!(dump.contains("SharedTransfer Copy"), "{dump}");
    assert!(dump.contains("ArrayInitialization deep-copy"), "{dump}");
    assert!(
        dump.contains("StaticObjectSource c1:static10 : c0"),
        "{dump}"
    );
}

#[test]
fn applies_declaring_class_privacy_and_initializer_overload_selection() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  private init() { self.value = 0; }\n",
        "  private init(value: i64) { self.value = value; }\n",
        "  static own: Item = Item(42);\n",
        "}\n",
        "class Foreign { static denied: Item = Item(); init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == crate::typeck::PRIVATE_INITIALIZER_ACCESS)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn direct_production_does_not_require_copy_but_grouped_production_does() {
    let mut program = resolve_text(concat!(
        "class Item { init() {} }\n",
        "class State {\n",
        "  static direct: Item = Item();\n",
        "  static copied: Item = (Item());\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[0].copy_constructor =
        crate::resolve::ResolvedCopyOperation::Unavailable;
    let output = type_check(&program);

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == crate::typeck::COPY_OPERATION_UNAVAILABLE)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn rejects_non_stored_explicit_types_and_initializer_type_mismatches() {
    let output = check_text(concat!(
        "interface View { fn read() -> i64; }\n",
        "class Item implements View {\n",
        "  init() {}\n",
        "  fn read() -> i64 { return 0; }\n",
        "}\n",
        "fn nothing() -> unit {}\n",
        "class Invalid {\n",
        "  static empty: unit = nothing();\n",
        "  static object_view: Obj = (Obj) Item();\n",
        "  static interface_view: View = (View) Item();\n",
        "  static mismatch: i64 = true;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_STATIC_FIELD_TYPE)
            .count(),
        3,
        "{:?}",
        output.diagnostics
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == crate::typeck::TYPE_MISMATCH)
            .count(),
        1,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn lowers_static_array_operations_aliases_and_replacement() {
    let output = check_text(concat!(
        "fn inspect(ref values: i64[]) -> u64 { return values.len(); }\n",
        "fn change(mut ref values: i64[]) -> unit { values[0] = 40; }\n",
        "class Item { init(value: i64) {} }\n",
        "class State {\n",
        "  static values: i64[]; static items: Item[]; static nested: i64[][];\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  State.values = i64[](2u); change(State.values);\n",
        "  State.values[1] = 2; var copy: i64[] = State.values[:];\n",
        "  return State.values[0] + (i64) inspect(State.values) + copy[1] - 2;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("ArrayPlace static c1:static0"), "{dump}");
    assert!(dump.contains("anchor=InlineBacking"), "{dump}");
    assert!(dump.contains("ArraySlice"), "{dump}");
    assert!(dump.contains("ArrayAliasArgument"), "{dump}");
}

#[test]
fn preserves_source_argument_order_without_a_static_receiver() {
    let output = check_text(concat!(
        "fn middle() -> i64 { return 2; }\n",
        "fn combine(first: i64, second: i64, third: i64) -> i64 {\n",
        "  return first + second + third;\n",
        "}\n",
        "class State { static first: i64; static third: i64; init() {} }\n",
        "fn main() -> i64 { return combine(State.first, middle(), State.third); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let HirExpressionKind::DirectCall { arguments, .. } =
        &returned_expression(hir.definitions.get(hir.entry_function).unwrap()).kind
    else {
        panic!("expected direct call");
    };
    assert!(matches!(
        &arguments[0],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::StaticRead(_),
            ..
        })
    ));
    assert!(matches!(
        &arguments[1],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::DirectCall { .. },
            ..
        })
    ));
    assert!(matches!(
        &arguments[2],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::StaticRead(_),
            ..
        })
    ));
}

#[test]
fn module_qualified_inline_optional_places_retain_declaring_identity() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import state;\n",
                    "fn main() -> i64 {\n",
                    "  state::State.value = 42;\n",
                    "  if (state::State.value is some) { return state::State.value!; }\n",
                    "  return 0;\n",
                    "}\n",
                ),
            ),
            (
                "state.ska",
                "public class State { static value: i64?; init() {} }\n",
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("OptionalStaticPlace c0:static0"), "{dump}");
}

#[test]
fn module_qualified_optional_shared_static_executes_through_declaring_identity() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import state;\n",
                    "fn main() -> i64 {\n",
                    "  state::State.prepare();\n",
                    "  return state::State.owner!->read();\n",
                    "}\n",
                ),
            ),
            (
                "state.ska",
                concat!(
                    "public class Item {\n",
                    "  value: i64; init(value: i64) { self.value = value; }\n",
                    "  fn read() -> i64 { return self.value; }\n",
                    "}\n",
                    "public class State {\n",
                    "  static owner: shared? Item; init() {}\n",
                    "  static fn prepare() -> unit { State.owner = new Item(42); }\n",
                    "}\n",
                ),
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("OptionalSharedStaticPlace"), "{dump}");
}

#[test]
fn module_qualified_inherited_array_static_retains_declaring_identity() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import state;\n",
                    "fn main() -> i64 {\n",
                    "  state::Derived.values = i64[](1u);\n",
                    "  state::Derived.values[0] = 42;\n",
                    "  return state::Base.values[0];\n",
                    "}\n",
                ),
            ),
            (
                "state.ska",
                concat!(
                    "public class Base { static values: i64[]; init() {} }\n",
                    "public class Derived extends Base { init() { super(); } }\n",
                ),
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("ArrayPlace static c0:static0"), "{dump}");
    assert!(!dump.contains("ArrayPlace static c1:static0"), "{dump}");
}

#[test]
fn primitive_statics_compose_with_bit_intrinsics_and_io_scalar_arguments() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "import std::f64; import std::io;\n",
                "class State {\n",
                "  static signed: i64; static unsigned: u64; static byte: u8;\n",
                "  static float: f64; static flag: bool; init() {}\n",
                "}\n",
                "fn main() -> i64 {\n",
                "  State.unsigned = std::f64::to_bits(State.float);\n",
                "  State.float = std::f64::from_bits(State.unsigned);\n",
                "  std::io::println_i64(State.signed); std::io::println_u64(State.unsigned);\n",
                "  std::io::println_u8(State.byte); std::io::println_f64(State.float);\n",
                "  std::io::println_bool(State.flag);\n",
                "  return 0;\n",
                "}\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("bit_reinterpretation f64.u64"));
    assert!(dump.contains("bit_reinterpretation u64.f64"));
    // Seven reads belong to the application. The canonical floating-point
    // conversion tables contribute ten more reads to the complete graph.
    assert_eq!(dump.matches("StaticRead").count(), 17);
}

#[test]
fn primitive_statics_reuse_operator_and_control_flow_semantics() {
    let output = check_text(concat!(
        "class State {\n",
        "  static signed: i64; static unsigned: u64; static byte: u8;\n",
        "  static flag: bool; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  State.signed = (State.signed / 1) % 1;\n",
        "  State.signed = (State.signed & 7) | (State.signed ^ 2);\n",
        "  State.signed = (State.signed << State.unsigned) >> State.unsigned;\n",
        "  State.unsigned = (State.unsigned / 1u) % 1u;\n",
        "  State.byte = (State.byte << State.unsigned) >> State.unsigned;\n",
        "  State.flag = State.flag == true;\n",
        "  if (State.flag && State.signed <= 0) { State.signed = 1; }\n",
        "  while (State.flag || false) { State.flag = false; }\n",
        "  return State.signed;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("CheckedIntegerDivision"));
    assert!(dump.contains("CheckedShift"));
    assert!(dump.contains("Logical"));
    assert!(dump.contains("Comparison"));
}

#[test]
fn statics_compose_with_initializer_overloads_instance_methods_and_lifecycle_bodies() {
    let output = check_text(concat!(
        "class State { static count: i64; init() {} }\n",
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = State.count; }\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return State.count; }\n",
        "  destroy { State.count = State.count + 1; }\n",
        "}\n",
        "fn main() -> i64 { State.count = 42; var item: Item = Item(); return item.read(); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert_eq!(
        dump.matches("PrimitiveStaticAssignment c0:static0").count(),
        2
    );
    assert!(dump.contains("StaticRead c0:static0 : i64"), "{dump}");
}

#[test]
fn optional_string_statics_use_ordinary_literal_and_method_semantics() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::str import Str;\n",
                "class State { static text: Str?; init() {} }\n",
                "fn main() -> i64 {\n",
                "  if (State.text is some) { return 1; }\n",
                "  State.text = \"42\";\n",
                "  return State.text!.to_i64()!;\n",
                "}\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("ClassOptionalStaticPlace"), "{dump}");
    assert!(dump.contains("StringLiteral"), "{dump}");
}

#[test]
fn string_literal_static_initialization_reuses_exact_object_copy_semantics() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::str import Str;\n",
                "class State { static text: Str = \"ready\"; init() {} }\n",
                "fn main() -> i64 { return 0; }\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let state = hir
        .classes
        .iter()
        .find(|class| class.name == "State")
        .unwrap();
    assert!(matches!(
        state.static_fields[0].initializer.as_ref().unwrap().value,
        HirStoredValueInitialization::Class(HirObjectDestinationInitialization::Copy { .. })
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("StringLiteral"), "{dump}");
    assert!(dump.contains("ClassInitialization copy"), "{dump}");
}
