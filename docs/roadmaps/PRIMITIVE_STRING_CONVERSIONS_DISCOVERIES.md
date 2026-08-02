# Primitive String Conversions Discoveries

This record owns maintainability work discovered while implementing the
primitive string conversions roadmap but not required for its source-visible
conversion behavior.

## Canonical standard-library fixture closure

**Priority:** medium after the primitive string conversions roadmap.

**Problem:** Compiler tests manually enumerate the canonical `std::str`,
`std::error`, `std::io`, and string companion modules in several independent
fixture builders. Adding `std::str::format_f64` and then its shared
`std::str::bigunsigned_helper` dependency required repeating each new module in
resolver intrinsic tests, MIR I/O fixtures, driver provider tests, and
cross-process pipeline fixtures. A future companion can silently leave a
fixture with an incomplete reachable module closure.

**Evidence:** The canonical constants and general loader live in
`crates/skald-compiler/src/test_support.rs`, while additional source lists
remain in `src/resolve/tests/intrinsics.rs`, `src/mir/test_fixtures/io.rs`,
`src/driver/tests/pipeline.rs`, and `tests/pipeline_determinism.rs`.

**Likely owner:** Compiler test-support and module-fixture infrastructure.

**Useful boundary:** Provide one reusable canonical standard-library closure
builder that can install all current modules, accept explicit per-module
overrides for malformed/replacement tests, and still let determinism tests
choose source-creation order deliberately. Migrate the duplicated lists and
retain exact reachable-source-count assertions. Do not turn this into a
production module-loader shortcut or hide provider-root behavior under test.

## Optional shared-array result unwrap

**Priority:** high after the primitive string conversions roadmap.

**Problem:** A standard-library helper returning `shared? u8[]` resolves and
type-checks, but unwrapping that result into a fresh `shared u8[]` owner can
produce MIR rejected by the verifier with "optional shared unwrap requires
matching optional source and fresh shared owner". The failure affects every
program loading `std::str`, even when the method is not called.

**Evidence:** Temporarily changing `std::str::format_f64::format` to return
`shared? u8[]` and unwrapping its finite result in `Str.from_f64` made the
golden runner fail during MIR lowering. Ordinary optional shared-class and
shared-array profiles otherwise compile and execute.

**Likely owner:** Optional shared-owner HIR-to-MIR lowering and verification.

**Useful boundary:** Add a focused source-to-MIR test for a produced optional
shared-array function result unwrapped into a fresh shared-array local. Repair
the ownership identities or lowering shape without weakening the verifier,
then cover present, absent, direct-call, forwarded-result, and cleanup paths.

**Exit criteria:** Produced `shared? T[]` results unwrap into fresh `shared T[]`
owners through verified MIR and execute with correct ownership and cleanup.
