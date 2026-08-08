# Primitive String Conversions Discoveries

This record owns maintainability work discovered while implementing the
primitive string conversions roadmap but not required for its source-visible
conversion behavior.

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
