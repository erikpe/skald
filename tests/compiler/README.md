# Compiler Tests

Compiler tests should mirror the pipeline: source/diagnostics, lexer, syntax, resolution, type checking, HIR, MIR, passes, driver, and each backend. Prefer small structural assertions and deterministic IR dumps. Tests of a target's emitted assembly should not require executing that target.

