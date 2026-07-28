# Primitive Integer Casts and Comparisons Roadmap

Status: in progress; INT0–INT2 are complete and INT3 is next.

This roadmap establishes a coherent integer-only operator profile before
ordinary standard-library strings depend on isolated numeric operations. It
adds explicit total casts among Skald's fixed-width integer types and complete
same-type integer equality and ordering without introducing implicit numeric
conversion, floating conversion, checked primitive casts, or string-specific
compiler rules.

## Scope and invariants

- The primitive integer types in this profile are exactly `i64`, `u64`, and
  `u8`. `bool` remains nonnumeric, and `f64` remains outside this roadmap.
- The comparison operators `==`, `!=`, `<`, `<=`, `>`, and `>=` accept two
  operands of the same exact primitive integer type and produce `bool`.
- `i64` ordering is signed. `u64` and `u8` ordering is unsigned. Equality
  compares the complete value and is independent of signed ordering.
- Comparisons do not promote operands, reinterpret literals from context, or
  accept mixed integer types. A programmer must cast explicitly before a
  cross-type comparison.
- Ungrouped comparison chains are rejected. Separate comparisons must be
  grouped and combined by a future explicitly designed logical operation.
- Primitive integer casts use unary `(T) source` syntax, where `T` and the
  source type are each one of `i64`, `u64`, and `u8`. Same-type casts are legal
  identities.
- Integer casts are total, pure value operations. They do not diagnose,
  terminate, allocate, call the runtime, or create an exceptional control-flow
  edge for any source value.
- Signed integers use a fixed-width two's-complement bit model for conversion.
  Casting to an `N`-bit integer retains the low `N` bits and interprets those
  bits using the target signedness. Equivalently, the result is determined
  modulo 2^N before target interpretation.
- The conversion-visible bit model is a portable language rule. It does not
  expose target memory layout, endianness, register choice, or external ABI.
- Cast and comparison operands evaluate exactly once in existing left-to-right
  expression order. Neither operation changes ownership or lifetime.
- HIR and MIR record the selected integer operand/source and result/target
  types explicitly. Comparison operations have an integer operand type and a
  `bool` result type; lower phases never infer signedness from source spelling.
- MIR represents integer casts as ordinary pure rvalues, never as hidden
  backend traps or conservatively marked control effects. Verification proves
  the complete source/target matrix before target lowering.
- The x86-64 implementation produces canonical boolean results and preserves
  the public C runtime ABI. Same-width casts may select no machine operation,
  widening may reuse canonical zero extension, and narrowing may mask or use
  the low byte.
- Floating-point casts and comparisons, numeric/boolean casts, implicit
  conversions, mixed-type comparisons, checked or saturating primitive
  conversions, user-defined conversions, logical operators, and object
  equality are non-goals.

The integer cast matrix is:

| Source | `i64` target | `u64` target | `u8` target |
|---|---|---|---|
| `i64` | identity | preserve all 64 bits | retain the low 8 bits |
| `u64` | preserve all 64 bits and interpret the sign bit | identity | retain the low 8 bits |
| `u8` | zero-extend | zero-extend | identity |

Consequently, representative required observations include:

```ska
(i64) 18446744073709551615u // -1
(u64) -1                   // 18446744073709551615u
(u8) 258u                  // 2u8
(u8) -1                    // 255u8
```

## Repository gates

Every task runs its focused tests and `make check`. Tasks that change accepted
syntax, Rust phase models, public facades, or backend code also run
`make msrv-check`. The closing task additionally runs `make robustness-long`,
`git diff --check`, and the full gates from an artifact-free snapshot or clean
checkout.

Tests stay with their owning layer: lexer, parser, resolution, type checking,
HIR/MIR dumps, verification, and backend-unit tests remain colocated; public
cross-phase Rust behavior belongs in the compiler integration-test directory;
and complete success, compile-failure, and native observations belong in the
top-level golden corpus. The C runtime suite changes only to prove that its
public ABI remains unchanged.

## Progress

- [x] INT0 — Freeze the integer operation contract
- [x] INT1 — Establish verified target-independent integer comparisons
- [x] INT2 — Execute integer comparisons on x86-64
- [ ] INT3 — Establish verified target-independent integer casts
- [ ] INT4 — Execute integer casts on x86-64
- [ ] INT5 — Harden and promote the integer operation profile

## PR-sized implementation sequence

### INT0 — Freeze the integer operation contract

**Purpose:** Put the agreed source-visible semantics and compiler invariants in
their authoritative living documents before implementation representations
depend on them.

- [x] Update the primitive language contract and status matrix with the exact
      comparison surface, explicit cast matrix, two's-complement/modulo rule,
      totality, evaluation order, and exclusions without claiming compiler
      availability.
- [x] Record the intended precedence: primitive casts retain unary precedence,
      one non-associative comparison level follows arithmetic, and contextual
      `is` remains weaker.
