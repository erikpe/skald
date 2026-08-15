//! Static-effect guarantees for closed generic class specializations.

use crate::{
    identity::StaticFieldId,
    mir::{PreliminaryMirProgram, StaticEffectEdgeKind, StaticEffectNode},
    test_support::lower_generic_source_to_preliminary_mir,
};

use super::super::{dump_static_effects, infer_static_effects};

const CACHE_SOURCE: &str = "class Str { init() {} }
     class Cache<T> {
       static seed: i64 = 1;
       static direct: i64 = Cache<T>.seed;
       static transitive: i64 = Cache<T>.read();
       init() {}
       static fn read() -> i64 { return Cache<T>.direct; }
     }
     fn main() -> i64 {
       return Cache<i64>.transitive + Cache<Str>.transitive;
     }";

fn field(program: &PreliminaryMirProgram, owner: &str, name: &str) -> StaticFieldId {
    program
        .static_fields()
        .map(|field| field.field)
        .find(|field| {
            program.static_field_qualified_name(*field).as_deref()
                == Some(&format!("{owner}.{name}"))
        })
        .unwrap_or_else(|| panic!("missing static field `{owner}.{name}`"))
}

#[test]
fn static_selection_alone_materializes_independent_slots_and_initializer_bodies() {
    let preliminary = lower_generic_source_to_preliminary_mir(CACHE_SOURCE);
    let fields = preliminary.static_fields().collect::<Vec<_>>();
    let initializers = preliminary.static_initializers().collect::<Vec<_>>();

    assert_eq!(fields.len(), 6);
    assert_eq!(initializers.len(), 6);
    for owner in ["Cache<i64>", "Cache<Str>"] {
        let owner_fields = [
            field(&preliminary, owner, "seed"),
            field(&preliminary, owner, "direct"),
            field(&preliminary, owner, "transitive"),
        ];
        assert!(owner_fields
            .iter()
            .all(|field| field.class() == owner_fields[0].class()));
        assert!(initializers
            .iter()
            .filter(|initializer| initializer.field.class() == owner_fields[0].class())
            .all(|initializer| initializer.id.class() == owner_fields[0].class()));
    }
    assert_ne!(
        field(&preliminary, "Cache<i64>", "seed").class(),
        field(&preliminary, "Cache<Str>", "seed").class()
    );
}

#[test]
fn generated_initializer_effects_remain_specialization_local_and_transitive() {
    let preliminary = lower_generic_source_to_preliminary_mir(CACHE_SOURCE);
    let analysis = infer_static_effects(&preliminary);

    for owner in ["Cache<i64>", "Cache<Str>"] {
        let direct = field(&preliminary, owner, "direct");
        let transitive = field(&preliminary, owner, "transitive");
        let initializer = preliminary
            .static_initializers()
            .find(|initializer| initializer.field == transitive)
            .expect("explicit generated static must retain its initializer body");
        let summary = analysis
            .summary(StaticEffectNode::Callable(initializer.callable()))
            .expect("generated static initializer must receive an effect summary");
        let effect = summary
            .effects
            .iter()
            .find(|effect| effect.field == direct)
            .expect("static method effect must propagate into its initializer");

        assert!(effect
            .witness
            .iter()
            .any(|edge| edge.kind == StaticEffectEdgeKind::StaticCall));
        assert!(summary
            .effects
            .iter()
            .all(|effect| effect.field.class() == transitive.class()));
    }

    let dump = dump_static_effects(&analysis);
    assert_eq!(
        dump,
        dump_static_effects(&infer_static_effects(&preliminary))
    );
}
