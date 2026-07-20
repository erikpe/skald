# Remaining Primitive Types Roadmap

Status: T0–T7 complete; roadmap complete.

This roadmap adds Skald's remaining primitive value types: `u64`, `u8`, and
`f64`. The floating-point type is named `f64`, not `double`; `double` remains
available as an ordinary identifier. The work is split into reviewable,
PR-sized tasks that keep the compiler buildable and preserve existing source
behavior after every task.

This is a good next slice because it exercises type representation, literal
handling, fixed-width arithmetic, storage, calls, target ABI classification,
and floating-point code generation without yet introducing object layout,
ownership, collections, or new control-flow syntax.

The completed slice should compile programs such as:

```ska
extern fn ska_rt_println_u64(value: u64) -> unit;
extern fn ska_rt_println_u8(value: u8) -> unit;
extern fn ska_rt_println_f64_bits(value: f64) -> unit;

fn scale(value: f64) -> f64 {
    return value * 2.0;
}

fn main() -> i64 {
    ska_rt_println_u64(18446744073709551615u);
    ska_rt_println_u8(255u8);
    ska_rt_println_f64_bits(scale(1.5));
    return 0;
}
```

with exact stdout:

```text
18446744073709551615
255
0x4008000000000000
```

The bit-oriented `f64` operation is deliberately a bootstrap observability
facility, not a user-facing floating-point formatter. It gives golden tests an
exact, locale-independent representation and preserves distinctions such as
positive versus negative zero without prematurely specifying decimal display.

## 1. Scope and Design Constraints

### Included

- primitive types `u64`, `u8`, and IEEE-754 binary64 `f64`;
- use in parameters, results, initialized locals, expressions, calls, returns,
  and the restricted exact-symbol external-function profile;
- decimal `u64` and `u8` literals with `u` and `u8` suffixes;
- decimal `f64` literals containing a decimal point or exponent;
- exact type checking with no implicit numeric conversions or promotions;
- the existing arithmetic surface: binary `+`, `-`, and `*` for matching
  numeric types, plus unary `-` for `f64`;
- explicit MIR types, constants, operations, loads, stores, calls, and returns;
- Linux x86-64 System V integer and SSE scalar ABI lowering, including mixed
  signatures;
- canonical zero-extension for all compiler-visible `u8` values;
- bootstrap runtime output for `u64`, `u8`, and raw `f64` bits;
- deterministic dumps, exact diagnostics, assembly-shape tests, and native
  golden coverage.

### Explicitly excluded

- implicit conversions, contextual literal typing, or usual arithmetic
  conversions;
- explicit casts between primitive types;
- equality, ordering, logical operators, bitwise operators, shifts,
  exponentiation, division, or remainder;
- hexadecimal, octal, binary, digit-separated, suffixed `f64`, `NaN`, or
  infinity literal syntax;
- a final decimal floating-point formatting or parsing library;
- compile-time constant folding or algebraic simplification;
- SIMD/vector values, extended precision, selectable rounding modes, or
  floating-point exception flags;
- changing the required entry signature `fn main() -> i64`;
- arrays, strings, objects, methods, ownership-bearing values, or general FFI;
- AArch64 lowering.

These features should receive later contracts. In particular, primitive casts
and comparisons cross semantic boundaries that deserve focused roadmaps rather
than entering as incidental helpers for tests.

### Literal contract

- An unsuffixed decimal integer remains an `i64` literal.
- `0u` through `18446744073709551615u` have type `u64`.
- `0u8` through `255u8` have type `u8`.
- A decimal form with a decimal point or exponent has type `f64`, for example
  `0.0`, `1.5`, `6.25e-1`, or `2e3`.
- A decimal point requires digits on both sides. Forms such as `.5` and `1.`
  are rejected initially.
- An exponent is `e` or `E`, an optional `+` or `-`, and one or more decimal
  digits.
- A leading `-` remains a unary operator rather than part of any literal.
- Integer suffixes are case-sensitive. `u` is the canonical concise suffix for
  `u64`; `u64` is not accepted as a literal suffix. The narrower type keeps the
  explicit `u8` suffix so its width remains visible.
- Integer bounds are checked during type checking, not lexing.
- A finite decimal `f64` literal is converted to the correctly rounded nearest
  binary64 value using round-to-nearest, ties-to-even. Literal overflow to
  infinity is diagnosed. Subnormal values and underflow to signed zero follow
  binary64 conversion.
