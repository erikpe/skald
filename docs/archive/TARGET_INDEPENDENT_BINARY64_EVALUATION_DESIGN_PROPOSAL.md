# Target-Independent Binary64 Evaluation Design Proposal

Status: frozen design; implemented and archived. B64E1 through B64E12 were
confirmed together on 2026-09-06 and delivered by the completed
[implementation roadmap](TARGET_INDEPENDENT_BINARY64_EVALUATION_ROADMAP.md).
The durable crate and compiler-phase boundaries are authoritative in the
living [compiler architecture](../compiler/README.md) and
[phase contract](../compiler/PHASES_AND_IR.md#target-independent-binary64-evaluation).

This proposal defines one target-independent IEEE-754 binary64 evaluation
boundary for the Skald compiler. An unpublished workspace crate wraps
`rustc_apfloat`, exposes only Skald-owned raw-bit and outcome types, and serves
both source-literal conversion and final-MIR constant evaluation. Compiler
passes remain responsible for language policy, checked-protocol recognition,
and structural rewrites.

The purpose is not merely to add floating-point cases to one optimizer match.
It is to establish a durable dependency firewall around a deliberately
unstable third-party API, remove host floating-point evaluation from
compile-time semantics, and make every supported floating fold use the same
auditable binary64 authority.

## Intended outcome

The design should provide:

- a private unpublished `skald-binary64` workspace crate;
- an exact pinned dependency on the published `rustc_apfloat` crate;
- no `rustc_apfloat` type, status flag, rounding enum, or trait in the
  `skald-binary64` public API;
- one raw-bit `Binary64` value with explicit arithmetic, comparison,
  classification, parsing, and conversion operations;
- host- and target-independent nearest-ties-to-even binary64 arithmetic;
- exact truncation-toward-zero integer conversion with explicit failure;
- exact decimal-literal conversion without Rust host `f64` parsing;
- floating constants in the existing convergent callable-local fact domain;
- folding of deterministic floating comparisons, exact bit operations,
  non-NaN floating arithmetic, and casts to or from `f64`;
- a separately selectable rewrite of successful constant checked
  `f64`-to-integer protocols;
- conservative retention of statically failing checked conversions and
  NaN-producing arithmetic;
- no source-language, MIR, backend, runtime ABI, or floating-exception-flag
  extension; and
- focused bit-exact, boundary, differential, source-to-MIR, and native
  conformance tests.

The crate is a compiler implementation component, not a Skald standard-library
or runtime facility. Generated programs do not link it.

## Current boundary and motivating evidence

### Source literals currently use the Rust host

Type checking converts each validated decimal floating literal using
`str::parse::<f64>()`, checks that the result is finite, and stores its raw bits
in HIR. The language contract already requires nearest-ties-to-even binary64
rounding, accepts subnormals and underflow to positive zero, and rejects a
finite spelling that rounds to infinity.

That conversion is normally reliable on Skald's current host, but it leaves a
target-independent language decision expressed through the host's floating
type and parser. The new binary64 boundary should own this conversion without
changing accepted spelling, diagnostics, spans, or HIR bits.

### MIR already carries exact binary64 data

HIR and MIR represent an `f64` constant as one exact `u64` bit pattern.
Primitive operations retain explicit `f64` variants, comparisons retain a
floating operand class, and primitive casts retain exact source, target, and
semantic kind. The x86-64 backend realizes the selected operations; it is not
the authority from which an optimizer should reconstruct their meaning.

This representation is already suitable for software evaluation. No new MIR
floating constant or runtime value representation is needed.

### Primitive evaluation deliberately excludes floating-point work

The existing primitive evaluator supports exact integer and boolean constants
and returns `Unsupported` for `ConstantF64Bits`, floating unary and binary
operations, floating comparisons, bit reinterpretation involving `f64`, and
numeric conversions involving `f64`. That conservative boundary was correct
without a host-independent evaluator.

The convergent local-constant solver and primitive folding pass already provide
the right propagation and assignment-replacement architecture. They should be
extended to carry raw-bit floating facts rather than replaced with a separate
floating dataflow engine.

### Checked floating-to-integer conversion is structural

Each `f64`-to-`i64`, `f64`-to-`u64`, or `f64`-to-`u8` cast lowers to a verified
range-check diamond. The source is secured in scalar storage, the success block
alone converts and initializes a result carrier, the failure block terminates
with `primitive cast out of range`, and the join reloads the result.

Folding the conversion therefore cannot replace only one success-block rvalue.
It requires exact topology observation, certified carrier facts, and one atomic
protocol rewrite. The existing checked-integer machinery supplies useful
architecture, but its topology and carrier vocabulary currently covers only
integer division, remainder, and shifts.

## Constraints and non-goals

- Existing source-visible binary64 arithmetic, comparison, cast, literal, and
  raw-bit semantics remain unchanged.
- Existing source diagnostics, diagnostic codes, spans, evaluation order,
  failure reasons, and exactly-once operand evaluation remain unchanged.
- `f64` remains represented in HIR and MIR by exact raw bits; the compiler does
  not introduce a second floating IR type.
- No host `f64` arithmetic or conversion may participate in production
  compile-time evaluation after the new boundary owns an operation.
- No backend instruction sequence is used as a compile-time semantic oracle.
- Floating exception flags remain unobservable and are not added to HIR, MIR,
  diagnostics, runtime state, or the source language.
- No particular NaN sign, payload, or signaling-state result is added to the
  language contract for arithmetic.
- Floating remainder, square root, fused multiply-add syntax, selectable
  rounding modes, total ordering, and new primitive types are not added.
- No static-failure rewrite is added for a constant failing
  `f64`-to-integer cast. The original checked protocol remains intact.
- No floating algebraic identities such as `x + 0.0`, `x * 1.0`, or
  `x - x` are introduced. Signed zero, NaN, infinity, and rounding make these
  separate optimization decisions.
- No C or C++ floating-point implementation, FFI surface, system library, or
  runtime dependency is introduced.
- `skald-binary64` does not depend on `skald-compiler`, MIR, HIR, diagnostics,
  source spans, pass identities, or backend types.
- The proposal does not promise that the current Rust 1.82 minimum remains
  unchanged. The workspace MSRV should follow the selected dependency and be
  updated deliberately if required.

## Design principles

1. **Bits cross the boundary.** Compiler IR and the binary64 service exchange
   exact `u64` data rather than host floats or dependency types.
2. **Mechanics and policy are separate.** The crate calculates IEEE results;
   compiler consumers decide whether a result is legal to substitute and how
   to rewrite IR.
3. **One compile-time authority.** Literal rounding, arithmetic, comparison,
   and conversion share one implementation rather than accumulating local
   approximations.
4. **Dependency instability is contained.** Only the wrapper implementation
   imports `rustc_apfloat`; its public facade remains stable under dependency
   upgrades.
5. **Language outcomes, not host exceptions.** Public outcomes describe what
   Skald consumers need. APFloat exception flags are consumed privately.
6. **Conservative observable NaNs.** Exact comparisons and bit operations may
   inspect any NaN, but the first arithmetic folding profile does not
   substitute a result whose bits the language leaves unspecified.
7. **Successful checked rewrites first.** Exact success can remove a checked
   diamond; known failure remains on the existing runtime path.
8. **Tests make the dependency replaceable.** Skald-owned vectors pin every
   behavior on which the compiler relies, independently of APFloat's API.

## Decision register

| ID | Decision | Recommended direction | State |
|---|---|---|---|
| [B64E1](#b64e1--crate-and-dependency-boundary) | Ownership | Add an unpublished `skald-binary64` workspace crate around `rustc_apfloat` | **Confirmed** |
| [B64E2](#b64e2--dependency-version-and-toolchain-policy) | Dependency policy | Pin one published crate version and raise the workspace MSRV if necessary | **Confirmed** |
| [B64E3](#b64e3--public-value-and-outcome-model) | Public model | Expose raw-bit `Binary64` and Skald-owned explicit outcomes only | **Confirmed** |
| [B64E4](#b64e4--arithmetic-negation-and-classification) | Arithmetic | Implement binary64 `+`, `-`, `*`, `/`, exact sign negation, and explicit classification predicates | **Confirmed** |
| [B64E5](#b64e5--comparison-semantics) | Comparison | Return an explicit four-way numeric comparison including unordered | **Confirmed** |
| [B64E6](#b64e6--integer-and-boolean-conversions) | Conversions | Provide typed integer entry points and truncating checked integer outcomes | **Confirmed** |
| [B64E7](#b64e7--decimal-literal-conversion) | Literals | Route validated source decimal spellings through exact APFloat parsing | **Confirmed** |
| [B64E8](#b64e8--nan-folding-policy) | NaN policy | Fold exact NaN observations but retain NaN-producing arithmetic | **Confirmed** |
| [B64E9](#b64e9--primitive-evaluation-and-constant-propagation) | Primitive folding | Extend the existing constant domain and evaluator; add no second solver | **Confirmed** |
| [B64E10](#b64e10--successful-checked-conversion-protocol-folding) | Checked casts | Add a separate proof-rich pass for successful constant `f64`-to-integer diamonds | **Confirmed** |
| [B64E11](#b64e11--module-structure-and-api-hygiene) | Maintainability | Keep a small facade with private responsibility modules and explicit re-exports | **Confirmed** |
| [B64E12](#b64e12--validation-and-differential-evidence) | Validation | Combine fixed bit vectors, boundaries, APFloat conformance, and native parity | **Confirmed** |

## Detailed decisions

### B64E1 — Crate and dependency boundary

Add `crates/skald-binary64` as an unpublished workspace member.
`skald-compiler` depends on this crate; no other production crate initially
needs a direct dependency. `skald-binary64` depends on `rustc_apfloat` and
nothing in the Skald compiler graph.

The crate boundary is justified by three durable facts:

- binary64 semantics are shared by type checking and optimization;
- `rustc_apfloat` explicitly describes its API as unstable; and
- a Cargo dependency boundary mechanically prevents its types and traits from
  becoming compiler-wide vocabulary.

This is not intended as a generally reusable numerical library. It implements
the precise compile-time binary64 operations required by Skald. The package is
`publish = false`, uses workspace edition and lint policy, and forbids unsafe
code through the workspace lint.

### B64E2 — Dependency version and toolchain policy

Use the published `rustc_apfloat` crate rather than a Git dependency or a
vendored fork. Pin the accepted release exactly in `skald-binary64/Cargo.toml`
and commit the corresponding `Cargo.lock` update. APFloat build metadata is
not a separate Cargo compatibility boundary, so the lockfile checksum remains
part of the reproducible selection.

Before adoption, compile the wrapper and its tests with the current workspace
toolchain. If the dependency requires a Rust version newer than 1.82, update
`workspace.package.rust-version`, development documentation, and dependency
pins whose comments depend on the old MSRV in the same foundational change.
The compiler should not emulate binary64 merely to retain an arbitrarily old
toolchain floor.

Dependency upgrades are explicit maintenance changes. Each upgrade reruns the
fixed vectors, differential tests, compiler tests, goldens, determinism suite,
and native release suite before its new result is accepted.

The dependency's Apache-2.0 WITH LLVM-exception licensing should be recorded in
the repository's appropriate third-party notice or dependency documentation
during implementation. This proposal does not duplicate the license text.

### B64E3 — Public value and outcome model

The central public type is an opaque copyable value containing one exact
binary64 representation:

```rust
pub struct Binary64 {
    bits: u64,
}
```

Its minimum construction and observation surface is:

```rust
impl Binary64 {
    pub const fn from_bits(bits: u64) -> Self;
    pub const fn to_bits(self) -> u64;
    pub const fn same_bits(self, other: Self) -> bool;
}
```

Do not implement `PartialOrd` or `Ord`: binary64 numeric ordering is unordered
for NaN. Prefer not to implement `PartialEq`/`Eq` either, because bit equality
and numeric equality differ for signed zero and NaNs. Explicitly named
operations prevent an accidental choice between those relations.

Public errors and results are Skald-owned closed enums. They must not wrap or
expose `rustc_apfloat::Status`, `StatusAnd`, `Round`, `Category`, `Double`, or
`ParseError`. Dependency conversion occurs entirely inside private modules.

### B64E4 — Arithmetic, negation, and classification

Expose `add`, `subtract`, `multiply`, and `divide` on `Binary64`. Each uses the
IEEE binary64 format and round-to-nearest, ties-to-even. The returned value
retains exact result bits from the software evaluator, including zero sign,
subnormal, infinity, and NaN results.

APFloat status flags are intentionally consumed inside the wrapper. Skald has
no source-visible floating exception state; divide-by-zero, overflow,
underflow, and inexact are ordinary floating outcomes rather than failures.
The wrapper must nevertheless handle the status match explicitly so a future
dependency API change cannot silently alter control flow.

Negation is exact sign-bit inversion, not an arithmetic operation. It preserves
the remaining 63 bits of zero, finite values, infinities, quiet NaNs, and
signaling NaNs. The public facade exposes explicit `is_zero`, `is_finite`,
`is_infinite`, `is_nan`, and `is_negative` predicates. It does not expose an
APFloat category or a second public classification enum.

No public operation accepts or returns Rust `f64`.

### B64E5 — Comparison semantics

Expose one explicit numeric comparison:

```rust
pub enum Binary64Comparison {
    Less,
    Equal,
    Greater,
    Unordered,
}

pub fn compare(self, other: Self) -> Binary64Comparison;
```

The compiler maps this result to the six existing MIR predicates:

| Comparison | `==` | `!=` | `<` | `<=` | `>` | `>=` |
|---|---:|---:|---:|---:|---:|---:|
| Less | false | true | true | true | false | false |
| Equal | true | false | false | true | false | true |
| Greater | false | true | false | false | true | true |
| Unordered | false | true | false | false | false | false |

Both zeroes compare equal. Either NaN yields `Unordered`. Infinity participates
in ordinary numeric ordering. Bit equality remains the separate `same_bits`
operation.

### B64E6 — Integer and boolean conversions

Use explicit typed functions rather than a public target-width enum coupled to
MIR:

```rust
impl Binary64 {
    pub fn from_i64(value: i64) -> Self;
    pub fn from_u64(value: u64) -> Self;
    pub fn from_u8(value: u8) -> Self;
    pub fn from_bool(value: bool) -> Self;

    pub fn truncating_to_i64(self) -> IntegerConversion<i64>;
    pub fn truncating_to_u64(self) -> IntegerConversion<u64>;
    pub fn truncating_to_u8(self) -> IntegerConversion<u8>;
    pub const fn to_bool(self) -> bool;
}

pub enum IntegerConversion<T> {
    Value(T),
    OutOfRange,
}
```

Integer-to-binary64 conversion uses nearest-ties-to-even. Boolean conversion
produces exact `0.0` or `1.0`. `Binary64::to_bool` is false for either zero and
true for every other representation, including infinities and every NaN.

Floating-to-integer conversion first requires a finite source, truncates toward
zero, and then checks the mathematical truncated value against the target
range. This preserves the existing rule that a negative finite fraction
strictly greater than `-1.0` converts successfully to unsigned zero. Inexact
truncation is a successful conversion; APFloat's `INEXACT` flag alone must
never become `OutOfRange`.

`OutOfRange` intentionally combines NaN, infinity, and finite range failure
because the current language exposes one failure reason. A later diagnostic
consumer would justify a richer private or public failure classification, but
the initial compiler must not speculate one.

Raw `f64`/`u64` reinterpretation does not call APFloat. It uses `from_bits` or
`to_bits` and therefore preserves every representation exactly.

### B64E7 — Decimal literal conversion

Expose a conversion from a lexer-validated decimal spelling using
nearest-ties-to-even:

```rust
pub fn parse_decimal(spelling: &str) -> DecimalConversion;

pub enum DecimalConversion {
    Finite(Binary64),
    Overflow,
    Invalid,
}
```

The compiler remains responsible for validating source grammar before this
call. A malformed spelling reaching the wrapper indicates an internal contract
violation and is represented by `Invalid` without exposing APFloat's parser
error.
Type checking maps `Finite` to the existing HIR raw bits and `Overflow` to the
existing `F64_LITERAL_OUT_OF_RANGE` diagnostic with unchanged span, message,
label, and note.

Subnormal results and underflow to positive zero are `Finite`. A source
spelling that rounds to positive infinity is `Overflow`. Source spelling has
no sign token, so unary negation continues to supply negative values after
literal conversion. Infinity and NaN remain unavailable as literal spellings.

Replacing host parsing is part of establishing the semantic authority, not a
prerequisite for the first optimizer fold. It should nevertheless occur before
the design is considered fully delivered.

The Skald-written `Str.to_f64` and `Str.from_f64` implementations remain
ordinary target code and do not call this compiler crate. Their exhaustive
runtime conversion algorithms are useful independent conformance evidence,
not code to be replaced by a compiler dependency.

### B64E8 — NaN folding policy

The binary64 crate calculates and returns APFloat's deterministic arithmetic
NaN representation. The first optimizer profile does not substitute an
arithmetic assignment when the calculated result is NaN.

This rule preserves exact optimization-off/default parity when a program
observes the result through `std::f64::to_bits`, even though the language does
not promise a particular arithmetic NaN sign, payload, or signaling state. It
also avoids silently making APFloat's NaN propagation convention part of the
language contract.

The restriction applies to arithmetic production, not all NaN-related folds.
These remain exact and eligible:

- a raw NaN constant as a solver fact;
- identity casts preserving the same bits;
- sign negation by exact sign-bit inversion;
- `to_bits`/`from_bits` reinterpretation;
- conversion to `bool`; and
- all six numeric comparisons through the explicit unordered result.

Integer-to-floating conversion cannot produce NaN and is unaffected.
Floating-to-integer conversion of NaN reports `OutOfRange`, so its checked
protocol remains unchanged under the initial successful-only rewrite policy.

A later design may permit canonical or target-profiled NaN-result folding, but
it must first decide whether optimization profiles are allowed to choose
different observable raw bits.

### B64E9 — Primitive evaluation and constant propagation

Extend the existing `PrimitiveConstant` domain with `F64Bits(u64)`. Retaining
raw bits keeps its `Eq` semantics internal and exact without asking Rust `f64`
to define equality. Conversion back to MIR produces
`MirRvalueKind::ConstantF64Bits` with the same bits.

The existing primitive evaluator remains the sole authority translating MIR
operations into constant outcomes. A cohesive private floating helper may own
the mapping to `skald-binary64`, while the evaluator facade continues to own
type matching and `PrimitiveEvaluation::{Constant, Unsupported}`.

The supported initial matrix is:

| MIR family | Initial constant behavior |
|---|---|
| Existing `ConstantF64Bits` | Record exact `F64Bits` fact |
| `NegateF64` | Fold by exact sign-bit inversion |
| `AddF64`, `SubtractF64`, `MultiplyF64`, `DivideF64` | Fold only when the computed result is not NaN |
| Floating comparisons | Fold all operands, including NaNs, signed zeroes, and infinities |
| `f64` identity | Preserve exact bits |
| `f64` to `bool` | Fold both zeroes to false and every other value to true |
| Integer or `bool` to `f64` | Fold with exact nearest-ties-to-even conversion |
| `f64`/`u64` bit reinterpretation | Fold by exact raw-bit transfer |
| Checked `f64` to integer rvalue | Evaluated only as part of its verified protocol |

The convergent callable-local solver then propagates floating facts through the
same ordinary value graph as existing primitive facts. It gains no floating
lattice, approximate value, range fact, host comparison, or second worklist.

Primitive algebraic simplification remains unchanged. Constant folding is
permitted because both operands are exact. Identities involving a dynamic
floating operand remain excluded.

### B64E10 — Successful checked-conversion protocol folding

Add one independently selectable proof-rich pass named
`checked-f64-to-integer-constant-folding`. Do not hide the work in the existing
`checked-integer-constant-folding` registration: the source domain, topology,
evaluation, metrics, and future maintenance risks are distinct.

The pass should reuse the existing architecture where its contracts generalize:

1. observe an exact `PrimitiveCastRangeCheck` topology;
2. certify the source and result scalar carriers and their lifetime/use sites;
3. obtain the exact source `F64Bits` fact from the convergent local solution;
4. evaluate the target-specific conversion through `skald-binary64`;
5. retain `OutOfRange` protocols unchanged;
6. plan every edit for a callable before mutation;
7. atomically replace a successful protocol with the exact integer constant;
8. preserve operand evaluation, result identity consumers, source span, and
   surrounding ownership/control effects; and
9. let dense commit and central verification validate the result.

Shared checked-carrier machinery should be generalized around exact
checked-scalar protocol ownership rather than making floating casts pretend to
be integer division. Protocol-specific topology and evaluation remain in
separate modules. The local solver may consume certified facts from either
protocol family without depending on the rewrite pass being enabled.

The pass reports folded `i64`, `u64`, and `u8` conversions, propagated-source
folds, removed protocol values, and retained static failures. Pass ordering
places it after primitive constant folding has made ordinary floating facts
available and before dead-pure and CFG cleanup. The default schedule may repeat
primitive folding afterward so a constant result enables its consumers.

Known failures deliberately remain executable checked protocols. This matches
the existing treatment of constant zero divisors and out-of-range shift counts
and avoids introducing a general static-failure rewrite policy.

### B64E11 — Module structure and API hygiene

The new crate should use a concise facade-oriented recursive layout, initially:

```text
crates/skald-binary64/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── value.rs
    ├── conversion.rs
    ├── decimal.rs
    └── tests.rs
```

`lib.rs` owns crate documentation, private module declarations, and explicit
re-exports of the small public facade. `value.rs` owns raw representation,
classification, arithmetic, and numeric comparison. `conversion.rs` owns
integer and boolean conversions. `decimal.rs` owns decimal parsing and its
outcome. If implementation shows that two of these remain trivial, they may be
combined rather than preserving needless files.

Only private implementation modules import `rustc_apfloat`. A repository
search outside `crates/skald-binary64` for `rustc_apfloat` should remain empty.
The wrapper API contains no generic APFloat abstraction, selectable semantic
format, arbitrary precision, MIR enum, or backend target.

Inside `skald-compiler`, keep MIR-to-binary64 mapping private to primitive
evaluation. Extract a floating helper when the existing evaluator would
otherwise mix substantial APFloat-adapter calls with integer mechanics. Keep
the main evaluator facade and its established crate-private re-exports stable.

### B64E12 — Validation and differential evidence

The binary64 crate owns focused tests over raw bits. At minimum they cover:

- positive and negative zero;
- minimum and maximum subnormals;
- minimum normal and maximum finite values;
- both infinities;
- quiet and signaling NaNs with several signs and payloads;
- exact, inexact, overflow, underflow, and gradual-underflow arithmetic;
- cancellation and signed-zero production;
- division by both zeroes and zero divided by zero;
- comparisons for every ordered relation and either-operand NaN;
- integers around `2^53`, `2^63`, and `2^64` rounding boundaries;
- every `i64`, `u64`, and `u8` conversion boundary plus adjacent binary64
  values;
- negative fractions around zero for unsigned conversion;
- finite decimal extrema, subnormals, halfway cases, underflow to zero, and
  overflow to infinity; and
- fixed regression vectors for every dependency behavior the wrapper uses.

Tests should compute expected results as literal bit patterns or integer
outcomes, not with host `f64`. APFloat upstream tests support confidence in the
dependency, but they do not replace Skald-owned contract tests.

Compiler-local tests cover exhaustive operation/type routing, unsupported
pairs, NaN arithmetic retention, all comparison predicates, propagated facts,
and checked topology/rewrite guards. Type-checker tests prove unchanged literal
bits and diagnostics. MIR and verifier tests prove exact successful protocol
replacement and malformed-shape rejection.

Golden tests cover source-to-native arithmetic, comparison, conversions,
literal boundaries, raw-bit observations, optimization `none`/`default`
equivalence, deterministic MIR and assembly, and the existing VM benchmark.
Selected vectors should compare software-evaluated results with native x86-64
execution. That comparison is evidence about the current target, not the
definition of compile-time semantics.

The complete repository gates remain `make check` and `make check-long`, with
focused crate, compiler, golden, release, and determinism commands used during
implementation. If the workspace MSRV changes, `make msrv-check` must exercise
the new declared value.

## Public facade sketch

The exact Rust spelling remains an implementation detail until the design is
frozen, but the intended dependency-free surface is approximately:

```rust
#[derive(Clone, Copy, Debug)]
pub struct Binary64 {
    bits: u64,
}

impl Binary64 {
    pub const fn is_zero(self) -> bool;
    pub const fn is_finite(self) -> bool;
    pub const fn is_infinite(self) -> bool;
    pub const fn is_nan(self) -> bool;
    pub const fn is_negative(self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binary64Comparison {
    Less,
    Equal,
    Greater,
    Unordered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerConversion<T> {
    Value(T),
    OutOfRange,
}

#[derive(Clone, Copy, Debug)]
pub enum DecimalConversion {
    Finite(Binary64),
    Overflow,
    Invalid,
}
```

Classification uses the explicit predicates above. No currently identified
consumer requires APFloat's complete category or status vocabulary.

## Compiler ownership after delivery

| Responsibility | Owner |
|---|---|
| Software binary64 arithmetic and rounding | `skald-binary64` |
| Raw-bit representation and classification | `skald-binary64` |
| Decimal spelling to binary64 bits | `skald-binary64` |
| Integer/bool conversion mechanics | `skald-binary64` |
| Source spelling validation and diagnostics | lexer/type checker |
| Source-visible floating semantics | language documentation |
| MIR operation and checked-protocol representation | MIR model/lowering/verifier |
| Constant provenance and propagation | existing local-constant solver |
| Fold eligibility, NaN policy, and assignment replacement | primitive constant-folding pass |
| Checked-diamond recognition and atomic rewrite | checked floating-cast folding pass |
| Runtime instruction realization | target backend |
| Runtime string parsing and formatting | Skald standard library |

This division prevents the wrapper from becoming either a miniature compiler
IR or a source-language policy layer.

## Alternatives considered

### Keep the wrapper inside `skald-compiler`

A private compiler module would reduce workspace structure. It would not
mechanically prevent other compiler phases from importing APFloat directly,
and the shared literal/optimization responsibility does not belong naturally
to either type checking or the pass hierarchy. The unstable dependency and
cohesive MIR-independent semantics make the small crate boundary worthwhile.

### Use host Rust `f64` with special-case guards

This is smaller but makes compiler results depend on the host implementation
and asks local guards to grow around NaNs, conversions, rounding boundaries,
and future targets. It does not provide the desired target-independent
semantic authority.

### Implement binary64 arithmetic in Skald

A custom implementation would avoid a dependency but would be much larger
than the wrapper and optimizer work combined. Correct rounding, subnormals,
NaNs, overflow, and integer boundaries would create a substantial numerical
software project outside the compiler's purpose.

### Depend directly on Berkeley SoftFloat through FFI

Berkeley SoftFloat is a strong reference implementation, but a direct wrapper
adds C compilation, FFI, unsafe implementation boundaries, and floating
environment state. It is better suited as an optional external oracle for
differential testing than as Skald's production compile-time dependency.

### Expose APFloat types to `skald-compiler`

This minimizes adapter code but couples compiler phases to an explicitly
unstable API and lets dependency-specific status and rounding concepts become
accidental architecture. The proposed opaque bit boundary costs little and
keeps replacement feasible.

## Delivery boundary

B64E1 through B64E12 are frozen together. The living compiler documentation
owns their durable architecture and phase boundary, while the active roadmap
owns their implementation order and completion evidence.

Implementation should establish the crate and test oracle before migrating
literal conversion or enabling optimization. Pure floating evaluation should
precede checked-protocol mutation. Default-profile activation should occur only
after optimization-off/default native parity, NaN retention, deterministic
artifacts, dependency licensing, and the complete repository gates pass.
