//! Static-effect coverage for ordinary calls selected by structural brackets.

use crate::{
    mir::StaticEffectNode,
    test_support::{lower_hir_to_final_mir, type_check_source},
};

use super::super::{dump_static_effects, infer_static_effects, StaticEffectEdgeKind};

const STATIC_BRACKET_SOURCE: &str = concat!(
    "class Cell {\n",
    "  static seed: i64 = 4;\n",
    "  static source: Cell = Cell(5);\n",
    "  static result: i64 = Cell.source[0];\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn index_get(key: i64) -> i64 { return self.value + Cell.seed; }\n",
    "}\n",
    "fn main() -> i64 { return Cell.result; }\n",
);

#[test]
fn structural_static_receivers_and_selected_methods_contribute_ordinary_effects() {
    let checked = type_check_source(STATIC_BRACKET_SOURCE);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.expect("valid static bracket source has HIR");
    let preliminary = crate::mir::lower_preliminary_hir(&hir);
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 3);
    let [seed, source, result] = fields.as_slice() else {
        unreachable!()
    };
    let initializer = preliminary
        .static_initializers()
        .find(|initializer| initializer.field == *result)
        .expect("structural result static has an initializer");

    let analysis = infer_static_effects(&preliminary);
    let summary = analysis
        .summary(StaticEffectNode::Callable(initializer.callable()))
        .expect("structural result initializer has an effect summary");
    assert!(summary
        .direct_effects
        .iter()
        .any(|effect| effect.field == *source));
    let seed_effect = summary
        .effects
        .iter()
        .find(|effect| effect.field == *seed)
        .expect("selected structural getter effect must propagate");
    assert!(seed_effect
        .witness
        .iter()
        .any(|edge| edge.kind == StaticEffectEdgeKind::DirectMethodCall));

    let dump = dump_static_effects(&analysis);
    assert_eq!(
        dump,
        dump_static_effects(&infer_static_effects(&preliminary))
    );
    let final_program = lower_hir_to_final_mir(&hir);
    crate::mir::verify_mir(&final_program)
        .expect("planned structural static effects must produce valid final MIR");
    assert!(fields
        .iter()
        .all(|field| final_program.static_field(*field).is_some()));
}
