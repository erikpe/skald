# Standard map golden tests

`maps.golden.toml` covers generic `std::map::Map<K, V>` construction,
capacity growth, deliberately colliding keys, equality-based replacement,
lookup, removal, tombstone reuse, clear, requested capacity, and structural
bracket reads and writes.
The group also verifies the `Equatable` and `Hashable` key bounds and the
checked missing-key failure.

Run this group with `scripts/golden.sh --filter 'standard_map/**'`.