- [x] Record phase ownership: syntax preserves source shape, type checking
      selects exact integer operations, MIR verifies operand and result types,
      and backends realize already selected signedness and width.
- [x] Keep checked, saturating, floating, boolean/numeric, mixed-type, and
      user-defined conversions explicitly deferred rather than leaving their
      behavior inferable from integer casts.

**Tests:** Documentation checker and link tests; `make docs-check`,
`make check`, and `git diff --check`.

**Exit criteria:** Living language and compiler documentation freeze one
unambiguous integer comparison and cast profile while the status matrix still
describes it as not yet implemented.

### INT1 — Establish verified target-independent integer comparisons

**Purpose:** Carry every same-type integer comparison from source syntax into
verified MIR before any target chooses condition codes.

- [x] Lex `==`, `!=`, `<`, `<=`, `>`, and `>=` by longest match without
      disturbing assignment `=`, postfix `!`, arrows, or malformed-token
      recovery.
- [x] Parse one non-associative comparison level below arithmetic and above
      contextual `is`; retain operator and operand spans, deterministic dumps,
      common nesting limits, and focused recovery.
- [x] Preserve comparison shape through resolution without name lookup or
      target assumptions.
- [x] Require two identical `i64`, `u64`, or `u8` operands, report both actual
      types for invalid pairs, and produce typed `bool` HIR for every valid
      predicate.
- [x] Represent predicate and integer operand kind cohesively rather than
      adding string-motivated special cases. Expose explicit operand and result
      type queries to HIR/MIR consumers and public facades.
- [x] Lower left then right into a target-independent MIR comparison rvalue,
      preserve existing spill rules for a control-affecting right operand, and
      dump stable signedness-independent operation names such as `lt.u64`.
- [x] Verify predicate legality, matching operand definitions and types,
      canonical `bool` result type, block-local value use, and deterministic
      error accumulation under one-invariant MIR mutations.

