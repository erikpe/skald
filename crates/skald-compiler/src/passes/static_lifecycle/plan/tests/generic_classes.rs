//! Static lifetime planning for closed generic owners.

use crate::{
    identity::StaticFieldId,
    mir::{PlannedMirProgram, PreliminaryMirProgram},
    test_support::lower_generic_source_to_preliminary_mir,
};

use super::super::{
    dump_planned_mir, dump_static_lifetime_plan, plan_static_lifetimes,
    STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY,
};

fn field(program: &PreliminaryMirProgram, owner: &str, name: &str) -> StaticFieldId {
    let expected = format!("{owner}.{name}");
    program
        .static_fields()
        .map(|field| field.field)
        .find(|field| {
            program.static_field_qualified_name(*field).as_deref() == Some(expected.as_str())
        })
        .unwrap_or_else(|| panic!("missing static field `{expected}`"))
}

fn plan(source: &str) -> PlannedMirProgram {
    plan_static_lifetimes(lower_generic_source_to_preliminary_mir(source))
        .expect("generic static lifetimes must plan successfully")
}

#[test]
fn direct_and_transitive_dependencies_are_local_to_each_closed_owner() {
    let planned = plan(
        "class Str { init() {} }
         class Cache<T> {
           final static seed: i64 = 1;
           final static direct: i64 = Cache<T>.seed;
           final static transitive: i64 = Cache<T>.read();
           init() {}
           static fn read() -> i64 { return Cache<T>.direct; }
         }
         fn main() -> i64 {
           return Cache<i64>.transitive + Cache<Str>.transitive;
         }",
    );
    let preliminary = planned.preliminary();

    for owner in ["Cache<i64>", "Cache<Str>"] {
        let seed = field(preliminary, owner, "seed");
        let direct = field(preliminary, owner, "direct");
        let transitive = field(preliminary, owner, "transitive");
        let activation = planned.lifecycle().activation();

        let seed_index = activation.iter().position(|field| *field == seed).unwrap();
        let direct_index = activation
            .iter()
            .position(|field| *field == direct)
            .unwrap();
        let transitive_index = activation
            .iter()
            .position(|field| *field == transitive)
            .unwrap();
        assert!(seed_index < direct_index);
        assert!(direct_index < transitive_index);
        assert!(planned.dependencies().iter().any(|dependency| {
            dependency.prerequisite == seed && dependency.dependent == direct
        }));
        assert!(planned.dependencies().iter().any(|dependency| {
            dependency.prerequisite == direct && dependency.dependent == transitive
        }));
        for field in [seed, direct, transitive] {
            assert!(preliminary
                .static_fields()
                .find(|candidate| candidate.field == field)
                .is_some_and(|candidate| candidate.final_span.is_some()));
            assert!(planned
                .lifecycle_mir()
                .definitions()
                .iter()
                .find(|candidate| candidate.field == field)
                .is_some_and(|candidate| candidate.final_span.is_some()));
        }
    }
    assert!(planned
        .dependencies()
        .iter()
        .all(|dependency| { dependency.prerequisite.class() == dependency.dependent.class() }));
    assert_eq!(
        planned.lifecycle().shutdown(),
        planned
            .lifecycle()
            .activation()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>()
    );
}

#[test]
fn plans_direct_and_transitive_dependencies_across_closed_owners() {
    let planned = plan(
        "class Source<T> {
           static base: i64 = 1;
           init() {}
         }
         class Sink<T> {
           static direct: i64 = Source<T>.base;
           static transitive: i64 = Sink<T>.read();
           init() {}
           static fn read() -> i64 { return Source<T>.base; }
         }
         fn main() -> i64 { return Sink<i64>.direct + Sink<i64>.transitive; }",
    );
    let preliminary = planned.preliminary();
    let base = field(preliminary, "Source<i64>", "base");
    let direct = field(preliminary, "Sink<i64>", "direct");
    let transitive = field(preliminary, "Sink<i64>", "transitive");

    assert_ne!(base.class(), direct.class());
    assert!(planned
        .dependencies()
        .iter()
        .any(|dependency| { dependency.prerequisite == base && dependency.dependent == direct }));
    assert!(planned.dependencies().iter().any(|dependency| {
        dependency.prerequisite == base && dependency.dependent == transitive
    }));
    let activation = planned.lifecycle().activation();
    let base_index = activation.iter().position(|field| *field == base).unwrap();
    assert!(
        base_index
            < activation
                .iter()
                .position(|field| *field == direct)
                .unwrap()
    );
    assert!(
        base_index
            < activation
                .iter()
                .position(|field| *field == transitive)
                .unwrap()
    );
}

#[test]
fn generic_plan_dumps_are_deterministic_readable_and_identity_preserving() {
    const SOURCE: &str = "class Cache<T> {
           static first: i64 = 1;
           static second: i64 = Cache<T>.first;
           init() {}
         }
         fn main() -> i64 {
           return Cache<i64>.second + Cache<bool>.second;
         }";
    let planned = plan(SOURCE);
    let plan_dump = dump_static_lifetime_plan(&planned);
    let full_dump = dump_planned_mir(&planned);

    for name in [
        "Cache<i64>.first",
        "Cache<i64>.second",
        "Cache<bool>.first",
        "Cache<bool>.second",
    ] {
        assert!(plan_dump.contains(&format!("\"{name}\"")), "{plan_dump}");
        assert!(full_dump.contains(&format!("\"{name}\"")), "{full_dump}");
    }
    for field in planned.static_fields() {
        assert!(plan_dump.contains(&field.field.to_string()), "{plan_dump}");
    }
    assert!(plan_dump.contains("activation="), "{plan_dump}");
    assert!(plan_dump.contains("shutdown="), "{plan_dump}");
    for _ in 0..8 {
        let again = plan(SOURCE);
        assert_eq!(dump_planned_mir(&again), full_dump);
    }
}

#[test]
fn diagnoses_specialized_self_dependencies_with_the_closed_owner_name() {
    let failure = plan_static_lifetimes(lower_generic_source_to_preliminary_mir(
        "class Loop<T> {
           static value: i64 = Loop<T>.read();
           init() {}
           static fn read() -> i64 { return Loop<T>.value; }
         }
         fn main() -> i64 { return Loop<i64>.value; }",
    ))
    .expect_err("a generated static initializer must not read itself before publication");
    let diagnostic = failure.diagnostics().next().unwrap();

    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_SELF_DEPENDENCY);
    assert!(diagnostic.message.contains("Loop<i64>.value"));
}

#[test]
fn diagnoses_cycles_across_closed_generic_owners() {
    let failure = plan_static_lifetimes(lower_generic_source_to_preliminary_mir(
        "class Left<T> {
           static value: i64 = Right<T>.value;
           init() {}
         }
         class Right<T> {
           static value: i64 = Left<T>.value;
           init() {}
         }
         fn main() -> i64 { return Left<i64>.value; }",
    ))
    .expect_err("cross-owner generic static dependencies must be rejected");
    let diagnostic = failure.diagnostics().next().unwrap();

    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_DEPENDENCY_CYCLE);
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("Left<i64>.value") && note.contains("Right<i64>.value")));
}
