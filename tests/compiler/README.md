# Compiler Test Data

This directory contains reusable non-Rust inputs for compiler tests. Rust test
code remains beside its implementation or in
`crates/skald-compiler/tests/`, as defined by the
[testing guide](../../docs/development/TESTING.md).

[`robustness/`](robustness/README.md) owns retained hostile frontend corpus
files. Add another subdirectory only when multiple tests need stable non-Rust
data with a distinct discovery or encoding format.
