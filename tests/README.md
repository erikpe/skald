# Test Data and Harnesses

General test ownership and selection are defined in the
[testing guide](../docs/development/TESTING.md). This tree contains reusable
non-Rust data and cross-crate harnesses:

- [`compiler/`](compiler/README.md) contains compiler corpus data;
- [`golden/`](golden/README.md) contains feature-owned `.ska` cases, specs, and
  external expectation data; and
- [`runtime/`](runtime/README.md) contains direct C runtime harnesses.

Rust unit tests live beside their implementation. Public and cross-phase Rust
integration tests live in the owning crate's `tests/` directory.
