//! Final static coordinator coverage for closed generic owners.

use crate::{
    mir::{dump_mir, MirStaticValueCleanup},
    passes::static_lifecycle::verify_synthesized_mir,
    test_support::lower_generic_source_to_final_mir,
};

const SOURCE: &str = "class Item { init() {} destroy {} }
     class Cache<T> {
       static current: T?;
       static explicit: T? = none;
       init() {}
     }
     fn main() -> i64 {
       Cache<Item>.current = Item();
       Cache<Item>.current = Item();
       Cache<shared Item>.current = new Item();
       Cache<shared Item>.current = none;
       return 42;
     }";

#[test]
fn specializes_replacement_cleanup_publication_and_reverse_shutdown() {
    let program = lower_generic_source_to_final_mir(SOURCE);
    verify_synthesized_mir(&program).expect("closed generic lifecycle certificate must verify");
    let coordinator = program
        .static_lifecycle
        .as_ref()
        .expect("generic owning statics require a final coordinator");

    assert_eq!(coordinator.activation().len(), 4);
    assert_eq!(coordinator.initializers().len(), 2);
    assert!(coordinator
        .shutdown()
        .iter()
        .map(|region| region.field)
        .eq(coordinator
            .activation()
            .iter()
            .rev()
            .map(|region| region.field)));

    let optional_class = coordinator
        .shutdown()
        .iter()
        .filter(|region| matches!(region.cleanup, MirStaticValueCleanup::OptionalClass(_)))
        .count();
    let optional_shared = coordinator
        .shutdown()
        .iter()
        .filter(|region| matches!(region.cleanup, MirStaticValueCleanup::OptionalShared(_)))
        .count();
    assert_eq!((optional_class, optional_shared), (2, 2));
}

#[test]
fn final_mir_dump_names_closed_static_owners_without_losing_identities() {
    let program = lower_generic_source_to_final_mir(SOURCE);
    let dump = dump_mir(&program);

    for name in [
        "Cache<Item>.current",
        "Cache<Item>.explicit",
        "Cache<shared Item>.current",
        "Cache<shared Item>.explicit",
    ] {
        assert!(dump.contains(&format!("\"{name}\"")), "{dump}");
    }
    for class in program
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Cache<"))
    {
        for field in &class.static_fields {
            assert!(dump.contains(&field.id.to_string()), "{dump}");
        }
    }
    assert_eq!(dump, dump_mir(&lower_generic_source_to_final_mir(SOURCE)));
}
