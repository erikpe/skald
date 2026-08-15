use super::*;
use crate::{
    hir::{
        dump_hir, HirArrayConstructionMode, HirArrayCopyElement, HirArrayDestroyElement,
        HirCopyCapability, HirExpressionKind, HirLocalInitializer, HirSharedTarget,
        HirStoredValueInitialization, Type,
    },
    identity::ClassId,
    resolve::ResolvedCopyOperation,
    typeck::{
        capabilities::CopyCapabilities, ARRAY_CAPABILITY_UNAVAILABLE, ARRAY_LENGTH_OUT_OF_RANGE,
        COPY_OPERATION_UNAVAILABLE, INVALID_ARRAY_ELEMENT, INVALID_EXTERNAL_DECLARATION,
        INVALID_INTERFACE_REQUIREMENT, PRIVATE_INITIALIZER_ACCESS, TYPE_MISMATCH,
    },
};

#[test]
fn selects_ordered_primitive_element_plans_and_lowers_them_to_mir() {
    let output = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{1, 2};\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(hir.entry_function).unwrap();
    let crate::hir::HirStatement::Local(local) = &definition.body.statements[0] else {
        panic!("expected array local");
    };
    let HirLocalInitializer::Array(initialization) = &local.initializer else {
        panic!("expected owning array initialization");
    };
    let crate::hir::HirArrayReceiverSource::Inline(expression) =
        &initialization.source.receiver.source
    else {
        panic!("expected inline array source");
    };
    let HirExpressionKind::ArrayConstruction(construction) = &expression.kind else {
        panic!("expected element-list construction");
    };
    let HirArrayConstructionMode::Elements(list) = &construction.mode else {
        panic!("expected typed element list");
    };
    assert_eq!(list.elements.len(), 2);
    assert_eq!(list.comma_spans.len(), 1);
    assert!(list
        .elements
        .iter()
        .all(|element| matches!(element.value, HirStoredValueInitialization::Scalar(_))));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("primitive element-list MIR must verify");
    let lowered = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        lowered
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(
                instruction,
                crate::mir::MirInstruction::Array(
                    crate::mir::MirArrayInstruction::InitializeElement { .. }
                )
            ))
            .count(),
        2
    );
}

#[test]
fn selects_destination_plans_for_every_stored_element_family() {
    let output = check_text(concat!(
        "class Item { init(value: i64) {} }\n",
        "fn make_item() -> Item { return Item(2); }\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(1);\n",
        "  var nested_source: i64[] = i64[]();\n",
        "  var owner: shared Item = new Item(3);\n",
        "  var maybe_owner: shared? Item = owner;\n",
        "  var primitive: i64[] = i64[]{1, 2};\n",
        "  var unsigned: u64[] = u64[]{3u};\n",
        "  var bytes: u8[] = u8[]{4u8};\n",
        "  var floating: f64[] = f64[]{5.0};\n",
        "  var booleans: bool[] = bool[]{true};\n",
        "  var optional: i64?[] = i64?[]{none, 3};\n",
        "  var objects: Item[] = Item[]{Item(4), make_item(), item};\n",
        "  var optional_objects: Item?[] = Item?[]{none, Item(5), item};\n",
        "  var nested: i64[][] = i64[][]{nested_source, i64[]{6}};\n",
        "  var owners: (shared Item)[] = (shared Item)[]{owner, new Item(7)};\n",
        "  var maybe_owners: (shared? Item)[] = (shared? Item)[]{none, maybe_owner, new Item(8)};\n",
        "  var shared_outer: shared i64[] = new i64[]{9};\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);

    for selected in [
        "PrimitiveInitialization",
        "OptionalPrimitiveInitialization i64?",
        "ClassInitialization direct",
        "ClassInitialization copy",
        "ClassOptionalInitialization class c0? absent",
        "ClassOptionalInitialization class c0? direct",
        "ClassOptionalInitialization class c0? copy",
        "ArrayInitialization deep-copy primitive",
        "ArrayInitialization adopt",
        "SharedTransfer Copy -> shared class c0",
        "SharedTransfer Adopt -> shared class c0",
        "OptionalSharedInitialization shared? class c0",
        "ArrayAllocation shared",
    ] {
        assert!(dump.contains(selected), "missing `{selected}`:\n{dump}");
    }
    assert_eq!(dump, dump_hir(&hir));
}

#[test]
fn direct_class_elements_require_neither_default_nor_copy_but_materialized_sources_do() {
    let source = concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 {\n",
        "  var values: Item[] = Item[]{Item(1)};\n",
        "  return 0;\n",
        "}\n",
    );
    let mut resolved = resolve_text(source);
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    let output = crate::typeck::type_check(&resolved);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let array = hir.array_types.iter().next().unwrap();
    assert!(array.lifecycle.default.is_none());
    assert!(array.lifecycle.copy.is_none());
    assert!(dump_hir(&hir).contains("ClassInitialization direct"));

    let grouped = concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 {\n",
        "  var values: Item[] = Item[]{(Item(1))};\n",
        "  return 0;\n",
        "}\n",
    );
    let mut resolved = resolve_text(grouped);
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    let output = crate::typeck::type_check(&resolved);
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE));
}

