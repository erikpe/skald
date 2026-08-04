# Hexadecimal Integer and Byte Literals Roadmap

Status: planned; LIT0 is next.

This roadmap adds hexadecimal source spellings for all three Skald integer
types and single-quoted byte literals of exact type `u8`. The design is frozen
here before implementation so lexer recovery, range checking, unary negation,
phase representation, diagnostics, and documentation all converge on one
contract. The existing decimal integer forms and the more complete Skald
`f64` literal contract remain unchanged.

The completed source surface includes these equivalent values:

```ska
var signed_decimal: i64 = 42;
var signed_hexadecimal: i64 = 0x2a;
var unsigned_hexadecimal: u64 = 0X2Au;
var byte_hexadecimal: u8 = 0xffu8;
var byte_character: u8 = 'A';
var byte_escape: u8 = '\n';
var byte_exact: u8 = '\x41';
```

This roadmap is the frozen planned contract. Living grammar, language, compiler,
and status documents continue to describe only implemented behavior until the
task that enables each source form updates them.

## Scope and invariants

### Hexadecimal integer contract

- Extend each existing integer literal kind with a hexadecimal form:

  ```text
  hex-prefix       = "0x" | "0X"
  hex-digit        = "0".."9" | "a".."f" | "A".."F"
  hex-digits       = hex-digit {hex-digit}

  i64-literal      = decimal-digits | hex-prefix hex-digits
  u64-literal      = decimal-digits "u" | hex-prefix hex-digits "u"
  u8-literal       = decimal-digits "u8" | hex-prefix hex-digits "u8"
  ```

- Keep suffixes case-sensitive and unchanged. `u` selects `u64`, `u8` selects
  `u8`, and unsuffixed integers select `i64`; `u64`, `U`, and `U8` are not
  literal suffixes.
- Interpret every hexadecimal spelling as a non-negative mathematical
  magnitude, not as a source-level two's-complement bit pattern. Unsuffixed
  `0xffffffffffffffff` is therefore out of range for `i64`, not `-1`.
- Perform integer range checking during type checking with the same inclusive
  ranges and diagnostic families as decimal integers. Radix does not change a
  literal's type, overflow policy, operator behavior, or exact-type rules.
- Keep leading `-` as a unary operator. The existing grouped minimum-boundary
  exception also accepts `-0x8000000000000000`, including redundant grouping,
  while positive `0x8000000000000000` and magnitudes beyond the minimum
  boundary remain out of range for `i64`.
- Preserve the complete original spelling and its explicit radix through
  tokens, source-shaped AST, and resolved IR. Type checking converts it once
  to the existing typed HIR integer constants; MIR, verification, backend,
  layout, ABI, runtime, arithmetic, casts, and formatting remain unchanged.
- Represent radix structurally rather than rediscovering it from spelling in
  consumers. Refactor `NumericLiteralKind` into invalid-state-free integer
  variants carrying `IntegerRadix::{Decimal, Hexadecimal}`, while `F64`
  remains a distinct radix-free variant.
- Consume an invalid hexadecimal-looking run as one invalid token where the
  existing numeric-tail recovery permits it. This includes missing digits,
  invalid digits or identifier tails, unsupported suffixes, separators, and
  decimal points, such as `0x`, `0xg`, `0x1g`, `0xffu64`, `0x_ff`, and
  `0x1.0`.

### Byte literal contract

- Add a single-quoted byte literal whose exact type is `u8`; it is not a new
  character type and receives no contextual typing or implicit conversion.
- A direct byte is one printable ASCII byte from `0x20` through `0x7e`, except
  single quote and backslash. Double quote is valid directly.
