# Primitive String Conversions Roadmap

Status: complete.

This roadmap moves portable primitive formatting and parsing into the Skald
standard library. The durable result is one explicit type-named `Str` method
pair for each primitive type, optional parse failure without content-triggered
panic, deterministic locale-independent text, and ordinary `std::io`
composition. Once standard-library behavior has replaced its testing role,
the bootstrap scalar-output C helpers can leave the runtime ABI.

The authoritative source-visible API and textual semantics are frozen in
[Skald Strings](../language/STRINGS.md#frozen-primitive-textual-conversions).
This roadmap owns implementation order and does not redefine that contract.

## Scope and invariants

- Add `Str.from_bool`, `Str.from_i64`, `Str.from_u64`, `Str.from_u8`, and
  `Str.from_f64` as ordinary static standard-library methods.
- Add `Str.to_bool`, `Str.to_i64`, `Str.to_u64`, `Str.to_u8`, and
  `Str.to_f64` as ordinary instance methods returning primitive optionals.
- Keep all method selection in ordinary source semantics. No method name
  becomes a language item, compiler intrinsic, or implicit conversion.
- Produce and accept only the frozen ASCII, locale-independent textual
  contract. Parse the complete receiver and return `none` for malformed or
  out-of-range content without content-triggered panic.
- Preserve exact integer boundaries, binary64 signed zero and infinities, and
  finite binary64 format-then-parse identity. NaN round trips by category, not
  payload or sign.
- Implement conversion algorithms in Skald source. The C runtime does not
  format, parse, allocate `Str`, inspect optional storage, or acquire a
  conversion ABI.
- Keep `from_u8` numeric and keep byte-string construction under the existing
  `from_bytes` name.
- Keep `std::io` byte-exact. Type-named primitive `println_<type>` conveniences
  append exactly one ASCII line feed; interpolation, format strings, arbitrary
  radices, parser diagnostics, whitespace trimming, and Unicode remain
  excluded.
- Preserve the five bootstrap scalar observation helpers until repository
  programs and tests no longer depend on them; remove them only in the final
  focused compatibility slice.
- New substantial conversion algorithms belong in cohesive private methods or
  companion standard-library modules, with classes reserved for stateful
  responsibilities. The supported conversion surface remains owned by `Str`;
  narrow implementation entry points may be publicly callable where current
  module visibility requires it.

## Progress

- [x] TXT0 — Freeze the API and textual contract
- [x] TXT1 — Implement boolean and integer formatting
- [x] TXT2 — Implement optional boolean and integer parsing
- [x] TXT3 — Implement correctly rounded binary64 parsing
- [x] TXT4 — Implement shortest round-tripping binary64 formatting and
      primitive line output
- [x] TXT5 — Adopt string I/O and retire scalar runtime observation

Every implementation task runs focused standard-library and native golden
tests, then `make check` and `make msrv-check`. Documentation-only TXT0 runs
`make docs-check`. The Makefile remains the repository automation interface.

## PR-sized implementation sequence

### TXT0 — Freeze the API and textual contract

**Purpose:** Settle names, optional failure, accepted grammars, canonical
output, binary64 round trips, ownership boundaries, and runtime exclusions
before algorithms or migrations depend on them.

- [x] Add the ten exact public method signatures to the authoritative string
      contract.
- [x] Freeze boolean, signed integer, unsigned integer, byte, finite binary64,
      infinity, NaN, and signed-zero formatting.
- [x] Freeze complete-input parsing, accepted noncanonical decimal forms,
      exact ranges, optional failure, and absence of content-triggered panic.
- [x] State round-trip requirements and the lack of compiler, intrinsic,
      locale, source-literal-suffix, or runtime conversion semantics.
- [x] Reconcile the language status, standard-library guide, grammar note, and
      active roadmap index while retaining current implementation wording.

**Tests:** `make docs-check`; review non-archived matches from
`rg -n "from_(bool|i64|u64|u8|f64)|to_(bool|i64|u64|u8|f64)|primitive textual" docs std -g '*.md' -g '*.ska'`.

**Exit criteria:** One living source contract answers every public naming,
grammar, range, failure, round-trip, ownership, and runtime-boundary question;
the status matrix clearly says the methods are frozen but unavailable; and no
executable source, compiler, or runtime behavior has changed.

### TXT1 — Implement boolean and integer formatting

**Purpose:** Establish total value-producing formatting with the simpler
primitive families before parsing or floating conversion adds larger
algorithms.

- [x] Implement `from_bool`, `from_i64`, `from_u64`, and `from_u8` in
      `std::str::Str` with exact canonical spellings and valid immutable
      descriptors.
- [x] Factor shared unsigned magnitude digit emission without conflating byte
      formatting with one-byte string construction.
- [x] Handle `i64` minimum without overflowing during magnitude formation.
- [x] Preserve ordinary allocation failure and descriptor lifecycle behavior;
      add no intrinsic or runtime call.
- [x] Add source-to-native tests for every boolean, zero, sign transition,
      powers of ten, and all integer extrema through `std::io::write_stdout`.

**Tests:** Focused standard-library/module/type-check tests; native boundary
goldens; deterministic assembly comparison; `make check`; `make msrv-check`.

**Exit criteria:** The four simpler formatters produce exactly the frozen bytes
for all primitive inputs through ordinary compiled Skald code, and no runtime
formatting service is selected.

### TXT2 — Implement optional boolean and integer parsing

**Purpose:** Establish one reusable complete-input and optional-failure parser
discipline before the more demanding binary64 grammar.

- [x] Implement exact lowercase boolean parsing and the three integer decimal
      parsers with their frozen sign rules.
- [x] Return `none` for empty, malformed, trailing, or out-of-range content;
      never call panic because of input content.
- [x] Check multiplication/addition thresholds before they can wrap and handle
      the asymmetric `i64` minimum magnitude directly.
- [x] Share digit classification and checked unsigned accumulation only where
      it keeps each target's range and sign policy explicit.
- [x] Test present and absent optionals without using unchecked unwrap on an
      expected failure path.

**Tests:** Exhaustive `u8` round trips; integer extrema and one-step overflow;
leading-zero, negative-zero, sign, whitespace, suffix, embedded-zero, and long
input matrices; native optional-result goldens; `make check`;
`make msrv-check`.

**Exit criteria:** All boolean and integer parsers accept exactly their frozen
languages, every formatter output round trips, arbitrary-length invalid input
cannot wrap or panic because of content, and failures are observable as
`none`.

### TXT3 — Implement correctly rounded binary64 parsing

**Purpose:** Build the exact decimal-to-binary64 boundary independently of the
formatter so formatter verification and final round-trip tests rest on an
implemented parser contract.

- [x] Recognize only the three exact special spellings in `Str` through its
      generic byte-equality method, and recognize only the frozen decimal
      grammar in the companion parser, preserving a leading negative sign
      through zero underflow.
- [x] Parse arbitrary-length significands and exponents without integer wrap,
      and round decimal values once to nearest binary64 with ties to even.
- [x] Return `none` for malformed text or a finite decimal that rounds to
      infinity; return present subnormal or signed zero for valid underflow.
- [x] Keep wide-integer and decimal-scaling machinery encapsulated, bounded,
      and shared with formatting only where it has the same proven
      responsibility; each conversion retains its own proven capacity.
- [x] Keep common values allocation-free by scanning at most 19 significant
      digits into `u64`, using only proven integer and exact-power conversions,
      and retaining the exact wide fallback for every other finite value.
- [x] Extract the substantial parser into private functions in the
      `std::str::parse_f64` companion module. Keep `Str.to_f64` as the
      conversion facade and pass only a validated, call-scoped backing-array
      range through the public implementation entry point.
- [x] Add an independently generated oracle corpus covering exact halfway
      cases, adjacent decimal strings, normal/subnormal transitions, extrema,
      signed zero, and excessive exponents.

**Tests:** Focused parser tests, checked-in oracle corpus, allocation-free
conversion boundaries and adjacent fallback cases, deterministic native
goldens, malformed and long-input robustness tests, `make check`, and
`make msrv-check`.

**Exit criteria:** `to_f64` is correctly rounded for every accepted decimal,
rejects exactly the frozen syntax/range failures, preserves signed zero, and
uses no host parser or runtime conversion service.

### TXT4 — Implement shortest round-tripping binary64 formatting and primitive line output

**Purpose:** Complete deterministic user-facing formatting only after the
inverse operation and its boundary corpus can validate every emitted
candidate, then expose canonical primitive line output by composing those
conversions with standard output.

- [x] Implement NaN and infinity spelling in the `Str` facade, finite and
      signed-zero formatting in the companion module, and no promise of NaN
      payload or sign preservation.
- [x] Emit the frozen shortest finite decimal with nearest/even tie selection,
      plain/scientific threshold, decimal point, uppercase exponent marker,
      and exponent spelling.
- [x] Keep the algorithm independent of host locale and formatting libraries;
      use no runtime conversion helper.
- [x] Verify finite output by exact binary64 identity after `to_f64`, not by
      approximate floating comparison.
- [x] Add exhaustive strategically bounded bit-pattern sweeps plus a stable
      generated corpus spanning every exponent class and significand edge.
- [x] Add type-named primitive `std::io::println_<type>` conveniences that
      compose `Str.from_<type>`, exact stdout writing, and one ASCII line feed
      without adding compiler or runtime behavior.

**Tests:** Special values, powers and neighbors, normal/subnormal boundaries,
largest finite values, halfway selections, corpus round trips, output grammar
checks, every primitive line helper and numeric extrema through exact stdout,
determinism, `make check`, and `make msrv-check`.

**Exit criteria:** Every `f64` category has its frozen spelling, every finite
datum round trips bit-identically, output is shortest under the contract, every
primitive has exact LF-terminated standard output, and no compiler or runtime
formatting behavior is involved.

### TXT5 — Adopt string I/O and retire scalar runtime observation

**Purpose:** Remove the bootstrap boundary only after ordinary standard-library
conversion and exact `Str` output cover its useful repository roles.

- [x] Migrate samples and source-to-native tests that observe primitive values
      to `std::io::println_<type>` where canonical line output is intended, or
      to `Str.from_<type>` plus exact `std::io::write_stdout` where composition
      requires explicit control.
- [x] Preserve low-level compiler/backend tests that need scalar ABI probes by
      replacing their observations with exit status, memory-visible behavior,
      assembly inspection, or the new standard-library path as appropriate.
- [x] Remove the five `ska_rt_println_*` declarations, implementations,
      headers, direct harnesses, link dependencies, and stale documentation.
- [x] Advance the runtime ABI and link marker once, retaining allocation,
      panic, and byte I/O behavior unchanged.
- [x] Audit the repository for remaining scalar observation symbols and prove
      exact stdout/stderr and deterministic execution through ordinary I/O.

**Tests:** Focused migrated goldens and runtime contract tests; stale-runtime
link mismatch; symbol/assembly audit; `make check`; `make msrv-check`.

**Exit criteria:** User and test source formats primitives in Skald and writes
`Str` through `std::io`; no public C scalar print helper remains; the runtime
ABI contains no primitive text conversion; and all repository gates pass.

## Ordering and dependencies

The frozen contract precedes all code. Boolean and integer formatting lands
before parsing to establish shared byte-production helpers without coupling
them to failure handling. Simpler optional parsers establish the failure
discipline before binary64 parsing introduces wide decimal arithmetic.
Binary64 parsing precedes shortest formatting so emitted candidates can be
checked against the exact inverse operation. Runtime observation remains until
all conversions and their `std::io` replacement path are executable, making
its final removal a compatibility change rather than an implementation
prerequisite.
