# Standard-library configuration fixtures

`replacement.golden.toml` owns disabled and replacement standard-library
selection, replacement implementations of the string and I/O intrinsics, and
the canonical operator-protocol bundle. The operator case exercises ordinary
class conformance, a generic bound, and a manual bound-selected method call;
primitive punctuation remains usable with the standard library disabled. The
I/O success case copies its data into the private run directory instead of
sharing a writable fixture working directory.

Run this group with `scripts/golden.sh --filter 'standard_library/**'`.
