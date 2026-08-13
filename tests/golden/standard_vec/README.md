# Standard vector golden tests

`vectors.golden.toml` owns executable behavior for generic `std::vec::Vec<T>`
and the retained `VecObj` compatibility class. Coverage includes default and
requested capacity, geometric growth, positive and negative indexing,
replacement, pop and clear cleanup, independent structural copies, logical-
length bounds, empty last/pop failure, exact inline values, strings, nested
optionals, arrays, shared exact/interface owners, prompt last-owner release,
and cross-module applications.

`generic_failures.golden.toml` owns contextual rejection of bare interface
storage and unavailable element lifecycle operations. Full golden determinism
repeats these source-to-observation cases across independent compiler and
native processes.

Run this group with `scripts/golden.sh --filter 'standard_vec/**'`.