- Support the exact escapes `\'`, `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, and
  `\xNN`, where `NN` is exactly two case-insensitive hexadecimal digits.
  Every accepted spelling therefore decodes to exactly one value in
  `0..=255`.
- Reject empty, multi-byte, direct control-byte, direct non-ASCII, unknown or
  incomplete escape, unescaped newline, and unterminated byte literals with a
  dedicated malformed-byte-literal diagnostic and deterministic recovery.
  Recovery consumes through a closing quote when possible and treats a
  physical newline as a hard boundary, matching the established string
  scanner policy.
- Give byte literals their own token, syntax node, and resolved node with the
  decoded `u8` value and complete span. Syntax and resolved dumps identify the
  source family and render the decoded value as two lowercase hexadecimal
  digits. Type checking then converges on the existing `HirExpressionKind::U8`
  representation, so no byte-literal distinction survives into MIR or the
  backend.
- Keep string literals byte-oriented and unchanged. Shared escape recognition
  or hexadecimal decoding may be factored only where it leaves the distinct
  delimiter, error, recovery, and AST contracts clear.

### Preserved behavior and exclusions

- Preserve the current `f64` grammar and semantics exactly: decimal-point and
  exponent forms, correctly rounded finite binary64 conversion, subnormals,
  underflow to zero, and range rejection for values rounding to infinity.
- Do not add hexadecimal floating literals, floating suffixes, `NaN` or
  infinity spellings, binary or octal integers, digit separators, an `i64`
  suffix, arbitrary integer widths, or contextual numeric literal inference.
- Do not add Unicode scalar, code-point, grapheme, multibyte, or locale
  semantics; a direct non-ASCII character inside single quotes is invalid.
- Do not change string contents, adjacency, interpolation, formatting,
  parsing, or the `std::str::Str` language-item boundary.
- Preserve deterministic tokens, AST, resolved IR, HIR, MIR, diagnostics, and
  assembly. Equivalent decimal, hexadecimal, and byte spellings may differ in
  source-shaped products but must converge to identical typed constants below
  type checking.
- Add no runtime function, ABI version, data layout, target instruction,
  ownership rule, cleanup edge, or backend-specific literal parser.

## Progress

- [ ] LIT0 — Make integer literal representation radix-aware
- [ ] LIT1 — Implement hexadecimal integers end to end
- [ ] LIT2 — Implement single-quoted `u8` byte literals end to end

Every task runs its focused tests, then `make check` and `make msrv-check`.
The Makefile remains the repository automation interface.

## PR-sized implementation sequence

### LIT0 — Make integer literal representation radix-aware

**Purpose:** Establish an explicit, invalid-state-free radix representation
without changing accepted source, diagnostics, dumps, or compiled behavior.

- [ ] Add the public source-literal `IntegerRadix` vocabulary beside
      `NumericLiteralKind`, and make each integer kind carry its radix while
      leaving `F64` radix-free.
- [ ] Thread decimal radix metadata through numeric scanning, tokens, AST,
      syntax dumps, resolution, resolved dumps, overload classification, and
      type checking without asking later consumers to inspect a prefix.
- [ ] Centralize suffix removal, digit selection, radix conversion, and
      magnitude classification in the type-check literal owner. Preserve the
      original full spelling for that one semantic conversion plus
      source-facing dumps and diagnostics; do not use it to infer kind or
      radix.
- [ ] Preserve the grouped `i64::MIN` path, all decimal range diagnostics,
      exact literal types, malformed-tail recovery, token names, and normalized
      HIR/MIR dumps byte for byte.
- [ ] Keep module facades concise and selectively re-export only the literal
      vocabulary required across phase boundaries.
- [ ] Add focused representation and conversion tests proving that every
      currently accepted numeric form carries decimal radix and that arbitrary
      precision is retained until the unary-minus boundary is classified.

**Tests:** `cargo test --locked -p skald-compiler lexer::tests` plus focused
syntax, resolver, and type-check literal tests; existing numeric compile-failure
and native goldens; `make check`; `make msrv-check`.

**Exit criteria:** All existing source behaves identically, every integer
literal carries explicit decimal radix through resolved IR, no semantic
consumer rediscovers radix from spelling, and the full quality gates pass.

### LIT1 — Implement hexadecimal integers end to end

**Purpose:** Enable the frozen hexadecimal spelling for `i64`, `u64`, and
`u8` while reusing every existing typed integer path below type checking.

- [ ] Extend the numeric scanner with `0x`/`0X`, case-insensitive hexadecimal
      digits, the existing lowercase suffixes, longest valid recognition, and
      whole-token malformed-tail recovery.
- [ ] Preserve integer kind, hexadecimal radix, original spelling, and exact
      span through tokens, AST, and resolved IR with deterministic source-shaped
      dumps.
- [ ] Convert suffix-free hexadecimal digits with the centralized radix-aware
      type-check helpers and emit the existing typed HIR constants.
- [ ] Apply existing range diagnostic codes and wording to hexadecimal values,
      retaining the original spelling and type-specific range notes.
- [ ] Extend the unary-minus magnitude rule through grouping for hexadecimal
      `i64::MIN`; reject its positive magnitude and adjacent larger positive
      and negative values.
- [ ] Cover lowercase and uppercase prefixes and digits, all three suffix
      selections, leading zeroes, extrema, overflow, invalid digits, missing
      digits, bad suffixes, separators, decimal points, token boundaries, and
      recovery into a following valid statement.
- [ ] Prove hexadecimal values work through representative arithmetic,
      bitwise operations, shifts, comparisons, casts, calls, returns, storage,
      arrays, and native observation without a new HIR, MIR, verifier, backend,
      runtime, or ABI branch.
- [ ] Update the implemented grammar, type/value contract, phase documentation,
      status matrix, and relevant examples in the same change that enables the
      syntax. Remove `0xff` from lists of malformed examples while preserving
      the still-excluded literal forms.
- [ ] Retain focused regression coverage for every existing `f64` form and for
      the distinction between hexadecimal digits containing `e`/`E` and
      decimal exponent syntax.

**Tests:** Focused lexer numeric, syntax, resolved-dump, type-check literal,
HIR, and native backend tests; exact malformed and out-of-range compile-failure
goldens; one source-to-native hexadecimal literal golden; `make check`;
`make msrv-check`.

**Exit criteria:** Every frozen hexadecimal integer form compiles to the same
typed value as its decimal equivalent, every malformed or out-of-range form
produces a structured source diagnostic without a panic, `i64::MIN` works only
under unary negation, `f64` behavior is unchanged, and no lower phase gains a
source-radix concern.

### LIT2 — Implement single-quoted `u8` byte literals end to end

**Purpose:** Add concise raw-byte source notation while keeping Skald free of
an implied character or Unicode model.

- [ ] Add a cohesive byte-literal scanner and decoder with the frozen direct
      ASCII set, exact escapes, full spans, dedicated diagnostic code and
      wording, and newline/closing-quote recovery.
- [ ] Add the byte-literal token to token names, dumps, expression starts,
      parser recovery, nesting traversal, receiver/place classification, and
      every other exhaustive frontend match.
- [ ] Carry one distinct decoded-byte expression through the source-shaped AST
      and resolved IR, with stable two-digit hexadecimal syntax and resolved
      dumps, then type-check it directly to the existing exact `u8` HIR
      constant.
- [ ] Cover the direct printable-ASCII boundaries and exclusions, all simple
      escapes, `\x00`, mixed-case hexadecimal digits, `\xff`, empty and
      multiple contents, direct control and non-ASCII input, unknown and
      incomplete escapes, newline, EOF, accurate UTF-8 spans, and recovery into
      following declarations and statements.
- [ ] Prove byte literals compose with representative `u8` arithmetic,
      comparisons, casts, calls, returns, fields, arrays, string byte APIs, and
      native output, while mismatches with `i64` and `u64` remain exact-type
      errors.
- [ ] Compare equivalent byte, hexadecimal `u8`, and decimal `u8` spellings
      across normalized HIR, MIR, assembly, and native results, while keeping
      their source-shaped token, syntax, and resolved dumps intentionally
      distinct.
- [ ] Update the implemented grammar, types/value contract, string byte-model
      cross-reference, phase documentation, status matrix, and examples in the
      same change that enables the syntax. Continue to state explicitly that
      Skald has no character or Unicode type.
- [ ] Extend deterministic frontend robustness coverage so arbitrary bytes and
      malformed quote runs cannot panic, loop, or escape their source spans.

**Tests:** Focused lexer byte-scanner, syntax/recovery, resolved-dump,
type-check/HIR, robustness, and backend native tests; exact malformed-byte and
type-mismatch compile-failure goldens; one source-to-native byte-literal golden;
`make check`; `make msrv-check`.

**Exit criteria:** Every accepted single-quoted spelling denotes exactly one
`u8`, all invalid spellings recover with one stable lexical diagnostic family,
lower phases treat the result as an ordinary canonical byte, documentation
contains no character/Unicode ambiguity, and the complete repository and MSRV
gates pass.

## Ordering and dependencies

LIT0 comes first because hexadecimal range checking must consume structural
radix metadata rather than add a second spelling-inspection path. LIT1 then
changes the existing numeric scanner and proves that source radix disappears
at the typed HIR boundary. LIT2 follows so the two changes do not compete in
the lexer, parser recovery sets, and exhaustive expression matches; it is
otherwise semantically independent and converges on the already proven `u8`
constant pipeline.

The roadmap depends only on the implemented primitive literal, operator,
cast, array, string-byte, diagnostics, and native target foundations. It has no
semantic dependency on the planned produced-object alias-argument roadmap and
does not change the runtime ABI.
