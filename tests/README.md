# Tests

The test tree separates compiler behavior, the C runtime, and complete source-to-executable behavior:

- `compiler/` — reserved for larger cross-phase compiler fixtures and integration tests;
- `runtime/` — separate C harnesses for the runtime contract, successful output, and fatal output failures;
- `golden/` — end-to-end `.ska` compilation cases with stable expected results.

Current phase-level Rust tests live beside the implementation they exercise.
Larger suites use behavior-oriented `tests/` modules, such as MIR builder,
lowering, control-flow, verification, and dump tests. Cross-phase fixtures and
compiler integration tests should remain under the compiler test category
rather than being mixed with language golden cases.
