# Tests

The test tree separates compiler behavior, the C runtime, and complete source-to-executable behavior:

- `compiler/` — phase-level compiler tests and shared fixtures;
- `runtime/` — C ABI and runtime implementation tests;
- `golden/` — end-to-end `.ska` compilation cases with stable expected results.

Idiomatic Rust unit tests may live beside the implementation they exercise. Cross-phase fixtures and compiler integration tests should remain under the compiler test category rather than being mixed with language golden cases.