- HIR and MIR preserve an `f64` constant as its raw 64 bits. Dumps render those
  bits as exactly 16 lowercase hexadecimal digits rather than relying on host
  decimal formatting.

Malformed numeric-looking text is consumed as one invalid token where
possible, so diagnostics remain focused for bad suffixes, incomplete
exponents, repeated decimal points, and identifier tails.

### Arithmetic contract for this slice

Operands must have exactly the same numeric type; result type equals operand
type. No operator causes an implicit widening or signedness conversion.

- `u64` addition, subtraction, and multiplication wrap modulo `2^64`.
- `u8` addition, subtraction, and multiplication wrap modulo `2^8`, and the
  produced value is canonicalized to `0..=255` before it becomes visible to
  another MIR operation, storage location, call, or return.
- `f64` addition, subtraction, multiplication, and unary negation follow
  IEEE-754 binary64 behavior with the target's default round-to-nearest,
  ties-to-even environment. Infinities, signed zeroes, subnormals, and NaNs are
  runtime values even though this slice adds no direct infinity or NaN literal.
- `u64` and `u8` do not support unary minus.

The existing `i64` overflow contract remains outside this roadmap. Adding
unsigned modular arithmetic must not silently redefine signed arithmetic.

### ABI and representation contract

- Skald `u64`, `u8`, and `f64` map to C `uint64_t`, `uint8_t`, and `double` in
  the restricted external profile. The C implementation must be binary64 for
  the target to support Skald `f64`.
- `u64` and `u8` use the System V integer class; `f64` uses the SSE class.
- Integer and SSE argument-register allocation use independent counters:
  `%rdi` through `%r9` for integer-class values and `%xmm0` through `%xmm7` for
  `f64` values. Mixed signatures must not use one shared positional register
  index.
- `u64` returns in `%rax`, `u8` returns in `%al` and is zero-extended at the
  Skald boundary, and `f64` returns in `%xmm0`.
- The initial stack-heavy backend may give each scalar an eight-byte home.
  This does not make `u8` an eight-byte language value.
- Internal and external incoming `u8` values are canonicalized before entering
  general MIR-visible use. Outgoing `u8` values are canonical by construction.
- MIR remains target-independent: it records semantic scalar types and
  operations, never XMM registers or C type names.

### Architectural rules

1. Every primitive remains a distinct type in resolved IR, HIR, and MIR.
2. Numeric behavior is selected in type checking and represented by explicit
   typed HIR/MIR operations; the backend never infers it from source spelling.
3. Literal parsing is centralized. Later phases must not inspect suffix text to
   rediscover a literal's type.
4. Floating constants cross IR boundaries as raw binary64 bits, avoiding host
   `f64` equality, hashing, or formatting in deterministic structures.
5. ABI classification is represented as a complete call layout with separate
   integer, SSE, and stack locations, not scattered index tests.
6. Width normalization is explicit at the operation or ABI boundary and tested
   as an invariant; it is not an accidental consequence of register choice.
7. Runtime output functions are ordinary exact-symbol externals. No compiler
   phase recognizes their names.
8. Public source syntax is enabled only when its complete path through the
   supported x86-64 target exists.
9. A later backend must either implement every new MIR scalar operation or
   reject it through structured target legality.

## 2. Progress Summary

- [x] T0 — Freeze primitive, literal, arithmetic, and ABI contracts
- [x] T1 — Add direct runtime observability for the new primitives
- [x] T2 — Refactor numeric literal infrastructure without changing behavior
- [x] T3 — Implement `u64` end-to-end
- [x] T4 — Implement `u8` end-to-end with explicit canonicalization
- [x] T5 — Add target-independent and backend `f64` infrastructure
- [x] T6 — Enable `f64` source syntax and semantics end-to-end
- [x] T7 — Complete golden coverage and harden the primitive slice

Milestone checkboxes below should be marked as implementation progresses. A
task is complete only when its acceptance criteria and relevant quality gates
pass.

## 3. PR-Sized Implementation Tasks

### T0 — Freeze primitive, literal, arithmetic, and ABI contracts

**Purpose:** Remove semantic ambiguity before adding the same concepts to every
compiler phase.

- [x] Rename the language type `double` to `f64` throughout the Skald
      specification; document that the restricted C ABI maps it to C `double`.
- [x] Add the literal grammar and range/conversion rules from this roadmap to
      `grammar/README.md` and `docs/SKALD_DRAFT_SPEC.md`.
- [x] Specify exact typing, no implicit promotion, unsigned modular arithmetic,
      `u8` canonicalization, and binary64 arithmetic behavior.
