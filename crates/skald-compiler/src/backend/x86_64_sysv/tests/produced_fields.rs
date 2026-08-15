use super::*;

const PRODUCED_FIELD_SOURCE: &str = concat!(
    "class Leaf {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "}\n",
    "class Holder {\n",
    "  leaf: Leaf;\n",
    "  init(value: i64) { self.leaf = Leaf(value); }\n",
    "}\n",
    "fn inspect(ref leaf: Leaf) -> i64 { return leaf.value; }\n",
    "fn main() -> i64 { return inspect(Holder(42).leaf); }\n",
);

#[test]
fn produced_fields_keep_ordinary_layout_runtime_surface_and_abi_marker() {
    let program = lower_source_to_final_mir(PRODUCED_FIELD_SOURCE);
    let layouts = super::super::layout::DataLayout::compute(&program).unwrap();
    let holder = program
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .expect("fixture Holder class must exist");
    assert_eq!(layouts.ty(MirType::Class(holder.id)).unwrap().size(), 8);

    let first = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let second = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("call ska_rt_"))
            .collect::<Vec<_>>(),
        ["call ska_rt_abi_v9"]
    );
    assert!(!first.contains("produced_field"));
    assert!(!first.contains("produced.field"));
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(42));

    let runtime_header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)"));
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
}
