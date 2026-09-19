use super::*;

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