- [x] Specify the mixed integer/SSE System V argument and return contract.
- [x] Specify the exact bootstrap output bytes and runtime ABI version change.
- [x] Reconcile primitive defaults, operators, casts deferred from this slice,
      and primitive edge-case gaps in the draft.
- [x] Record the deliberate exclusions and confirm `main` remains `i64`.

**Tests:** Manual cross-document review against the x86-64 System V ABI and the
current compiler/runtime interfaces. No implementation behavior changes.

**Acceptance criteria:** Literal spelling, type identity, arithmetic, width
normalization, ABI behavior, observability, and exclusions have one normative
contract usable by every later task.

### T1 — Add direct runtime observability for the new primitives

**Purpose:** Make exact values observable before compiler support can produce
them, keeping runtime correctness independent of frontend and backend work.

- [x] Add `ska_rt_println_u64(uint64_t)` with shortest unsigned decimal output.
- [x] Add `ska_rt_println_u8(uint8_t)` with unsigned decimal output in
      `0..=255`.
- [x] Add `ska_rt_println_f64_bits(double)` that writes `0x` followed by exactly
      16 lowercase hexadecimal digits encoding the binary64 representation.
- [x] Extract `f64` bits with `memcpy` or another alias-safe method and assert
      the runtime target's required binary64 properties at compile time.
- [x] Reuse the checked complete-record output boundary without duplicating
      error handling or exposing libc implementation types publicly.
- [x] Increment `SKALD_RUNTIME_ABI_VERSION` from 3 to 4 once for the three new
      symbols.
- [x] Extend the direct C harness for zeroes, extrema, consecutive calls,
      positive and negative zero, representative finite bit patterns, and
      detected output failure for every new operation.
- [x] Update runtime ABI and test documentation.

**Tests:** `make runtime-test` under C11 with warnings denied, exact captured
bytes, header/archive version agreement, binary64 compile-time assertions, and
failure-path child processes.

**Acceptance criteria:** Direct C callers observe exact specified records for
all three types; runtime errors cannot return as success; no Skald compiler
support is needed to validate the ABI.

### T2 — Refactor numeric literal infrastructure without changing behavior

**Purpose:** Create one maintainable literal pipeline before adding several
spellings and types, while keeping the accepted language exactly unchanged.

- [x] Replace integer-specific lexer/parser plumbing with an explicit numeric
      token/literal representation that can distinguish integer suffixes and
      decimal floating forms without downstream string heuristics.
- [x] Centralize numeric scanning and malformed-tail recovery.
- [x] Keep only the existing unsuffixed decimal `i64` spelling enabled in this
      task; `u64`, `u8`, and `f64` forms must remain focused unsupported or
      malformed-source diagnostics until their end-to-end tasks.
- [x] Preserve original spelling and complete spans for diagnostics while
      keeping semantic numeric values out of the lexer.
- [x] Preserve the existing `i64::MIN` unary-minus normalization behavior.
- [x] Keep AST/resolved/HIR/MIR dumps and all existing diagnostics
      deterministic. Malformed exponent recovery deliberately keeps an
      exponent sign in the invalid token, so `1e+` and `2E-foo` receive one
      complete-span lexical diagnostic.
- [x] Remove superseded integer-only helpers rather than leaving parallel
      literal paths.

**Tests:** Lexer boundaries and malformed forms, parser recovery, `i64` range
tests, exact dump snapshots, exact compile-failure goldens, and the complete
existing suite.

**Acceptance criteria:** Numeric syntax has one extensible representation and
scanner, no later phase classifies types from suffix strings, and no previously
valid or invalid source changes behavior accidentally.

### T3 — Implement `u64` end-to-end

**Purpose:** Add the full-width unsigned type by extending the existing integer
path before introducing narrow-width or floating-point concerns.

- [x] Enable the `u64` keyword and `digits u` literal suffix.
- [x] Preserve `u64` types and literal magnitude through AST and resolved dumps.
- [x] Add `u64` to semantic types and typed HIR with exact initializer,
      argument, return, and operator checking.
- [x] Diagnose literal overflow and every implicit `i64`/`u64` mismatch.
- [x] Add explicit `AddU64`, `SubtractU64`, and `MultiplyU64` operations.
- [x] Add `u64` constants, storage, loads, stores, calls, and returns to MIR and
      its verifier.
- [x] Lower `u64` operations and System V integer-class parameters/results on
      x86-64 without signed-only assumptions.
