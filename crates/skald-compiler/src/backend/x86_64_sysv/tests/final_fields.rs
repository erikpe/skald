use super::*;
use crate::backend::x86_64_sysv::layout::DataLayout;

#[test]
fn final_markers_do_not_change_class_layout() {
    let mutable = lower_source_to_mir(concat!(
        "class Value {\n",
        "  byte: u8; payload: i64;\n",
        "  init(byte: u8, payload: i64) { self.byte = byte; self.payload = payload; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let final_fields = lower_source_to_mir(concat!(
        "class Value {\n",
        "  final byte: u8; final payload: i64;\n",
        "  init(byte: u8, payload: i64) { self.byte = byte; self.payload = payload; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let mutable_layout = DataLayout::compute(&mutable).unwrap();
    let final_layout = DataLayout::compute(&final_fields).unwrap();
    let class = ClassId::new(0);
    assert_eq!(
        mutable_layout.class(class).unwrap().ty(),
        final_layout.class(class).unwrap().ty()
    );
    for index in 0..2 {
        let field = FieldId::new(class, index);
        assert_eq!(mutable_layout.field(field), final_layout.field(field));
    }
}
