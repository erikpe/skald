# `Str` Cached-Hash Migration Roadmap

Status: complete and archived.

This roadmap migrates the compiler-known `std::str::Str` descriptor from
three fields to four and makes `Str` an ordinary `Equatable` and `Hashable`
implementation with a byte-content hash cached through `private cell`.

## Scope and invariants

- Require `_hash_code: u64?` as the fourth exact private cell field.
- Initialize the cache to `none` in every source and intrinsic construction.
- Hash only the observable byte sequence, independent of backing, range, or
  escape spelling; equal strings must always have equal hashes.
- Preserve logical immutability, synthesized lifecycle, deterministic dumps,
  literal backing, layout derivation, and runtime ABI version 9.
- Add no string hashing intrinsic or runtime service.

## Progress

- [x] SHM0 — Migrate language-item metadata and intrinsic literal publication
- [x] SHM1 — Implement standard-library equality and cached hashing
- [x] SHM2 — Harden source-to-native behavior and publish the contract

## PR-sized implementation sequence

### SHM0 — Migrate language-item metadata and intrinsic literal publication

**Purpose:** Make the fourth field an exact cross-phase invariant before the
standard library depends on it.

- [x] Validate the exact private-cell optional-`u64` field in resolution.
- [x] Carry its identity through resolved, HIR, MIR, dumps, and verification.
- [x] Initialize intrinsic literals with an absent cache in target lowering.
- [x] Extend malformed-product, layout, and determinism tests.

**Tests:** Focused resolution, HIR, MIR, verifier, backend, dump, and string
determinism tests, then the repository Rust gates.

**Exit criteria:** Every accepted `Str` has the four-field identity and every
compiler-created literal publishes a verified absent cache.

### SHM1 — Implement standard-library equality and cached hashing

**Purpose:** Expose ordinary interface behavior with one byte-content hash
algorithm and safe interior cache mutation.

- [x] Make `Str` implement `Equatable` and `Hashable`.
- [x] Initialize the cache on every ordinary descriptor construction path.
- [x] Compute a stable byte-content hash, cache it once, and preserve exact
      byte-wise equality for different backing and slice shapes.
- [x] Add direct, interface, generic-bound, literal, dynamic, slice, copy, and
      repeated-call native tests.

**Tests:** Standard-library resolution/type-check tests and focused native
goldens covering equality/hash consistency and cache-bearing lifecycle.

**Exit criteria:** All equal strings hash equally and every supported `Str`
construction and lifecycle path remains correct.

### SHM2 — Harden source-to-native behavior and publish the contract

**Purpose:** Close diagnostics, documentation, determinism, ABI, and roadmap
state after the representation and library behavior are proven.

- [x] Extend malformed language-item diagnostics and complete phase dumps.
- [x] Publish the four-field descriptor and interface/hash contract in living
      language, compiler, testing, and debugging documentation.
- [x] Confirm no intrinsic, runtime symbol, public header, or ABI revision.
- [x] Run artifact-free full, extended, MSRV, documentation, and diff gates;
      archive this roadmap and the resolved discovery.

**Tests:** Complete golden determinism, `make check`, `make check-long`,
`make msrv-check`, documentation validation, and `git diff --check`.

**Exit criteria:** The cached hash is the implemented documented contract,
all repository gates pass, and no actionable discovery remains.

Completed 2026-08-16. The four-field language item, absent intrinsic cache
publication, standard-library interfaces, cached byte-content hash, malformed
product checks, native composition matrix, and living documentation shipped
together. Artifact-free `make check-long`, full golden determinism, Rust
1.82.0 MSRV, 10,000-case robustness, documentation validation, and diff
hygiene passed.

## Ordering and dependencies

The compiler representation changes before standard-library source so
intrinsic literals can never construct a partial descriptor. Library behavior
then exercises the private-cell contract, and publication closes only after
source-to-native and malformed-product coverage passes.