- [x] Extend the restricted external profile to C `uint64_t`.
- [x] Add the ordinary source declaration/call path for
      `ska_rt_println_u64`.

**Tests:** Lexer/parser and range diagnostics; resolution/HIR/MIR dumps; exact
typing failures; verifier mutation tests; backend assembly shape; boundary and
wrapping arithmetic; internal and external calls including register/stack
arguments; native exact-output goldens.

**Acceptance criteria:** Programs can declare, calculate, pass, return, and
print every `u64` value through the complete pipeline. Unsuffixed integers stay
`i64`, mixed arithmetic is rejected, and source/compiler output is
deterministic.

### T4 — Implement `u8` end-to-end with explicit canonicalization

**Purpose:** Add the byte type while making narrow-value invariants explicit
enough for future arrays, strings, and additional backends.

- [x] Enable the `u8` keyword and `digits u8` literal suffix.
- [x] Add `u8` to semantic IR and exact type checking; diagnose values above
      255 and all implicit widening or narrowing.
- [x] Add explicit `AddU8`, `SubtractU8`, and `MultiplyU8` operations.
- [x] Add `u8` constants, storage, calls, returns, and verifier invariants.
- [x] Define one centralized MIR/backend policy for truncating arithmetic to
      eight bits and zero-extending it into general value homes.
- [x] Canonicalize incoming internal and external parameters/results rather
      than trusting unspecified upper register bits.
- [x] Extend the restricted external profile to C `uint8_t`.
- [x] Add the ordinary source declaration/call path for `ska_rt_println_u8`.
- [x] Avoid proliferating byte-specific instruction selection throughout
      unrelated backend code; keep width handling behind typed helpers.

**Tests:** Literal bounds `0`, `255`, and `256`; exact mismatch diagnostics;
all wrapping arithmetic edges; repeated arithmetic proving canonicalization;
register and stack calls; external return normalization; MIR corruption tests;
assembly shape; and exact native output.

**Acceptance criteria:** Every observable `u8` is in `0..=255`, arithmetic wraps
modulo 256, ABI boundaries cannot introduce noncanonical upper bits, and the
implementation provides a reusable narrow-scalar policy for future work.

### T5 — Add target-independent and backend `f64` infrastructure

**Purpose:** Establish binary64 MIR and mixed-class ABI lowering before source
syntax can depend on it, following the control-flow slice's backend-first
pattern.

- [x] Add `f64` to MIR types using raw-bit constants and explicit add,
      subtract, multiply, and negate operations.
- [x] Extend MIR verification for floating storage, values, calls, returns, and
      operation signatures.
- [x] Refactor x86-64 call layout to classify integer and SSE arguments with
      independent register counters and deterministic stack locations.
- [x] Add XMM argument/result and caller-saved scratch representation to the
      target machine model without leaking registers into MIR.
- [x] Lower raw-bit constants, loads, stores, arithmetic, calls, and returns
      using scalar SSE2 instructions.
- [x] Preserve 16-byte call-site alignment for mixed register/stack signatures.
- [x] Extend target legality and structured error handling for every new MIR
      form.
- [x] Keep source `f64` syntax disabled until T6 completes the frontend path.

**Tests:** Hand-built verified MIR for raw constants and arithmetic; mixed
integer/SSE signatures that independently exhaust both register banks; stack
arguments; internal/external returns; exact assembly shape; assembler
acceptance; malformed MIR rejection; and native execution through a small
backend harness.

**Acceptance criteria:** The backend correctly executes verified `f64` MIR and
mixed scalar calls, existing integer-only assembly remains stable apart from
intentional call-layout refactoring, and no frontend source construct is
partially enabled.

### T6 — Enable `f64` source syntax and semantics end-to-end

**Purpose:** Connect the already-supported MIR/backend path to a precise source
literal and semantic model.

- [x] Enable the `f64` keyword and specified decimal-point/exponent literal
      grammar with focused malformed-literal recovery.
- [x] Convert valid finite literals once at the semantic boundary and preserve
      raw binary64 bits below it.
- [x] Diagnose literal overflow and reject non-language spellings such as
      `NaN`, `inf`, `.5`, `1.`, and `1.0f64` without cascades.
- [x] Add `f64` to resolved IR and typed HIR, including explicit add, subtract,
      multiply, and negate operations.
- [x] Enforce exact types for locals, calls, returns, and arithmetic; do not
      infer `f64` from an expected type or promote integers.