#[test]
fn direct_optional_payloads_require_no_copy_but_materialized_payloads_do() {
    let direct = concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 {\n",
        "  var values: Item?[] = Item?[]{none, Item(1)};\n",
        "  return 0;\n",
        "}\n",
    );
    let mut resolved = resolve_text(direct);
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    let output = crate::typeck::type_check(&resolved);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(dump_hir(output.hir.as_ref().unwrap())
        .contains("ClassOptionalInitialization class c0? direct"));

    let grouped = concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 {\n",
        "  var values: Item?[] = Item?[]{(Item(1))};\n",
        "  return 0;\n",
        "}\n",
    );
    let mut resolved = resolve_text(grouped);
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    let output = crate::typeck::type_check(&resolved);
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE));
}

#[test]
fn diagnoses_each_invalid_element_and_continues_in_source_order() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{true, 1u, false};\n",
        "  return 0;\n",
        "}\n",
    );
    let output = check_text(source);
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == TYPE_MISMATCH)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3, "{:?}", output.diagnostics);
    let starts = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.labels[0].span.range().start())
        .collect::<Vec<_>>();
    assert!(starts.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(starts[0], source.find("true").unwrap());
    assert_eq!(starts[1], source.find("1u").unwrap());
    assert_eq!(starts[2], source.find("false").unwrap());
    assert!(output.hir.is_none());
}

#[test]
fn enforces_initializer_access_at_the_exact_list_element() {
    let source = concat!(
        "class Secret { private init() {} }\n",
        "fn main() -> i64 {\n",
        "  var values: Secret[] = Secret[]{Secret()};\n",
        "  return 0;\n",
        "}\n",
    );
    let output = check_text(source);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
        .expect("private list initializer must be rejected");
    assert_eq!(
        diagnostic.labels[0].span.range().start(),
        source.rfind("Secret()").unwrap()
    );
}

#[test]
fn records_exact_lifecycle_plans_for_supported_element_categories() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var primitive: i64[] = i64[](2u);\n",
        "  var optional: i64?[] = i64?[](2u);\n",
        "  var object: Item[] = Item[](2u);\n",
        "  var nested: i64[][] = i64[][](2u);\n",
        "  var owners: (shared Item)[] = (shared Item)[](2u);\n",
        "  var maybe_owners: (shared? Item)[] = (shared? Item)[](2u);\n",
        "  var shared_array: shared i64[] = new i64[](2u);\n",
        "  var maybe_array: shared? i64[] = new i64[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(
        output.diagnostics.is_empty(),
        "supported arrays must type-check: {:?}",
        output.diagnostics
    );
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);

    assert!(dump.contains("Default primitive-zero"));
    assert!(dump.contains("Default optional-absent"));
    assert!(dump.contains("Default class c0 via c0:init0"));
    assert!(dump.contains("Default empty-array a0"));
    assert!(dump.contains("Default shared-class c0 via c0:init0"));
    assert!(dump.contains("Destruction shared? class c0"));
    assert!(dump.contains("ArrayAllocation shared a0"));
    assert_eq!(dump, dump_hir(&hir));
}

