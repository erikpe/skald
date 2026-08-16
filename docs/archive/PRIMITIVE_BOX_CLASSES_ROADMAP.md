# Primitive Box Classes Roadmap

Status: complete and archived.

This roadmap completes the standard-library object wrappers for every
implemented primitive value type so primitive values can participate in
ordinary `Equatable` and `Hashable` object APIs.

## Scope and invariants

- Add one ordinary standard-library module and exact box class for each of
  `i64`, `u64`, `u8`, and `bool`, complementing `std::f64::BoxF64`.
- Preserve exact primitive equality and use a distinct fixed hash domain for
  every box class before the shared `mix_u64` finalizer.
- Add no primitive interface conformance, compiler intrinsic, implicit boxing,
  runtime symbol, or ABI change.

## Progress

- [x] PB0 — Complete the primitive box-class matrix

## PR-sized implementation sequence

### PB0 — Complete the primitive box-class matrix

**Purpose:** Provide consistent explicit object wrappers for all remaining
primitive types and prove their ordinary interface composition.

- [x] Add `BoxI64`, `BoxU64`, `BoxU8`, and `BoxBool` in their type-named
      standard-library modules.
- [x] Implement exact equality and domain-separated mixed hashing.
- [x] Cover direct, cross-class, interface, generic-bound, boundary-value, and
      cross-domain native behavior.
- [x] Publish the module and hashing contract in living documentation.
- [x] Run focused native checks and the full repository quality gate.

**Tests:** Focused primitive-box golden execution, standard-library module
resolution through the normal compiler pipeline, then `make check`,
documentation validation, formatting, and diff hygiene.

**Exit criteria:** Every implemented value-bearing primitive has an explicit
box class with consistent equality/hash behavior, all domains are distinct,
and all repository gates pass.

## Ordering and dependencies

The existing `Equatable`, `Hashable`, `mix_u64`, and `BoxF64` contracts are the
complete baseline. The four independent modules land together so tests can
prove cross-box inequality and hash-domain separation in one matrix.

Completed 2026-08-16. The installed standard library now provides explicit
box classes for all five value-bearing primitive types. Focused native
behavior, `make check`, artifact-free `make check-long`, full golden
determinism, Rust 1.82.0 MSRV, 10,000-case robustness, documentation, and diff
hygiene passed without adding compiler or runtime machinery.
