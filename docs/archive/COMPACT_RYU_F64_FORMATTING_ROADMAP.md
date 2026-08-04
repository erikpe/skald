# Compact Ryū Binary64 Formatting Roadmap

Status: **complete**. RYU0 and RYU1 are implemented and validated.

This roadmap replaced the allocation-heavy exact-rational finite binary64
formatter with a fixed-width Ryū implementation. The public `Str.from_f64`
API, its frozen shortest-text contract, special-value facade, parser, and
runtime boundary remain unchanged. The durable implementation uses exact
binary64 bits, Ryū's size-optimized cached powers, and one lazily decoded
process-lifetime table owned by zero-default static storage.

## Scope and invariants

- Preserve every byte of the implemented finite formatting contract,
  including signed zero, shortest round trips, nearest/even selection, and
  the existing plain/scientific threshold.
- Decode one canonical 832-byte little-endian table encoding, split into five
  immortal string-literal sections, into one private 104-word static `u64[]`
  on first finite non-small use, then reuse it.
- Publish the table only after complete local decoding; use the empty static
  array as the readiness sentinel and rely on the current single-threaded
  execution profile.
- Use only fixed-width wrapping integer arithmetic, `std::f64::to_bits`, and
  ordinary Skald arrays and strings. Add no formatting runtime ABI, bigint,
  host formatter, static initializer, or compiler-owned Ryū operation.
- Keep the correctly rounded decimal parser and its wide fallback unchanged.
- Retain upstream Ryū attribution and keep generated constants auditable.

## Progress

- [x] RYU0 — Implement the compact cached-power and fixed-width conversion core
- [x] RYU1 — Integrate canonical rendering, validation, and documentation

## PR-sized implementation sequence

### RYU0 — Implement the compact cached-power and fixed-width conversion core

**Purpose:** Establish the one-time table lifetime and exact fixed-width
arithmetic before replacing the user-visible formatter path.

- [x] Add the canonical packed table, local decoder, private static owner, and
      publish-after-success initialization path.
- [x] Implement portable `64 × 64 -> 128` multiplication, 128-bit shifting,
      compact cached-power reconstruction, and the exact Ryū interval core.
- [x] Decode the input with `std::f64::to_bits`; keep every operation bounded
      by binary64/Ryū invariants and free of per-value table allocation.
- [x] Add focused native coverage for table reuse and fixed-width boundary
      behavior.

**Tests:** Focused formatter golden tests, table initialization/allocation
observation, deterministic compilation, and the existing generated corpus.

**Exit criteria:** Every finite nonzero input reaches a fixed-width Ryū decimal
pair through one reusable decoded table, with no bigint construction or
per-value table decoding.

### RYU1 — Integrate canonical rendering, validation, and documentation

**Purpose:** Replace the old formatter completely while preserving its frozen
textual behavior and proving the intended performance improvement.

- [x] Feed the Ryū mantissa and exponent into the existing canonical
      plain/scientific rendering policy and retain exact special-value output.
- [x] Remove formatter-only `BigUnsigned` use while leaving parsing unchanged.
- [x] Verify exact expected text and bit-identical parser round trips across
      the checked-in corpus and focused edge cases.
- [x] Measure the corpus before and after, update living implementation and
      testing documentation, and record any out-of-scope findings separately.

**Tests:** Native formatter cases and corpus, timing comparison, `make check`,
and `make msrv-check`.

**Exit criteria:** The complete repository observes the same frozen text with
materially faster finite formatting, one lazy process-lifetime table, no
formatter bigint path, and all repository gates passing.

## Validation result

- The 2,929-line corpus remained byte-identical and reparsed every formatted
  result to the same binary64 bits.
- On the same development build and machine, corpus execution improved from
  20.54 seconds to 0.71 seconds, approximately 29× faster.
- All 274 native and compile-fail goldens passed.
- `make check` and the Rust 1.82 `make msrv-check` gate passed.

## Ordering and dependencies

The implemented exact `std::f64` bit reinterpretation and zero-default static
fields were prerequisites. Fixed-width conversion and table lifetime landed
before rendering integration so arithmetic defects remained separable from
text policy. The completed primitive string conversion roadmap remains
historical; this focused replacement changed implementation cost, not its
public contract.