#[test]
fn distinguishes_named_deep_copy_from_produced_backing_adoption() {
    let output = check_text(concat!(
        "fn pass(value: i64[]) -> unit { return; }\n",
        "fn make() -> i64[] { return i64[](3u); }\n",
        "fn main() -> i64 {\n",
        "  var source: i64[] = i64[](3u);\n",
        "  var named: i64[] = source;\n",
        "  var produced: i64[] = i64[](copy source);\n",
        "  pass(source);\n",
        "  pass(make());\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(output.hir.as_ref().unwrap());

    assert!(dump.contains("ArrayInitialization deep-copy primitive"));
    assert!(dump.contains("ArrayInitialization adopt"));
    assert!(dump.contains("ArraySource named a0"));
    assert!(dump.contains("ArraySource produced a0"));
    assert!(dump.contains("ArrayArgument"));
}

#[test]
fn recursive_class_array_edges_terminate_and_remain_copyable() {
    let resolved = resolve_text(concat!(
        "class Node {\n",
        "  children: Node[];\n",
        "  init() { self.children = Node[](); }\n",
        "}\n",
        "fn main() -> i64 { var nodes: Node[] = Node[](2u); return 0; }\n",
    ));
    let capabilities = CopyCapabilities::compute(&resolved);

    assert!(matches!(
        capabilities.constructor(ClassId::new(0)),
        HirCopyCapability::Synthesized(_)
    ));
    assert!(matches!(
        capabilities
            .array(crate::identity::ArrayTypeId::new(0))
            .lifecycle
            .copy,
        Some(HirArrayCopyElement::Class { .. })
    ));
}

#[test]
fn unavailable_class_operations_propagate_through_nested_array_capabilities() {
    let mut resolved = resolve_text(concat!(
        "class Item { init() {} }\n",
        "class Box { values: Item[][]; init() { self.values = Item[][](); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        ResolvedCopyOperation::Unavailable;
    resolved.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;
    let capabilities = CopyCapabilities::compute(&resolved);

    assert_eq!(
        capabilities.constructor(ClassId::new(1)),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.assignment(ClassId::new(1)),
        &HirCopyCapability::Unavailable
    );
    assert!(capabilities
        .array(crate::identity::ArrayTypeId::new(0))
        .lifecycle
        .copy
        .is_none());
    assert!(capabilities
        .array(crate::identity::ArrayTypeId::new(1))
        .lifecycle
        .copy
        .is_none());
    assert!(capabilities
        .array(crate::identity::ArrayTypeId::new(1))
        .lifecycle
        .assignment
        .is_none());
}

#[test]
fn empty_arrays_do_not_require_default_or_copy_capabilities() {
    let output = check_text(concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 { var values: Item[] = Item[](); return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let array = hir.array_types.iter().next().unwrap();

    assert_eq!(array.lifecycle.default, None);
    assert_eq!(
        array.lifecycle.destruction,
        HirArrayDestroyElement::Class(ClassId::new(0))
    );
}

#[test]
fn rejects_unavailable_defaults_lengths_and_ownership_conversions() {
    let unavailable = check_text(concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 { var values: Item[] = Item[](2u); return 0; }\n",
    ));
    assert!(unavailable
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ARRAY_CAPABILITY_UNAVAILABLE));

    let wrong_length = check_text("fn main() -> i64 { var values: i64[] = i64[](2); return 0; }");
    assert!(wrong_length
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));

    let too_large = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](9223372036854775808u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(too_large
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ARRAY_LENGTH_OUT_OF_RANGE));

    let conversion = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var inline: i64[] = new i64[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(conversion
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));

    let abstract_shared = check_text(concat!(
        "interface Item {}\n",
        "fn main() -> i64 {\n",
        "  var values: (shared Item)[] = (shared Item)[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(abstract_shared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ARRAY_CAPABILITY_UNAVAILABLE));

    let object_shared = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: (shared Obj)[] = (shared Obj)[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(object_shared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ARRAY_CAPABILITY_UNAVAILABLE));

    let missing_shared_default = check_text(concat!(
        "class Item { init(value: i64) {} }\n",
        "fn main() -> i64 {\n",
        "  var values: (shared Item)[] = (shared Item)[](2u);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(missing_shared_default
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == ARRAY_CAPABILITY_UNAVAILABLE));
}

#[test]
fn rejects_non_storable_elements_and_array_contract_boundaries() {
    for element in ["unit", "Obj", "Readable"] {
        let declaration = if element == "Readable" {
            "interface Readable { fn read() -> i64; }\n"
        } else {
            ""
        };
        let source = format!(
            "{declaration}fn main() -> i64 {{ var values: {element}[] = {element}[](); return 0; }}"
        );
        let output = check_text(&source);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_ARRAY_ELEMENT),
            "expected invalid element diagnostic for {element}: {:?}",
            output.diagnostics
        );
    }

    let external = check_text(concat!(
        "extern fn consume(values: i64[]) -> i64;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        external
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION)
            .count(),
        1
    );

    let interface = check_text(concat!(
        "interface Source { fn values() -> i64[]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(interface
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INTERFACE_REQUIREMENT));
}

#[test]
fn declarations_and_shared_targets_retain_exact_array_identity() {
    let output = check_text(concat!(
        "class Holder { values: i64[]; init() { self.values = i64[](); } }\n",
        "fn consume(values: i64[], owner: shared i64[], maybe: shared? i64[]) -> i64 {\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let consume = hir
        .declarations
        .iter()
        .find(|item| item.name == "consume")
        .unwrap();

    assert_eq!(
        consume.parameters[0].ty,
        Type::Array(crate::identity::ArrayTypeId::new(0))
    );
    assert_eq!(
        consume.parameters[1].ty,
        Type::Shared(HirSharedTarget::Array(crate::identity::ArrayTypeId::new(0)))
    );
    let Type::Optional(optional) = consume.parameters[2].ty else {
        panic!("expected optional")
    };
    assert_eq!(
        hir.optional_type(optional).unwrap().payload,
        Type::Shared(HirSharedTarget::Array(crate::identity::ArrayTypeId::new(0)))
    );
}

#[test]
fn exact_array_identity_participates_in_overload_selection_and_invariance() {
    let selected = check_text(concat!(
        "class Pick {\n",
        "  init(values: i64[]) {}\n",
        "  init(values: u64[]) {}\n",
        "}\n",
        "fn main() -> i64 { var pick: Pick = Pick(i64[]()); return 0; }\n",
    ));
    assert!(
        selected.diagnostics.is_empty(),
        "{:?}",
        selected.diagnostics
    );
    assert!(dump_hir(&selected.hir.unwrap()).contains("Construct c0 via c0:init0"));

    let mismatch = check_text(concat!(
        "fn consume(values: i64[]) -> unit { return; }\n",
        "fn main() -> i64 { consume(u64[]()); return 0; }\n",
    ));
    assert!(mismatch
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
}

#[test]
fn typed_arrays_lower_to_verified_target_independent_mir() {
    let output = check_text("fn main() -> i64 { var values: i64[] = i64[](); return 0; }");
    let mir = crate::mir::lower_hir(output.hir.as_ref().unwrap());
    crate::mir::verify_mir(&mir).expect("array MIR must verify before target lowering");
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("ArrayTypes"));
    assert!(dump.contains("array-allocate"));
    assert!(dump.contains("array-loop"));
    assert!(dump.contains("array-publish"));
    assert!(dump.contains("array-adopt"));
}

#[test]
fn array_hir_dump_is_exact_and_identity_based() {
    let output = check_text("fn main() -> i64 { var values: i64[] = i64[](); return 0; }");
    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..59\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f0\n",
            "  ArrayTypes\n",
            "    ArrayType a0 element i64\n",
            "      Default primitive-zero\n",
            "      Copy primitive\n",
            "      Assignment primitive\n",
            "      Destruction trivial\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"main\" internal @0..59\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @0..59\n",
            "      Locals\n",
            "        Local f0:l0 \"values\" : array a0 @19..47\n",
            "      Block @17..59\n",
            "        LocalDeclaration f0:l0 @19..47\n",
            "          ArrayInitialization adopt @39..46\n",
            "            ArraySource produced a0 @39..46\n",
            "              ArrayReceiver a0 Inline access=ReadOnly anchor=InlineBacking @39..46\n",
            "                ArrayConstruction : array a0 @39..46\n",
            "                  ArrayAllocation inline a0 @39..46\n",
            "                    Empty\n",
            "        Return @48..57\n",
            "          Integer 0 : i64 @55..56\n",
        )
    );
}

#[test]
fn types_length_indices_slices_and_distinct_assignment_kinds() {
    let output = check_text(concat!(
        "fn length_after_updates() -> u64 {\n",
        "  var a: i64[] = i64[](10u);\n",
        "  var b: i64[] = i64[](20u);\n",
        "  a[-1] = 7;\n",
        "  b[5:15] = a;\n",
        "  b[10:15] = a[2:7];\n",
        "  a = b;\n",
        "  return a.len();\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(output.hir.as_ref().unwrap());
    assert!(dump.contains("ArrayLength : u64"));
    assert!(dump.contains("normalization=SignedFromEndOnce"));
    assert!(dump.contains("failure=SliceLengthMismatchTerminate"));
    assert!(dump.contains("CopiedArraySlice : array a0"));
    assert!(dump.contains("ArrayReplacement DestinationThenSourceThenReplace"));
}

#[test]
fn array_operation_families_lower_to_explicit_verified_mir() {
    let output = check_text(concat!(
        "fn update() -> u64 {\n",
        "  var a: i64[] = i64[](10u);\n",
        "  var b: i64[] = i64[](20u);\n",
        "  a[-1] = 7;\n",
        "  b[5:15] = a;\n",
        "  b[10:15] = a[2:7];\n",
        "  a = b;\n",
        "  return a.len();\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let mir = crate::mir::lower_hir(output.hir.as_ref().unwrap());
    crate::mir::verify_mir(&mir).expect("all array operation families must verify");
    let dump = crate::mir::dump_mir(&mir);
    for operation in [
        "array-loop",
        "array-position-check",
        "array-slice",
        "array-replace",
        "array-len",
    ] {
        assert!(dump.contains(operation), "missing {operation}:\n{dump}");
    }
}

#[test]
fn types_shared_and_optional_shared_projection_with_owner_anchors() {
    let output = check_text(concat!(
        "fn shared_length() -> u64 {\n",
        "  var owner: shared i64[] = new i64[](4u);\n",
        "  var maybe: shared? i64[] = new i64[](4u);\n",
        "  owner->[-1] = 3;\n",
        "  maybe!->[0] = owner->[0];\n",
        "  return maybe!->len();\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.as_ref().unwrap();
    let dump = dump_hir(hir);
    assert!(dump.contains("anchor=StableSharedOwner"));
    assert!(dump.contains("anchor=SecuredOptionalSharedOwner"));
    assert!(dump.contains("ExplicitSharedPointee"));

    let mir = crate::mir::lower_hir(hir);
    crate::mir::verify_mir(&mir).expect("shared array owners and projections must verify");
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("PublishShared"));
    assert!(mir_dump.contains("shared-anchor"));
}

#[test]
fn array_aliases_propagate_access_and_admit_exact_elements() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn read(ref values: i64[]) -> u64 { return values.len(); }\n",
        "fn clear(mut ref values: i64[]) -> unit { values[:] = i64[](); return; }\n",
        "fn increment(mut ref value: i64) -> unit { value = value + 1; return; }\n",
        "fn touch(mut ref item: Item) -> unit { item.value = 1; return; }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](2u);\n",
        "  var items: Item[] = Item[](1u);\n",
        "  touch(items[0]);\n",
        "  increment((values[0]));\n",
        "  clear(values);\n",
        "  var length: u64 = read(values);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("ArrayAliasArgument : class c0 access=Mutable"));
    assert!(dump.contains("ArrayAliasArgument : array a0 access=Mutable"));
    assert!(dump.contains("ArrayAliasArgument : array a0 access=ReadOnly"));
    assert!(dump.contains("anchor=InlineBacking"));
}

#[test]
fn array_values_and_aliases_lower_across_internal_calls_and_results() {
    let output = check_text(concat!(
        "fn duplicate(values: i64[]) -> i64[] { return values; }\n",
        "fn length(ref values: i64[]) -> u64 { return values.len(); }\n",
        "fn exercise() -> u64 {\n",
        "  var source: i64[] = i64[](3u);\n",
        "  var copied: i64[] = duplicate(source);\n",
        "  return length(copied);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let mir = crate::mir::lower_hir(&output.hir.unwrap());
    crate::mir::verify_mir(&mir).expect("array calls, results, and aliases must verify");
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("array-return"));
    assert!(dump.contains("owned("));
    assert!(dump.contains("ref-parameter"));
}

#[test]
fn rejects_wrong_index_bound_access_and_alias_root_rebinding() {
    let wrong_index = check_text("fn main() -> i64 { var a: i64[] = i64[](1u); return a[0u]; }");
    assert!(wrong_index
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));

    let wrong_bound = check_text(
        "fn main() -> i64 { var a: i64[] = i64[](1u); var b: i64[] = a[0u:]; return 0; }",
    );
    assert!(wrong_bound
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));

    let read_only = check_text(concat!(
        "fn mutate(ref values: i64[]) -> unit { values[0] = 1; return; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(read_only
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::READ_ONLY_RECEIVER));

    let rebind = check_text(concat!(
        "fn replace(mut ref values: i64[]) -> unit { values = i64[](); return; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(rebind
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_ALIAS_ARGUMENT));

    let slice_view = check_text(concat!(
        "fn read(ref values: i64[]) -> u64 { return values.len(); }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](1u);\n",
        "  var length: u64 = read(values[:]);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(slice_view
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_ALIAS_ARGUMENT));

    let structural_index = crate::test_support::resolve_source(concat!(
        "class Sequence { init() {} fn len() -> u64 { return 1u; } }\n",
        "fn main() -> i64 {\n",
        "  var values: Sequence = Sequence();\n",
        "  return values[0];\n",
        "}\n",
    ));
    assert!(structural_index
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::resolve::INVALID_INDEX_PROTOCOL));

    let raw_shared = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: shared i64[] = new i64[](1u);\n",
        "  return values[0];\n",
        "}\n",
    ));
    assert!(raw_shared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::IMPLICIT_SHARED_DEREFERENCE));

    let optional_without_unwrap = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: shared? i64[] = new i64[](1u);\n",
        "  return values->[0];\n",
        "}\n",
    ));
    assert!(optional_without_unwrap
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_SHARED_CONVERSION));
}

