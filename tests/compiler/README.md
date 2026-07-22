# Compiler Tests

Compiler tests mirror the pipeline: source/diagnostics, lexer, syntax,
resolution, type checking, HIR, MIR, passes, driver, and each backend. Prefer
small structural assertions and deterministic IR dumps. Tests of emitted
assembly should not require executing the target.

Fast Rust unit tests live beside the implementation they exercise, with larger
suites split into behavior-oriented modules. Run all current compiler tests
with `make compiler-test`. Public-API and cross-phase Rust integration tests
live in `crates/skald-compiler/tests/`.

This directory owns non-Rust test data that should remain independent of a
production crate. `robustness/` contains deterministic hostile-input corpora
and documents the fast smoke and longer scheduled commands.