**Tests:** Longest-token and invalid-token lexer tests; parser precedence,
grouping, chain rejection, recovery, nesting, AST/resolved dump tests;
type-check matrices for all eighteen valid combinations plus mixed, boolean,
floating, class, optional, and unit rejection; HIR/MIR dump and verifier
mutation tests; evaluation-order tests; `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Every valid same-type integer comparison has deterministic
typed HIR and verified target-independent MIR with an explicit integer operand
type and `bool` result, while invalid pairs fail before HIR.

### INT2 — Execute integer comparisons on x86-64

**Purpose:** Realize the verified comparison profile with canonical booleans
and correct signedness without leaking target conditions into MIR.

- [x] Extend the private machine model with cohesive condition-setting support
      for equality, inequality, signed ordering, and unsigned ordering rather
      than predicate-specific ad hoc instructions.
- [x] Select signed conditions for `i64`, unsigned conditions for `u64` and
      canonical `u8`, and zero-extend every condition result before storing a
      `bool`.
- [x] Extend target legality and backend errors so every verified comparison is
      accepted and malformed MIR remains rejected at the verifier boundary.
- [x] Preserve deterministic labels, assembly, frame behavior, evaluation
      order, and the unchanged runtime header and ABI version.

**Tests:** Instruction emission and selector tests for every predicate and
integer kind; signed minimum/negative/zero/maximum boundaries; unsigned values
on both sides of the sign bit and `u64::MAX`; `u8` zero/127/128/255; canonical
boolean storage and branching; malformed-MIR backend rejection; assembly
acceptance and native goldens; runtime ABI assertions; repository gates.

**Exit criteria:** All same-type integer comparisons execute correctly through
the public x86-64 backend and produce canonical `bool` without runtime support.

### INT3 — Establish verified target-independent integer casts

**Purpose:** Add one complete explicit integer-cast matrix as pure typed
operations before target code relies on shared register widths.

- [ ] Recognize primitive-keyword cast targets without changing nominal
      object-cast or shared-cast lookup, precedence, postfix grouping, source
      spans, nesting accounting, or recovery.
- [ ] Preserve the primitive target and source expression through syntax and
      resolution; type checking selects the source and target integer kinds
      once and rejects floating, boolean, unit, optional, array, class, and
      object-view combinations.
- [ ] Accept all nine integer source/target pairs, including identities, with
      no expected-type inference or implicit use at initialization, argument,
      return, assignment, arithmetic, or comparison boundaries.
- [ ] Record one cohesive HIR/MIR integer-cast operation carrying enough
      source/target information to verify identity, bit preservation,
      truncation, and zero extension without inspecting source spelling.
- [ ] Lower casts as ordinary pure rvalues that evaluate their operand once,
      add no block or termination edge, and are not classified as control
      effects.
- [ ] Verify exact source and result types, the closed integer matrix,
      block-local use, and deterministic mutation diagnostics; expose only the
      minimal public phase vocabulary and deterministic dumps.
- [ ] Keep constant operands semantically identical to runtime operands:
      after a literal is valid for its own spelling-selected source type, its
      explicit cast is never a new range diagnostic.

**Tests:** Primitive/object cast disambiguation, nested casts, unary and postfix
precedence, grouping, recovery, common nesting limits, AST/resolved dumps;
all nine type-check/HIR/MIR cases; rejection of implicit, mixed comparison,
floating, boolean, optional, array, class, `Obj`, and `unit` conversions;
representative negative and high-bit dumps; verifier mutation and
evaluation-once tests; repository gates.

**Exit criteria:** Every explicit primitive integer cast becomes one total,
pure, verified MIR value operation, and no unsupported source/target pair
reaches HIR.

### INT4 — Execute integer casts on x86-64

**Purpose:** Realize the portable cast matrix using the simplest correct target
operations while proving that no cast can fail or enter the runtime.

- [ ] Select identity or bit-preserving moves for `i64`/`u64` same-width casts,
      canonical zero extension for `u8` widening, and low-byte masking for
      `i64`/`u64` narrowing to `u8`.
- [ ] Reuse the backend's canonical scalar load/store boundary so cast results
      remain valid across locals, fields, calls, returns, temporaries, and
      subsequent comparisons.
- [ ] Add target-legality coverage and structured rejection for malformed MIR
      without adding a cast-specific trap, branch, helper, symbol, allocation,
      or public runtime ABI change.
- [ ] Prove exact modulo observations around zero, sign transitions, low-byte
      boundaries, and both 64-bit extrema through native execution.

**Tests:** Selector and assembly-shape tests for all nine pairs; identities,
`-1`, `i64::MIN`, `i64::MAX`, `u64` values around `i64::MAX`, `u64::MAX`,
`u8` 0/127/128/255, and narrowing values with significant discarded bits;
argument/result/field/temporary compositions; absence of trap, comparison, and
runtime-call instructions for casts themselves; native goldens and ABI
assertions; repository gates.

**Exit criteria:** Every verified integer cast executes with the frozen
two's-complement/modulo result and cannot terminate or call the runtime.

### INT5 — Harden and promote the integer operation profile

**Purpose:** Close the prerequisite with complete matrices, current
documentation, and no rollout-only structure before string-library work
depends on it.

- [ ] Complete independent-process diagnostic, phase-dump, assembly, and native
      determinism coverage for comparisons and casts.
- [ ] Add source-to-native tests composing range checks with casts in the shape
      required by public string byte and slice methods, including unsigned
      values above `i64::MAX` that are rejected by the comparison before their
      cast result can be used as an array position.
- [ ] Confirm the existing array maximum-length invariant makes every
      range-validated string backing position numerically representable as
      `i64`; do not add a checked cast or string-specific numeric rule.
- [ ] Audit touched lexer, syntax, resolution, type-check, HIR, MIR, verifier,
      backend, facade, dump, and test owners by responsibility; resolve
      high-priority hotspots and index lower-priority discoveries separately.
- [ ] Update grammar, language/compiler overviews, status, debugging/testing
      guidance, string dependency wording, and cross-links to describe only
      current implemented behavior.
- [ ] Remove rollout vocabulary from living code and documentation, confirm
      runtime ABI and artifact cleanliness, complete this roadmap, and archive
      it.

**Tests:** Full valid/invalid comparison and cast matrices; focused
documentation/link tests; independent-process determinism; compile-failure and
native golden suites; `make check`, `make msrv-check`,
`make robustness-long`, `git diff --check`, and an artifact-free full gate.

**Exit criteria:** Explicit total integer casts and exact-type integer
comparisons are implemented deterministic source-to-native contracts, all
living documentation is current, and the prerequisite is archived before
ordinary standard-library string behavior resumes.

## Ordering and dependencies

The contract lands first because operator precedence, cast totality, and the
closed matrices constrain every later representation. Target-independent
comparisons land before x86-64 condition selection, and target-independent
casts land before a backend may exploit same-width representation. Closing
hardening follows only after both operation families are observable through
native execution.

This roadmap depends on the implemented fixed-width primitive types, exact-type
expression checking, canonical `u8` and `bool` representation, left-to-right
evaluation, verified block-local MIR values, and the x86-64 private machine
model. It changes no ownership, cleanup, array, object-cast, external-signature,
or public runtime contract.

The roadmap is a prerequisite for
[ordinary standard-library string behavior](STRINGS_ROADMAP.md). That work
uses matching `u64` `<` and `<=` comparisons to validate public byte and slice
bounds, then uses the total `u64`-to-`i64` cast only after existing string
descriptor and array maximum-length invariants establish numeric
representability. Floating conversion, checked primitive conversion, and
mixed-type comparison are not required by strings and do not block that work.