- [x] Extend the restricted external profile to C `double` and add the ordinary
      declaration/call path for `ska_rt_println_f64_bits`.
- [x] Render floating constants deterministically as raw bits in HIR and MIR
      dumps.
- [x] Exercise `f64` inside conditional arms without adding floating
      comparisons or truthiness.

**Tests:** Lexer/parser boundaries and recovery; rounding-sensitive literals;
positive/negative zero; subnormal, maximum finite, overflow, and underflow
cases; exact type failures; deterministic dumps; MIR lowering; mixed-signature
backend integration; and native raw-bit output for binary-exact arithmetic.

**Acceptance criteria:** Source programs can declare, calculate, pass, return,
and exactly observe binary64 values; integers never promote implicitly; dumps
and diagnostics are host-format-independent; and generated mixed-signature
calls obey System V.

### T7 — Complete golden coverage and harden the primitive slice

**Purpose:** Prove externally observable boundaries and reconcile all public
documentation with the completed implementation.

- [x] Add exact native output for zero, one, maxima, wrapping edges, locals,
      parameters, internal results, external calls, and consecutive mixed
      output operations.
- [x] Cover binary64 positive/negative zero, representative exact fractions,
      arithmetic, call/return flow, and raw-bit observation.
- [x] Cover mixed integer/SSE signatures in register-only, independently
      exhausted, and stack-argument forms.
- [x] Add exact compile-failure goldens for malformed suffixes/floats, every
      literal overflow family, implicit conversions, mixed arithmetic, invalid
      unary minus, and invalid external signatures.
- [x] Confirm repeated compiler processes produce identical assembly and
      diagnostics for all new types.
- [x] Audit type matches, dump renderers, verifier checks, and backend legality
      so adding a future primitive has one obvious set of extension points.
- [x] Update `README.md`, `grammar/README.md`, `docs/REPO_STRUCTURE.md`,
      `docs/DEBUGGING.md`, `docs/NEXT_SLICE_BOUNDARIES.md`, runtime/golden test
      documentation, and the draft specification.
- [x] Record remaining division/remainder, casts, comparisons, decimal
      formatting, floating exceptional behavior, and cross-target questions.
- [x] Run the complete repository quality gates from a clean build state.

**Tests:** All compiler and CLI tests, direct runtime tests, successful and
compile-failure goldens, formatting, Clippy with warnings denied, and
`git diff --check`.

**Acceptance criteria:** All three types are usable through the public pipeline
and exact native observations; width, signedness, binary64, and mixed ABI rules
are proven by tests; every failure family has a stable diagnostic; and the
architecture remains straightforward to extend.

## 4. Required Quality Gates for Every Task

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace --all-targets`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace`
- [x] `make runtime-test` when the runtime or ABI is touched
- [x] `make golden-test` when source behavior, diagnostics, MIR, backend,
      runtime linking, or golden expectations are touched
- [x] `git diff --check`

These global checkboxes represent T7's final clean-build gate. Each earlier
task must run its relevant checks without marking the complete roadmap early.

## 5. Completion Definition

This roadmap is complete when T0–T7 and every quality-gate checkbox are marked,
and the following paths are covered end-to-end:

```text
u64/u8 source types and suffixed literals
  → exact typed operations and modular semantics
  → canonical target-independent MIR values
  → integer-class x86-64 calls and exact native output

f64 source type and decimal literals
  → raw binary64 bits and typed floating operations
  → target-independent floating MIR
  → SSE-class x86-64 calls and exact bit output
```

No semantic phase may infer numeric type from a backend representation, no
backend may infer signedness or width from source spelling, and no runtime
output symbol may be compiler-special-cased.

## 6. Deferred Primitive Work

T7 closes this slice without silently expanding its operator or library
surface. Follow-up roadmaps must settle these concerns before implementing
them:

- integer and floating division and remainder, including zero divisors and
  signed-minimum edge cases;
- explicit casts, their syntax, range failures, and lowering rules;
- integer and floating comparisons, especially unordered NaN behavior;
- user-facing decimal `f64` formatting, which is intentionally distinct from
  the bootstrap raw-bit observer;
- production and propagation of infinity, NaN signs and payloads, and floating
  environment assumptions beyond the currently specified operations;
- per-target binary64 validation, C ABI mapping, mixed-class argument layout,
  and conformance tests, beginning with the planned AArch64 backend.

The draft specification records the corresponding language-level open
questions. None is implicitly answered by the stage-0 x86-64 implementation.