#[test]
fn types_nested_shared_edges_and_optional_element_lifecycle() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn inspect() -> i64 {\n",
        "  var nested: i64[][] = i64[][](2u);\n",
        "  nested[0] = i64[](3u);\n",
        "  var owners: (shared i64[])[] = (shared i64[])[](2u);\n",
        "  owners[0]->[-1] = 4;\n",
        "  var maybe_owners: (shared? i64[])[] = (shared? i64[])[](2u);\n",
        "  maybe_owners[0] = new i64[](1u);\n",
        "  maybe_owners[0]!->[0] = owners[0]->[0];\n",
        "  var optional_items: Item?[] = Item?[](1u);\n",
        "  optional_items[0] = Item();\n",
        "  var items: Item[] = Item[](1u);\n",
        "  items[0].value = 7;\n",
        "  var read_back: i64 = items[0].value;\n",
        "  var called: i64 = items[0].read();\n",
        "  return nested[0][0];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.as_ref().unwrap();
    let dump = dump_hir(hir);
    assert!(dump.contains("Assignment array a0"));
    assert!(dump.contains("SharedArrayElement"));
    assert!(dump.contains("OptionalSharedArrayElementPlace"));
    assert!(dump.contains("construct-via"));
    assert!(dump.contains("assign-via"));

    let mir = crate::mir::lower_hir(hir);
    crate::mir::verify_mir(&mir).expect("nested and nontrivial array element MIR must verify");
}
