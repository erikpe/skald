# Compiler Tests

Compiler tests should mirror the pipeline: source/diagnostics, lexer, syntax, resolution, type checking, HIR, MIR, passes, driver, and each backend. Prefer small structural assertions and deterministic IR dumps. Tests of a target's emitted assembly should not require executing that target.

Fast Rust unit tests live beside the implementation they exercise. Run all current compiler tests with `make compiler-test`. The top-level directory is reserved for larger cross-module fixtures and compiler integration tests as those become necessary.
