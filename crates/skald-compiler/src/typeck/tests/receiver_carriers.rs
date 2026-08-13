use super::*;
use crate::{
    hir::{HirObjectReceiver, HirViewSource},
    identity::FunctionId,
    mir::{dump_mir, lower_hir, verify_mir},
};

const RECEIVER_CARRIERS: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "}\n",
    "class Holder {\n",
    "  item: shared Item;\n",
    "  init() { self.item = new Item(2); }\n",
    "}\n",
    "fn stable(ref value: Item) -> i64 { return value.read(); }\n",
    "fn shared_member(ref holder: Holder) -> i64 { return holder.item->read(); }\n",
    "fn optional_member(value: Item?) -> i64 { return value!.read(); }\n",
    "fn array_member(ref values: Item[]) -> i64 { return values[0].read(); }\n",
    "fn checked_member(ref value: Obj) -> i64 { return ((Item) value).read(); }\n",
    "fn shared_field(ref holder: Holder) -> i64 { return holder.item->value; }\n",
    "fn optional_field(value: Item?) -> i64 { return value!.value; }\n",
    "fn main() -> i64 { return 0; }\n",
);

fn returned_receiver(hir: &crate::hir::HirProgram, function: usize) -> &HirObjectReceiver {
    let expression = returned_expression(hir.definitions.get(FunctionId::new(function)).unwrap());
    let HirExpressionKind::MethodCall { receiver, .. } = &expression.kind else {
        panic!("expected method call receiver");
    };
    receiver
}

#[test]
fn member_receivers_use_one_exhaustive_provenance_carrier() {
    let output = check_text(RECEIVER_CARRIERS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.expect("receiver carrier source must type check");

    assert!(matches!(
        returned_receiver(&hir, 0),
        HirObjectReceiver::Place { .. }
    ));
    assert!(matches!(
        returned_receiver(&hir, 1),
        HirObjectReceiver::View { view, .. }
            if matches!(view.source, HirViewSource::AnchoredShared { .. })
    ));
    assert!(matches!(
        returned_receiver(&hir, 2),
        HirObjectReceiver::View { view, .. }
            if matches!(view.source, HirViewSource::OptionalPayload { .. })
    ));
    assert!(matches!(
        returned_receiver(&hir, 3),
        HirObjectReceiver::ArrayElement { .. }
    ));
    assert!(matches!(
        returned_receiver(&hir, 4),
        HirObjectReceiver::Checked { .. }
    ));

    for (function, expected_source) in [
        (5, "shared-backed field receiver"),
        (6, "optional-backed field receiver"),
    ] {
        let expression =
            returned_expression(hir.definitions.get(FunctionId::new(function)).unwrap());
        let HirExpressionKind::FieldRead(field) = &expression.kind else {
            panic!("expected {expected_source}");
        };
        assert!(matches!(field.receiver, HirObjectReceiver::View { .. }));
    }

    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("SharedMethodReceiver"), "{hir_dump}");
    assert!(hir_dump.contains("OptionalMethodReceiver"), "{hir_dump}");

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("all normalized receiver carriers must lower and verify");

    let repeated = check_text(RECEIVER_CARRIERS);
    assert!(
        repeated.diagnostics.is_empty(),
        "{:?}",
        repeated.diagnostics
    );
    let repeated_hir = repeated
        .hir
        .expect("repeated receiver carrier source must type check");
    assert_eq!(dump_hir(&repeated_hir), hir_dump);
    assert_eq!(dump_mir(&lower_hir(&repeated_hir)), dump_mir(&mir));
}
