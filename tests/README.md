# Tests

The test tree separates reusable compiler corpus data, the C runtime, and
complete source-to-executable behavior:

- `compiler/` — non-Rust corpus data shared by compiler robustness tests;
- `runtime/` — separate C harnesses for the runtime contract, successful output, and fatal output failures;
- `golden/` — end-to-end `.ska` compilation cases with stable expected results.

Phase-level Rust tests live beside the implementation they exercise. Larger
suites use behavior-oriented `tests/` modules. Rust tests that require only the
public compiler API, including cross-phase checks, live in
`crates/skald-compiler/tests/`. Top-level compiler corpus files are test data,
not an alternate Rust integration-test location.
