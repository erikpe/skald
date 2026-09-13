use crate::{backend::Target, driver::compile_source_to_assembly};

#[test]
fn indexed_shared_owner_arrays_reach_backend_lowering() {
    let output = compile_source_to_assembly(
        "indexed-shared-owner-array.ska",
        concat!(
            "class Item { init(value: i64) {} }\n",
            "fn main() -> i64 {\n",
            "  var value: shared Item = new Item(1);\n",
            "  var values: (shared Item)[] = (shared Item)[](1u; index => value);\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("indexed shared-owner construction must reach backend lowering");
    assert!(
        output.assembly.contains(".globl main"),
        "{}",
        output.assembly
    );
}

#[test]
fn primitive_inline_array_locals_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "arrays.ska",
        concat!(
            "fn duplicate(values: i64[]) -> i64[] { return values; }\n",
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[](4u);\n",
            "  values[-1] = 7;\n",
            "  var copied: i64[] = duplicate(values);\n",
            "  return copied[3];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive inline local arrays must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("call ska_rt_free"));
    assert!(artifact.assembly.contains("[r11 + r10*8 + 16]"));
}

#[test]
fn primitive_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[]{1, 2};\n",
            "  return values[0] + values[1];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("[r11 + r10*8 + 16]"));
}

#[test]
fn exact_class_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var values: Item[] = Item[]{Item()};\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("exact-class element lists must lower through x86-64");
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Item.c0.init.i0"));
}

#[test]
fn inline_optional_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 {\n",
            "  var scalars: i64?[] = i64?[]{none, 2};\n",
            "  var objects: shared Item?[] = new Item?[]{none, Item(3)};\n",
            "  return scalars[1]!;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("inline optional element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Item.c0.init.i0"));
}

#[test]
fn nested_inline_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "fn main() -> i64 {\n",
            "  var inner: i64[] = i64[]{1, 2};\n",
            "  var values: i64[][] = i64[][]{inner, i64[]{3}};\n",
            "  return values[1][0];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("nested inline-array element lists must lower through x86-64");
    assert!(!artifact.assembly.contains("_clone"));
    assert!(artifact.assembly.contains("_release"));
}

#[test]
fn owner_element_list_families_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 {\n",
            "  var owner: shared Item = new Item(20);\n",
            "  var values: (shared Item)[] = (shared Item)[]{owner, new Item(22)};\n",
            "  var maybe: shared (shared? Item)[] = new (shared? Item)[]{none, owner};\n",
            "  var first: shared Item = values[0];\n",
            "  return first->value + (i64) maybe->len();\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("shared-owner element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("call ska_rt_free"));
}

#[test]
fn verified_primitive_optional_boxes_reach_native_assembly() {
    let artifact = compile_source_to_assembly(
        "optional-box.ska",
        "fn main() -> i64 { var box: shared i64? = new i64?(1); return 0; }",
        Target::X86_64SysV,
    )
    .expect("verified primitive optional boxes must lower through the backend");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rdi, 32\n    lea r11"));
    assert!(artifact.assembly.contains(".Lska_optional_box_0_metadata"));
}
