# Standard vector golden tests

`vectors.golden.toml` owns executable behavior for generic `std::vec::Vec<T>`.
Coverage includes default and requested capacity, geometric growth, positive
and negative indexing, replacement, pop and clear cleanup, independent
structural copies, logical-length bounds, empty last/pop failure, exact inline
values, strings, nested optionals, arrays, nested generic vectors, shared
exact/interface owners, heterogeneous shared `Obj` owners, optional shared
owners, shared optional boxes, optional box owners, prompt last-owner release,
and cross-module applications. The positive generic type-shape matrix keeps
every specialization in its own function within one compilation unit and
forces each vector through geometric growth.

`generic_failures.golden.toml` owns contextual rejection of bare interface
and `Obj` storage, the grammar-level rejection of `unit`, and unavailable
element lifecycle operations. Resolver-stage failures share one source and
one compile-fail leaf with independently named diagnostic matchers. The
parser-stage `unit` rejection remains a separate source because parsing stops
before resolver diagnostics can be produced. Full golden determinism repeats
these source-to-observation cases across independent compiler and native
processes.

Run this group with `scripts/golden.sh --filter 'standard_vec/**'`.
