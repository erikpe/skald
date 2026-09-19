use super::*;
use crate::test_support::TemporaryDirectory;
use std::{fs, sync::atomic::AtomicU64};

#[test]
fn temporary_store_round_trips_typed_keys_and_removes_fragments() {
    let mut store = TempFragmentStore::create().unwrap();
    let key = LirCallableId::Entry;
    store.write(key, "entry\n").unwrap();
    assert_eq!(store.read(key).unwrap(), "entry\n");
    assert!(store.write(key, "duplicate").is_err());
    store.remove(key).unwrap();
    assert!(store.read(key).is_err());
}

#[test]
fn temporary_store_skips_a_stale_process_counter_directory() {
    let parent = TemporaryDirectory::new("native-fragment-stale").unwrap();
    fs::create_dir(parent.join("skald-native-fragments-7-0")).unwrap();
    let next = AtomicU64::new(0);

    let store = TempFragmentStore::create_in(parent.path(), 7, &next).unwrap();
    assert!(parent.join("skald-native-fragments-7-1").is_dir());
    drop(store);
    assert!(!parent.join("skald-native-fragments-7-1").exists());
    assert!(parent.join("skald-native-fragments-7-0").is_dir());
}
