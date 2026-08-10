# Standard object vector golden tests

`vectors.golden.toml` owns the executable contract for `std::vec::VecObj`:
default and requested capacity, geometric growth, positive and negative
indexing, replacement, pop and clear ownership, independent structural copies,
logical-length bounds, and empty last/pop failure.

Run this group with `scripts/golden.sh --filter 'standard_vec/**'`.
