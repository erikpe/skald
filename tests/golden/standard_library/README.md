# Standard-library configuration fixtures

`replacement.golden.toml` owns disabled and replacement standard-library
selection plus replacement implementations of the string and I/O intrinsics.
The I/O success case copies its data into the private run directory instead of
sharing a writable fixture working directory.

Run this group with `scripts/golden.sh --filter 'standard_library/**'`.
