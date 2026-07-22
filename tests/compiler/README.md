# Compiler Tests

Compiler tests should mirror the pipeline: source/diagnostics, lexer, syntax, resolution, type checking, HIR, MIR, passes, driver, and each backend. Prefer small structural assertions and deterministic IR dumps. Tests of a target's emitted assembly should not require executing that target.

Fast Rust unit tests live beside the implementation they exercise, with larger
suites split into behavior-oriented modules. Run all current compiler tests
with `make compiler-test`. This top-level directory is reserved for larger
cross-module fixtures and compiler integration tests as those become necessary.

`robustness/` owns deterministic hostile-input corpora and documents the fast
smoke and longer scheduled commands. Corpus files stay here rather than inside
production crates.
