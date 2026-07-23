use super::*;
use crate::backend::x86_64_sysv::layout::DataLayout;

const INHERITED_LAYOUT_SOURCE: &str = concat!(
    "class Empty { init() {} }\n",
    "class Padded {\n",
    "  tag: u8;\n",
    "  payload: i64;\n",
    "  init(payload: i64) { self.tag = 1u8; self.payload = payload; }\n",
    "}\n",
    "class Derived extends Padded {\n",
    "  tail: u8;\n",
    "  init(payload: i64) { super(payload); self.tail = 2u8; }\n",
    "}\n",
    "class Deep extends Derived { init(payload: i64) { super(payload); } }\n",
    "class AfterEmpty extends Empty {\n",
    "  value: i64;\n",
    "  init(value: i64) { super(); self.value = value; }\n",
    "}\n",
    "fn main() -> i64 { var value: Deep = Deep(7); return value.payload; }\n",
);

#[test]
fn lays_out_empty_padded_and_deep_base_chains() {
    let program = lower_source_to_mir(INHERITED_LAYOUT_SOURCE);
    let layout = DataLayout::compute(&program).unwrap();

    assert_eq!(layout.class(ClassId::new(0)).unwrap().ty().size(), 1);

    let padded = layout.class(ClassId::new(1)).unwrap();
    assert_eq!(
        padded
            .field(FieldId::new(ClassId::new(1), 0))
            .unwrap()
            .offset,
        0
    );
    assert_eq!(
        padded
            .field(FieldId::new(ClassId::new(1), 1))
            .unwrap()
            .offset,
        8
    );
    assert_eq!(padded.ty().size(), 16);
    assert_eq!(padded.ty().alignment(), 8);

    let derived = layout.class(ClassId::new(2)).unwrap();
    assert_eq!(derived.base().unwrap().class, ClassId::new(1));
    assert_eq!(derived.base().unwrap().offset, 0);
    assert_eq!(
        derived
            .field(FieldId::new(ClassId::new(2), 0))
            .unwrap()
            .offset,
        16
    );
    assert_eq!(derived.ty().size(), 24);

    let deep = layout.class(ClassId::new(3)).unwrap();
    assert_eq!(deep.base().unwrap().class, ClassId::new(2));
    assert_eq!(deep.ty(), derived.ty());

    let after_empty = layout.class(ClassId::new(4)).unwrap();
    assert_eq!(after_empty.base().unwrap().class, ClassId::new(0));
    assert_eq!(
        after_empty
            .field(FieldId::new(ClassId::new(4), 0))
            .unwrap()
            .offset,
        8
    );
    assert_eq!(after_empty.ty().size(), 16);
}

#[test]
fn lowers_deep_base_places_through_checked_target_offsets() {
    let program = lower_source_to_mir(INHERITED_LAYOUT_SOURCE);
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("call .Lska_class_1_init_0"));
    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(7));
}

#[test]
fn corrupt_base_metadata_is_rejected_at_the_backend_trust_boundary() {
    let mut program = lower_source_to_mir(INHERITED_LAYOUT_SOURCE);
    program.classes.entries_mut_for_test()[3]
        .direct_base
        .as_mut()
        .unwrap()
        .class = ClassId::new(1);

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();

    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("invalid direct-base step"));
}
