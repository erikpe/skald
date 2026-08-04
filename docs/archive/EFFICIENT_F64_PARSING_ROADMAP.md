# Efficient Binary64 Parsing Roadmap

Status: complete.

This roadmap replaces the allocation-heavy common path in the standard-library
decimal-to-binary64 parser while preserving its frozen syntax, range, and exact
rounding contract. The durable result is a small exact path followed by
fixed-width Eisel-Lemire conversion, with bounded wide arithmetic reserved for
the rare inputs whose rounding cannot be decided from the retained prefix.

## Scope and invariants

- Preserve complete-input parsing, arbitrary textual length, nearest/even
  rounding, signed zero, finite-overflow failure, and the existing public
  `std::str::parse_f64::parse` signature.
- Keep the parser's cached powers and multiplication helpers independent from
  the Ryū formatter implementation.
- Retain at most 768 significant digits plus a nonzero-tail bit in the exact
  fallback; never allocate numeric storage proportional to input length.
- Move the parser-only fixed-capacity unsigned helper into
  `std::str::parse_f64` and remove the obsolete descendant module.
- Do not add a runtime parser, host floating conversion, locale dependency, or
  new source-visible `Str` conversion API.

## Progress

- [x] EFP0 — Add fixed-width Eisel-Lemire conversion
- [x] EFP1 — Bound and internalize the exact fallback

## PR-sized implementation sequence

### EFP0 — Add fixed-width Eisel-Lemire conversion

**Purpose:** Make ordinary finite decimals fast after the existing exact small
path without changing the parser's accepted language or rounding contract.

- [x] Retain the first 19 significant digits, adjusted decimal exponent, and
      truncation state during the validated lexical scan.
- [x] Implement Eisel-Lemire binary64 conversion with portable fixed-width
      integer arithmetic and an independently owned cached-power table.
- [x] Accept a truncated prefix only when conversions of both interval endpoints
      select the same binary64; otherwise enter the exact fallback.
- [x] Construct the result through `std::f64::from_bits`, preserving signed zero
      and rejecting conversion to infinity under the frozen finite contract.

**Tests:** Focused native parser golden, exact oracle corpus, common and
truncated Eisel-Lemire cases, and a timing sanity check over the checked-in
formatting corpus.

**Exit criteria:** Ordinary decimals use fixed-width conversion after the small
exact path, ambiguous cases still reach the exact fallback, and every existing
parser oracle result remains byte-exact.

### EFP1 — Bound and internalize the exact fallback

**Purpose:** Keep the rare exact-rounding path cohesive and bounded after the
common path no longer needs wide arithmetic.

- [x] Retain exactly 768 significant digits plus a sticky nonzero-tail bit and
      document the binary64 rounding bound.
- [x] Move `BigUnsigned` into `std::str::parse_f64`, update its focused test, and
      remove the obsolete module from canonical standard-library fixtures.
- [x] Update living standard-library and string documentation plus third-party
      provenance for the implemented parser architecture.
- [x] Run focused tests, `make check`, and `make msrv-check`.

**Tests:** Big-unsigned owner golden; halfway, subnormal, maximum-finite,
overflow, long-significand, and excessive-exponent parser cases; repository
quality gates.

**Exit criteria:** The parser has no common-path wide allocation, the exact
fallback is bounded to the proven digit count, only `parse_f64.ska` owns its
wide helper, documentation describes current behavior, and all gates pass.

## Ordering and dependencies

The fixed-width path lands first so the retained-prefix interval determines
when exact arithmetic is necessary. The fallback can then be tightened and
internalized without changing the decision boundary. Both tasks depend on the
implemented `std::f64` bit-conversion functions and the frozen primitive string
conversion contract; neither depends on formatter tables or helpers.
